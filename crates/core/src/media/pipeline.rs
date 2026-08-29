// =================================================================================================
// media::pipeline — Download → Split → Analyze → Persist
// =================================================================================================

use std::path::{Path, PathBuf};
use std::sync::Arc;

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

        // 1. Download (0.00 - 0.25)
        check_cancelled(&ctx.cancellation)?;
        on_event(PipelineEvent::StageChanged(Stage::Downloading));
        on_event(PipelineEvent::Progress(
            0.05,
            "Downloading (low res for analysis)".into(),
        ));
        ytdlp::download_video(
            &ctx.url,
            &video_path,
            cookies.use_cookies,
            (!cookies.path.is_empty()).then_some(cookies.path.as_str()),
            true,
            processing.retry_attempts,
            &ctx.cancellation,
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
        let chunks =
            chunk::split_video(&video_path, &ranges, &ctx.work_dir, &ctx.cancellation).await?;
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
                "auto_extract: {info} — extraction ships with export milestone (M5)"
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
