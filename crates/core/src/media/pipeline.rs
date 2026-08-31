// =================================================================================================
// media::pipeline — Download → Split → Analyze → Persist
// =================================================================================================

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::ai::build_registry;
use crate::config::AppConfig;
use crate::media::{chunk, ffmpeg, ytdlp};
use crate::session;
use crate::types::VideoMoment;

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Stage {
    Downloading,
    Splitting,
    Analyzing { current: usize, total: usize },
    Extracting,
}

#[derive(Debug, Clone)]
pub enum PipelineEvent {
    StageChanged(Stage),
    Progress(f32, String),
    Log(String),
}

#[derive(Debug, Clone)]
pub struct PipelineCtx {
    pub url: String,
    pub config: Arc<AppConfig>,
    /// Scratch directory for the downloaded video and chunks.
    pub work_dir: PathBuf,
    /// Directory where final products land.
    pub output_dir: PathBuf,
    pub cancellation: CancellationToken,
}

#[derive(Debug, Clone)]
pub struct RunOutput {
    pub video_id: String,
    pub video_path: PathBuf,
    pub chunks: Vec<crate::types::VideoChunk>,
    pub moments: Vec<VideoMoment>,
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

pub struct Pipeline;

impl Pipeline {
    pub fn new() -> Self {
        Self
    }

    pub async fn run<F>(&self, ctx: PipelineCtx, mut on_event: F) -> anyhow::Result<RunOutput>
    where
        F: FnMut(PipelineEvent) + Send + 'static,
    {
        // Fail fast if external tools are missing — avoids generic "yt-dlp failed" downstream.
        crate::setup::check_dependencies()?;

        let cfg = ctx.config.clone();
        let processing = &cfg.processing;
        let cookies = &cfg.cookies;
        let provider_registry = build_registry(&cfg.ai);

        let video_id = ytdlp::extract_video_id(&ctx.url)
            .unwrap_or_else(|| format!("video_{}", chrono::Local::now().format("%Y%m%d%H%M%S")));
        let video_path = ctx.work_dir.join(format!("{video_id}.mp4"));

        // 1. Download (0.00 - 0.25) — streams yt-dlp output line-by-line as Log events
        check_cancelled(&ctx.cancellation)?;
        on_event(PipelineEvent::StageChanged(Stage::Downloading));
        on_event(PipelineEvent::Progress(
            0.05,
            "Downloading (low res for analysis)".into(),
        ));
        on_event(PipelineEvent::Log(format!(
            "yt-dlp: {} -o {} -f worst[ext=mp4]/worst {}",
            ctx.url,
            video_path.display(),
            if cookies.use_cookies { "(cookies)" } else { "" }
        )));
        download_with_logs(
            &ctx.url,
            &video_path,
            cookies.use_cookies,
            (!cookies.path.is_empty()).then_some(cookies.path.as_str()),
            true,
            processing.retry_attempts,
            &ctx.cancellation,
            &mut on_event,
        )
        .await?;
        on_event(PipelineEvent::Progress(0.25, "Download complete".into()));

        // 2. Split (0.25 - 0.40)
        check_cancelled(&ctx.cancellation)?;
        on_event(PipelineEvent::StageChanged(Stage::Splitting));
        if let Ok((w, h)) = ffmpeg::get_resolution(&video_path) {
            on_event(PipelineEvent::Log(format!("Source resolution: {w}x{h}")));
        }
        let duration = ffmpeg::get_duration(&video_path)?;
        let ranges = chunk::calculate_chunks(
            duration.max(0.0) as u64,
            processing.chunk_size_secs,
            processing.max_last_chunk_secs,
        );
        if ranges.is_empty() {
            anyhow::bail!("video duration is 0; cannot split");
        }
        on_event(PipelineEvent::Log(format!(
            "Splitting {}s into {} chunks ({}s each)",
            duration as u64,
            ranges.len(),
            processing.chunk_size_secs
        )));
        // Emit per-chunk ffmpeg commands as logs before running
        for (i, (start, dur)) in ranges.iter().enumerate() {
            on_event(PipelineEvent::Log(format!(
                "ffmpeg: -ss {start} -i {} -t {dur} -c copy {}",
                video_path.display(),
                ctx.work_dir.join(format!("chunk_{i}.mp4")).display()
            )));
        }
        let chunks = split_with_logs(
            &video_path,
            &ranges,
            &ctx.work_dir,
            &ctx.cancellation,
            &mut on_event,
        )
        .await?;
        on_event(PipelineEvent::Progress(0.40, "Split complete".into()));

        // 3. Analyze per chunk (0.40 - 0.90)
        let provider = provider_registry
            .active()
            .ok_or_else(|| anyhow::anyhow!("no active AI provider"))?;
        let mut moments: Vec<VideoMoment> = Vec::new();
        let total = chunks.len();
        for (i, vc) in chunks.iter().enumerate() {
            check_cancelled(&ctx.cancellation)?;
            on_event(PipelineEvent::StageChanged(Stage::Analyzing {
                current: i + 1,
                total,
            }));
            on_event(PipelineEvent::Log(format!(
                "Analyzing chunk {}/{} (start {}s)",
                i + 1,
                total,
                vc.start_seconds
            )));

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<crate::ai::ProviderEvent>();
            let chunk_moments = provider
                .analyze_chunk(crate::ai::AnalyzeCtx {
                    chunk_path: PathBuf::from(&vc.file_path),
                    chunk_start_offset: std::time::Duration::from_secs(vc.start_seconds),
                    cancellation: ctx.cancellation.clone(),
                    progress_tx: Some(tx),
                })
                .await?;
            // Drain any provider events queued during analyze
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    crate::ai::ProviderEvent::Log(msg) => on_event(PipelineEvent::Log(msg)),
                    crate::ai::ProviderEvent::Progress(p) => on_event(PipelineEvent::Progress(
                        0.40 + 0.50 * p,
                        format!("AI {:.0}%", p * 100.0),
                    )),
                }
            }

            on_event(PipelineEvent::Log(format!(
                "Chunk {}: found {} moments",
                i + 1,
                chunk_moments.len()
            )));
            moments.extend(chunk_moments);

            let frac = (i + 1) as f32 / total as f32;
            on_event(PipelineEvent::Progress(
                0.40 + 0.50 * frac,
                format!("Analyzing {}/{}", i + 1, total),
            ));

            if i + 1 < total && processing.chunk_delay_secs > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(processing.chunk_delay_secs))
                    .await;
            }
        }

        // 4. Persist to DB
        persist(&ctx, &video_id, &video_path, &chunks, &moments)?;
        on_event(PipelineEvent::Progress(0.95, "Saved to database".into()));

        // 5. Optional auto-extract (0.95 - 1.00)
        if processing.auto_extract {
            on_event(PipelineEvent::StageChanged(Stage::Extracting));
            let plano = crate::plano::schema::create_default_plano();
            let info = crate::plano::preview::preview_info(&plano);
            on_event(PipelineEvent::Log(format!(
                "auto_extract: {info} - extraction ships with export milestone (M5)"
            )));
            // Demonstrate save_plano wiring (writes plano.json to work_dir for inspection)
            let tmp_plano = ctx.work_dir.join("plano.json");
            let _ = crate::plano::schema::save_plano(&tmp_plano.to_string_lossy(), &plano);
        }

        on_event(PipelineEvent::Progress(1.0, "Done".into()));
        Ok(RunOutput {
            video_id,
            video_path,
            chunks,
            moments,
        })
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

// -------------------------------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn download_with_logs<F>(
    url: &str,
    output: &Path,
    use_cookies: bool,
    cookies_path: Option<&str>,
    low_res: bool,
    retry_attempts: u32,
    cancellation: &CancellationToken,
    on_event: &mut F,
) -> anyhow::Result<()>
where
    F: FnMut(PipelineEvent) + Send + 'static,
{
    let mut last_err = None;
    for attempt in 0..retry_attempts.max(1) {
        if cancellation.is_cancelled() {
            anyhow::bail!("cancelled");
        }
        match download_once_with_logs(
            url,
            output,
            use_cookies,
            cookies_path,
            low_res,
            cancellation,
            on_event,
        )
        .await
        {
            Ok(()) => return Ok(()),
            Err(e) => {
                let msg = e.to_string();
                on_event(PipelineEvent::Log(format!(
                    "yt-dlp attempt {}/{} failed: {msg}",
                    attempt + 1,
                    retry_attempts.max(1)
                )));
                last_err = Some(e);
                if attempt + 1 < retry_attempts.max(1) {
                    let backoff = std::time::Duration::from_secs(1 << attempt);
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("download failed")))
}

async fn download_once_with_logs<F>(
    url: &str,
    output: &Path,
    use_cookies: bool,
    cookies_path: Option<&str>,
    low_res: bool,
    cancellation: &CancellationToken,
    on_event: &mut F,
) -> anyhow::Result<()>
where
    F: FnMut(PipelineEvent) + Send + 'static,
{
    let mut cmd = Command::new(crate::setup::yt_dlp_bin());
    cmd.arg("--no-playlist")
        .arg("-o")
        .arg(output.to_string_lossy().into_owned());
    if low_res {
        cmd.arg("-f").arg("worst[ext=mp4]/worst");
    } else {
        cmd.arg("-f")
            .arg("bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best");
        cmd.arg("--merge-output-format").arg("mp4");
    }
    if use_cookies {
        if let Some(p) = cookies_path.filter(|p| !p.is_empty()) {
            cmd.arg("--cookies").arg(p);
        }
    }
    cmd.arg(url);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            crate::setup::yt_dlp_missing_error()
        } else {
            anyhow::Error::from(e).context("failed to run yt-dlp")
        }
    })?;
    let stderr = child.stderr.take();
    if let Some(stderr) = stderr {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        loop {
            if cancellation.is_cancelled() {
                let _ = child.kill().await;
                anyhow::bail!("cancelled");
            }
            line.clear();
            let n = reader
                .read_line(&mut line)
                .await
                .context("reading yt-dlp stderr")?;
            if n == 0 {
                break;
            }
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() {
                on_event(PipelineEvent::Log(format!("[yt-dlp] {trimmed}")));
            }
            // Small cooperative yield to allow cancellation
            if line.len() > 4096 {
                line.clear();
            }
        }
    }
    // Also drain stdout if any
    if let Some(stdout) = child.stdout.take() {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            let n = reader
                .read_line(&mut line)
                .await
                .context("reading yt-dlp stdout")?;
            if n == 0 {
                break;
            }
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() {
                on_event(PipelineEvent::Log(format!("[yt-dlp] {trimmed}")));
            }
        }
    }
    let status = child.wait().await.context("waiting for yt-dlp")?;
    if !status.success() {
        anyhow::bail!("yt-dlp download failed (exit {status})");
    }
    Ok(())
}

async fn split_with_logs<F>(
    input: &Path,
    chunks: &[(u64, u64)],
    output_dir: &Path,
    cancellation: &CancellationToken,
    on_event: &mut F,
) -> anyhow::Result<Vec<crate::types::VideoChunk>>
where
    F: FnMut(PipelineEvent) + Send + 'static,
{
    std::fs::create_dir_all(output_dir).context("creating chunks dir")?;
    let mut video_chunks = Vec::new();
    for (i, (start, duration)) in chunks.iter().enumerate() {
        if cancellation.is_cancelled() {
            anyhow::bail!("cancelled");
        }
        let chunk_path = output_dir.join(format!("chunk_{i}.mp4"));
        let mut cmd = Command::new(crate::setup::ffmpeg_bin());
        cmd.args([
            "-v",
            "error",
            "-y",
            "-ss",
            &start.to_string(),
            "-i",
            &input.to_string_lossy(),
            "-t",
            &duration.to_string(),
            "-c",
            "copy",
            "-avoid_negative_ts",
            "make_zero",
            &chunk_path.to_string_lossy(),
        ]);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                crate::setup::ffmpeg_missing_error()
            } else {
                anyhow::Error::from(e).context("failed to run ffmpeg")
            }
        })?;
        // Stream stderr
        if let Some(stderr) = child.stderr.take() {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                let n = reader
                    .read_line(&mut line)
                    .await
                    .context("reading ffmpeg stderr")?;
                if n == 0 {
                    break;
                }
                let trimmed = line.trim().to_string();
                if !trimmed.is_empty() {
                    on_event(PipelineEvent::Log(format!("[ffmpeg chunk {i}] {trimmed}")));
                }
                if cancellation.is_cancelled() {
                    let _ = child.kill().await;
                    anyhow::bail!("cancelled");
                }
            }
        }
        let status = child.wait().await.context("waiting for ffmpeg")?;
        if !status.success() {
            anyhow::bail!("ffmpeg split failed for chunk {i} (exit {status})");
        }
        video_chunks.push(crate::types::VideoChunk {
            start_seconds: *start,
            file_path: chunk_path.to_string_lossy().to_string(),
        });
    }
    Ok(video_chunks)
}

fn check_cancelled(token: &CancellationToken) -> anyhow::Result<()> {
    if token.is_cancelled() {
        anyhow::bail!("cancelled");
    }
    Ok(())
}

fn persist(
    ctx: &PipelineCtx,
    video_id: &str,
    video_path: &Path,
    chunks: &[crate::types::VideoChunk],
    moments: &[VideoMoment],
) -> anyhow::Result<()> {
    let conn = session::init_db(&session::db_path()?)?;
    let project_id = format!("{video_id}-{}", chrono::Local::now().format("%Y%m%d%H%M%S"));
    let title = video_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| video_id.to_owned());
    session::insert_project(
        &conn,
        &session::Project {
            id: project_id.clone(),
            url: ctx.url.clone(),
            video_id: video_id.to_owned(),
            title,
            duration: ffmpeg::get_duration(video_path).unwrap_or(0.0),
            status: "analyzed".to_owned(),
        },
    )?;
    session::insert_chunks(&conn, &project_id, chunks)?;
    session::insert_moments(&conn, &project_id, moments)?;
    Ok(())
}
