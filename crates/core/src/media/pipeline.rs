// =================================================================================================
// media::pipeline â€” Download â†’ Split â†’ Analyze â†’ Persist (video-centric, resume via folder+DB)
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
    /// Persistent directory for this video (verified on resume via folder + DB).
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
        crate::setup::check_dependencies()?;

        let cfg = ctx.config.clone();
        let processing = &cfg.processing;
        let cookies = &cfg.cookies;
        let provider_registry = build_registry(&cfg.ai);

        // Stable id: centralized in session::resolve_stable_id â€” never timestamp/"video"
        let video_id = session::resolve_stable_id(&ctx.url);
        tracing::info!(
            "pipeline start video_id={} url={} work_dir={}",
            video_id,
            ctx.url,
            ctx.work_dir.display()
        );
        tracing::debug!(
            "config chunk_size={} max_last={} delay={} retry={}",
            processing.chunk_size_secs,
            processing.max_last_chunk_secs,
            processing.chunk_delay_secs,
            processing.retry_attempts
        );
        // Work dir is per-video persistent (session::video_work_dir). Verify folder exists.
        std::fs::create_dir_all(&ctx.work_dir).context("creating work_dir")?;
        tracing::debug!("work_dir verified {}", ctx.work_dir.display());
        let video_path = ctx.work_dir.join(format!("{video_id}.mp4"));

        // Ensure job row exists (for folder+DB verification)
        let title_hint = video_id.clone();
        let db_path = session::db_path()?;
        let conn = session::init_db(&db_path)?;
        let existing_job = session::get_video_job(&conn, &video_id).ok().flatten();
        if existing_job.is_none() {
            // Create placeholder job; title/duration will be updated after download
            let job = session::VideoJob {
                video_id: video_id.clone(),
                url: ctx.url.clone(),
                title: title_hint.clone(),
                duration: 0.0,
                work_dir: ctx.work_dir.to_string_lossy().to_string(),
                download_path: Some(video_path.to_string_lossy().to_string()),
                download_verified: false,
                split_verified: false,
                total_chunks: 0,
                analyzed_chunks: 0,
                status: "pending".into(),
            };
            let _ = session::upsert_video_job(&conn, &job);
        }
        drop(conn);

        // 1. Download â€” verify folder+file+DB before deciding
        check_cancelled(&ctx.cancellation)?;
        on_event(PipelineEvent::StageChanged(Stage::Downloading));
        let download_ok = is_download_verified(&video_id, &video_path);
        tracing::info!(
            "download check video_id={} path={} verified={}",
            video_id,
            video_path.display(),
            download_ok
        );
        if download_ok {
            tracing::info!("download skip yt-dlp verified {}", video_path.display());
            on_event(PipelineEvent::Log(format!(
                "download verified on disk+DB, skipping yt-dlp for {video_id}"
            )));
            on_event(PipelineEvent::Progress(1.0, "Download verified".into()));
            update_job_status(&video_id, "splitting", false)?;
        } else {
            tracing::info!("download start {} -> {}", ctx.url, video_path.display());
            if cookies.use_cookies {
                tracing::debug!("using cookies {}", cookies.path);
            }
            // Invalidate downstream if download missing
            update_job_download(&video_id, &video_path, false, 0.0)?;
            on_event(PipelineEvent::Progress(
                0.0,
                "Downloading (low res for analysis)".into(),
            ));
            on_event(PipelineEvent::Log(format!(
                "yt-dlp: {} -o {} -f {} {}",
                ctx.url,
                video_path.display(),
                ytdlp::LOW_RES_FORMAT,
                if cookies.use_cookies { "(cookies)" } else { "" }
            )));
            if let Err(e) = download_with_logs(
                &ctx.url,
                &video_path,
                cookies.use_cookies,
                (!cookies.path.is_empty()).then_some(cookies.path.as_str()),
                true,
                processing.retry_attempts,
                &ctx.cancellation,
                &mut on_event,
            )
            .await
            {
                tracing::error!("download failed video_id={} err={:#}", video_id, e);
                return Err(e);
            }
            // Verify downloaded file
            let dur = ffmpeg::get_duration(&video_path).unwrap_or(0.0);
            tracing::info!(
                "download complete {} duration={:.1}s size={} bytes",
                video_path.display(),
                dur,
                std::fs::metadata(&video_path).map(|m| m.len()).unwrap_or(0)
            );
            if dur <= 0.1 {
                tracing::error!(
                    "downloaded duration invalid {} dur={}",
                    video_path.display(),
                    dur
                );
                anyhow::bail!("downloaded video duration invalid; file likely corrupt");
            }
            update_job_download(&video_id, &video_path, true, dur)?;
            on_event(PipelineEvent::Progress(1.0, "Download complete".into()));
        }

        // 2. Split â€” verify folder+chunks+DB
        check_cancelled(&ctx.cancellation)?;
        on_event(PipelineEvent::StageChanged(Stage::Splitting));
        on_event(PipelineEvent::Progress(0.0, "Splitting".into()));
        tracing::info!("split start source={}", video_path.display());
        if let Ok((w, h)) = ffmpeg::get_resolution(&video_path) {
            tracing::info!(
                "source resolution {}x{} file={}",
                w,
                h,
                video_path.display()
            );
            on_event(PipelineEvent::Log(format!("Source resolution: {w}x{h}")));
        } else {
            tracing::warn!("could not get resolution for {}", video_path.display());
        }
        let duration = ffmpeg::get_duration(&video_path).inspect_err(|e| {
            tracing::error!("get_duration failed {} err={:#}", video_path.display(), e)
        })?;
        let ranges = chunk::calculate_chunks(
            duration.max(0.0) as u64,
            processing.chunk_size_secs,
            processing.max_last_chunk_secs,
        );
        if ranges.is_empty() {
            anyhow::bail!("video duration is 0; cannot split");
        }
        // Folder+DB verification for split
        let split_ok = is_split_verified(&video_id, &ctx.work_dir, &ranges);
        tracing::info!(
            "split check video_id={} verified={} expected_chunks={}",
            video_id,
            split_ok,
            ranges.len()
        );
        let chunks: Vec<crate::types::VideoChunk> = if split_ok {
            tracing::info!("split skip ffmpeg verified chunks={}", ranges.len());
            on_event(PipelineEvent::Log(format!(
                "split verified on disk+DB: {} chunks, skipping ffmpeg",
                ranges.len()
            )));
            // Load chunks from DB
            let conn = session::init_db(&session::db_path()?)?;
            let stored = session::get_job_chunks(&conn, &video_id)?;
            tracing::debug!("loaded {} chunks from DB for {}", stored.len(), video_id);
            if stored.len() == ranges.len() {
                stored
            } else {
                tracing::warn!(
                    "split verified but DB len {} != expected {} â€” re-splitting",
                    stored.len(),
                    ranges.len()
                );
                // Mismatch despite verified flag â€” re-split
                split_and_persist(
                    &video_id,
                    &video_path,
                    &ranges,
                    &ctx.work_dir,
                    &ctx.cancellation,
                    &mut on_event,
                )
                .await?
            }
        } else {
            tracing::info!(
                "splitting {}s into {} chunks",
                duration as u64,
                ranges.len()
            );
            on_event(PipelineEvent::Log(format!(
                "Splitting {}s into {} chunks ({}s each)",
                duration as u64,
                ranges.len(),
                processing.chunk_size_secs
            )));
            for (i, (start, dur)) in ranges.iter().enumerate() {
                let msg = format!(
                    "ffmpeg: -ss {start} -i {} -t {dur} -c copy {}",
                    video_path.display(),
                    ctx.work_dir.join(format!("chunk_{i}.mp4")).display()
                );
                tracing::debug!("{}", msg);
                on_event(PipelineEvent::Log(msg));
            }
            split_and_persist(
                &video_id,
                &video_path,
                &ranges,
                &ctx.work_dir,
                &ctx.cancellation,
                &mut on_event,
            )
            .await
            .inspect_err(|e| tracing::error!("split failed video_id={} err={:#}", video_id, e))?
        };
        on_event(PipelineEvent::Progress(1.0, "Split complete".into()));

        // 3. Analyze per chunk â€” resume by verifying each chunk's status sequentially
        let provider = provider_registry
            .active()
            .ok_or_else(|| anyhow::anyhow!("no active AI provider"))?;
        let total = chunks.len();
        tracing::info!(
            "analyze start total_chunks={} analyzed_cached={}",
            total,
            session::init_db(&session::db_path()?)
                .ok()
                .and_then(|c| session::get_job_chunk_status(&c, &video_id).ok())
                .map(|v| v.iter().filter(|(_, s)| s == "analyzed").count())
                .unwrap_or(0)
        );
        // Load existing moments for resume (already appended per chunk)
        let conn = session::init_db(&session::db_path()?)?;
        let mut moments: Vec<VideoMoment> =
            session::get_job_moments(&conn, &video_id).unwrap_or_default();
        tracing::debug!("loaded {} existing moments for {}", moments.len(), video_id);
        // Ensure we don't have more moments than expected due to prior partial run â€” keep as is
        drop(conn);
        // Build set of already analyzed chunk indices (mutable â€” updated after each persist)
        let mut analyzed_set = get_analyzed_indices(&video_id);
        tracing::debug!("analyzed_set={:?} for {}", analyzed_set, video_id);
        for (i, vc) in chunks.iter().enumerate() {
            check_cancelled(&ctx.cancellation)?;
            tracing::info!(
                "chunk {}/{} path={} start={}s analyzed={}",
                i + 1,
                total,
                vc.file_path,
                vc.start_seconds,
                analyzed_set.contains(&(i as i64))
            );
            if analyzed_set.contains(&(i as i64)) {
                // Strictly verify file still exists before skipping
                if !Path::new(&vc.file_path).exists() {
                    tracing::error!("chunk {} marked analyzed but missing {}", i, vc.file_path);
                    anyhow::bail!("chunk {} marked analyzed but file missing at {} â€” abort resume to avoid skipping", i, vc.file_path);
                }
                tracing::info!("chunk {}/{} skip cached", i + 1, total);
                on_event(PipelineEvent::Log(format!(
                    "Chunk {}/{} already analyzed, skipping (verified on disk+DB)",
                    i + 1,
                    total
                )));
                on_event(PipelineEvent::Progress(
                    (i + 1) as f32 / total as f32,
                    format!("Analyzing {}/{} (cached)", i + 1, total),
                ));
                continue;
            }
            // Must not skip ahead: ensure all prior chunks were analyzed
            for prev in 0..i {
                if !analyzed_set.contains(&(prev as i64)) {
                    tracing::error!(
                        "strict order violated cannot analyze {} before {} analyzed",
                        i,
                        prev
                    );
                    anyhow::bail!("cannot analyze chunk {} before chunk {} is analyzed â€” strict order violated", i, prev);
                }
            }
            on_event(PipelineEvent::StageChanged(Stage::Analyzing {
                current: i + 1,
                total,
            }));
            let step_frac_start = i as f32 / total as f32;
            on_event(PipelineEvent::Progress(
                step_frac_start,
                format!("Analyzing {}/{}", i + 1, total),
            ));
            on_event(PipelineEvent::Log(format!(
                "Analyzing chunk {}/{} (start {}s)",
                i + 1,
                total,
                vc.start_seconds
            )));

            // Verify chunk file exists before analyzing
            if !Path::new(&vc.file_path).exists() {
                tracing::error!("chunk file missing {}", vc.file_path);
                anyhow::bail!("chunk file missing for analysis: {}", vc.file_path);
            }
            tracing::info!(
                "analyze_chunk start idx={} file={} model={}",
                i,
                vc.file_path,
                cfg.ai.resolved_model()
            );
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<crate::ai::ProviderEvent>();
            let chunk_start = std::time::Instant::now();
            // Run provider future concurrently with log/progress forwarding so the UI
            // sees `Waiting for file ACTIVE (poll N)` etc. in real time instead of
            // buffering until the chunk finishes (previously caused 60s of silence
            // before the timeout error appeared).
            let analyze_fut = provider.analyze_chunk(crate::ai::AnalyzeCtx {
                chunk_path: PathBuf::from(&vc.file_path),
                chunk_start_offset: std::time::Duration::from_secs(vc.start_seconds),
                cancellation: ctx.cancellation.clone(),
                progress_tx: Some(tx),
            });
            tokio::pin!(analyze_fut);
            let chunk_res: anyhow::Result<Vec<crate::types::VideoMoment>> = loop {
                tokio::select! {
                    res = &mut analyze_fut => break res,
                    ev = rx.recv() => {
                        match ev {
                            Some(crate::ai::ProviderEvent::Log(msg)) => on_event(PipelineEvent::Log(msg)),
                            Some(crate::ai::ProviderEvent::Progress(p)) => {
                                let blended = (i as f32 + p.clamp(0.0, 1.0)) / total as f32;
                                on_event(PipelineEvent::Progress(
                                    blended,
                                    format!("AI {:.0}%", p * 100.0),
                                ));
                            }
                            None => continue,
                        }
                    }
                }
            };
            // Drain any trailing events buffered after the future completed
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    crate::ai::ProviderEvent::Log(msg) => on_event(PipelineEvent::Log(msg)),
                    crate::ai::ProviderEvent::Progress(p) => {
                        let blended = (i as f32 + p.clamp(0.0, 1.0)) / total as f32;
                        on_event(PipelineEvent::Progress(
                            blended,
                            format!("AI {:.0}%", p * 100.0),
                        ));
                    }
                }
            }
            let chunk_moments = match chunk_res {
                Ok(m) => {
                    tracing::info!(
                        "chunk {} success {} moments in {:?}",
                        i,
                        m.len(),
                        chunk_start.elapsed()
                    );
                    m
                }
                Err(e) => {
                    tracing::error!("chunk {} failed file={} err={:#}", i, vc.file_path, e);
                    return Err(e.context(format!("chunk {} analyze failed", i)));
                }
            };

            tracing::info!(
                "chunk {} found {} moments, persisting",
                i + 1,
                chunk_moments.len()
            );
            on_event(PipelineEvent::Log(format!(
                "Chunk {}: found {} moments",
                i + 1,
                chunk_moments.len()
            )));
            // Persist incrementally per chunk
            let conn = session::init_db(&session::db_path()?)?;
            session::append_job_moments(&conn, &video_id, &chunk_moments)
                .inspect_err(|e| tracing::error!("append moments failed {e:#}"))?;
            session::update_job_chunk_status(&conn, &video_id, i as i64, "analyzed")
                .inspect_err(|e| tracing::error!("update chunk status failed {e:#}"))?;
            // Update job progress
            let analyzed_now = session::get_job_chunk_status(&conn, &video_id)?
                .iter()
                .filter(|(_, s)| s == "analyzed")
                .count() as i64;
            if let Some(mut job) = session::get_video_job(&conn, &video_id)? {
                job.analyzed_chunks = analyzed_now;
                job.status = if analyzed_now as usize == total {
                    "analyzed".into()
                } else {
                    "analyzing".into()
                };
                job.work_dir = ctx.work_dir.to_string_lossy().to_string();
                session::upsert_video_job(&conn, &job)?;
            }
            drop(conn);
            analyzed_set.insert(i as i64);
            moments.extend(chunk_moments);

            let frac = (i + 1) as f32 / total as f32;
            on_event(PipelineEvent::Progress(
                frac,
                format!("Analyzing {}/{}", i + 1, total),
            ));

            if i + 1 < total && processing.chunk_delay_secs > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(processing.chunk_delay_secs))
                    .await;
            }
        }

        on_event(PipelineEvent::Progress(1.0, "Saving".into()));
        tracing::info!(
            "pipeline saving video_id={} total_moments={} total_chunks={}",
            video_id,
            moments.len(),
            chunks.len()
        );
        // Final job status update
        let conn = session::init_db(&session::db_path()?)?;
        if let Some(mut job) = session::get_video_job(&conn, &video_id)? {
            job.status = "done".into();
            job.duration = ffmpeg::get_duration(&video_path).unwrap_or(job.duration);
            session::upsert_video_job(&conn, &job)?;
            tracing::info!(
                "job done {} status={} duration={:.1}s work_dir={}",
                job.video_id,
                job.status,
                job.duration,
                job.work_dir
            );
        }
        drop(conn);
        on_event(PipelineEvent::Progress(1.0, "Saved to database".into()));

        if processing.auto_extract {
            on_event(PipelineEvent::StageChanged(Stage::Extracting));
            let struct_path = ctx.work_dir.join("structure.json");
            if !struct_path.exists() {
                on_event(PipelineEvent::Log(
                    "auto_extract skipped: no structure.json, configure Structure first".into(),
                ));
            } else {
                match crate::plano::schema::load_document(&struct_path.to_string_lossy()) {
                    Ok(doc) => match doc.validate_for_export() {
                        Ok(()) => {
                            let profile = crate::plano::export::ExportProfile::high_1080();
                            let struct_hash = crate::plano::export::structure_hash(&doc);
                            let profile_hash = profile.hash();
                            let slug = crate::util::slugify(&video_id);
                            let shorts_dir = ctx.output_dir.join(slug).join("shorts");
                            let _ = std::fs::create_dir_all(&shorts_dir);
                            for (idx, m) in moments.iter().enumerate() {
                                if ctx.cancellation.is_cancelled() {
                                    on_event(PipelineEvent::Log("auto_extract cancelled".into()));
                                    break;
                                }
                                let (Some(s), Some(e)) = (
                                    crate::types::parse_timestamp_to_seconds(&m.start_time),
                                    crate::types::parse_timestamp_to_seconds(&m.end_time),
                                ) else {
                                    on_event(PipelineEvent::Log(format!(
                                        "skip moment {idx}: bad timestamps"
                                    )));
                                    continue;
                                };
                                let out = shorts_dir.join(format!("short_{:03}.mp4", idx + 1));
                                match crate::plano::export::build_moment_command(
                                    &video_path,
                                    &doc,
                                    s,
                                    e,
                                    &profile,
                                    &out,
                                ) {
                                    Ok(cmd) => match crate::plano::export::run_render(&cmd) {
                                        Ok(()) => {
                                            on_event(PipelineEvent::Log(format!(
                                                "extracted short_{:03}.mp4",
                                                idx + 1
                                            )));
                                            if let Ok(db) = session::db_path() {
                                                if let Ok(conn) = session::init_db(&db) {
                                                    let _ = session::upsert_export(
                                                        &conn,
                                                        &session::ExportRecord {
                                                            video_id: video_id.clone(),
                                                            moment_idx: idx as i64,
                                                            output_path: out
                                                                .to_string_lossy()
                                                                .to_string(),
                                                            status: "completed".to_string(),
                                                            error: None,
                                                            structure_hash: struct_hash.clone(),
                                                            profile_hash: profile_hash.clone(),
                                                        },
                                                    );
                                                }
                                            }
                                        }
                                        Err(e) => on_event(PipelineEvent::Log(format!(
                                            "extract failed {idx}: {e:#}"
                                        ))),
                                    },
                                    Err(e) => on_event(PipelineEvent::Log(format!(
                                        "skip moment {idx}: {e:#}"
                                    ))),
                                }
                            }
                        }
                        Err(e) => on_event(PipelineEvent::Log(format!(
                            "auto_extract blocked: invalid structure: {e}"
                        ))),
                    },
                    Err(e) => on_event(PipelineEvent::Log(format!(
                        "auto_extract blocked: cannot load structure: {e:#}"
                    ))),
                }
            }
        }

        tracing::info!(
            "pipeline done video_id={} chunks={} moments={}",
            video_id,
            chunks.len(),
            moments.len()
        );
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
// Helpers â€” verification (folder + DB)
// -------------------------------------------------------------------------------------------------

fn is_download_verified(video_id: &str, path: &Path) -> bool {
    if !path.exists() {
        tracing::debug!("download verify miss: file not exists {}", path.display());
        return false;
    }
    if path.metadata().map(|m| m.len() < 1024).unwrap_or(true) {
        tracing::debug!("download verify miss: file too small {}", path.display());
        return false;
    }
    let dur = ffmpeg::get_duration(path).unwrap_or(0.0);
    if dur <= 0.1 {
        tracing::debug!(
            "download verify miss: bad duration {} dur={}",
            path.display(),
            dur
        );
        return false;
    }
    // Check DB flag
    match session::db_path()
        .and_then(|p| session::init_db(&p).map_err(|e| anyhow::anyhow!(e.to_string())))
    {
        Ok(db) => match session::get_video_job(&db, video_id) {
            Ok(Some(job)) => {
                if job.download_verified
                    && job.download_path.as_deref() == Some(path.to_string_lossy().as_ref())
                {
                    tracing::debug!("download verify hit db {}", video_id);
                    return true;
                }
                tracing::debug!(
                    "download verify miss db flag verified={} path={:?}",
                    job.download_verified,
                    job.download_path
                );
                false
            }
            Ok(None) => {
                tracing::debug!("download verify miss: no job for {}", video_id);
                false
            }
            Err(e) => {
                tracing::debug!("download verify miss db err {e:#}");
                false
            }
        },
        Err(e) => {
            tracing::debug!("download verify miss db_path err {e:#}");
            false
        }
    }
}

fn update_job_download(
    video_id: &str,
    path: &Path,
    verified: bool,
    duration: f64,
) -> anyhow::Result<()> {
    let conn = session::init_db(&session::db_path()?)?;
    if let Some(mut job) = session::get_video_job(&conn, video_id)? {
        job.download_path = Some(path.to_string_lossy().to_string());
        job.download_verified = verified;
        if duration > 0.0 {
            job.duration = duration;
        }
        job.status = if verified {
            "downloaded".into()
        } else {
            "downloading".into()
        };
        session::upsert_video_job(&conn, &job)?;
    }
    Ok(())
}

fn update_job_status(video_id: &str, status: &str, split_verified: bool) -> anyhow::Result<()> {
    let conn = session::init_db(&session::db_path()?)?;
    if let Some(mut job) = session::get_video_job(&conn, video_id)? {
        job.status = status.to_owned();
        if split_verified {
            job.split_verified = true;
        }
        session::upsert_video_job(&conn, &job)?;
    }
    Ok(())
}

fn is_split_verified(video_id: &str, work_dir: &Path, expected: &[(u64, u64)]) -> bool {
    if !work_dir.exists() {
        tracing::debug!(
            "split verify miss: work_dir not exists {}",
            work_dir.display()
        );
        return false;
    }
    let conn = match session::db_path()
        .and_then(|p| session::init_db(&p).map_err(|e| anyhow::anyhow!(e.to_string())))
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    let job = match session::get_video_job(&conn, video_id).ok().flatten() {
        Some(j) => j,
        None => return false,
    };
    if !job.split_verified || job.total_chunks as usize != expected.len() {
        return false;
    }
    let stored = match session::get_job_chunks(&conn, video_id) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if stored.len() != expected.len() {
        return false;
    }
    for (i, (exp_start, exp_dur)) in expected.iter().enumerate() {
        if i >= stored.len() {
            return false;
        }
        let c = &stored[i];
        if c.start_seconds != *exp_start {
            return false;
        }
        let path = Path::new(&c.file_path);
        if !path.exists() {
            return false;
        }
        // Verify chunk file has plausible duration
        if let Ok(d) = ffmpeg::get_duration(path) {
            if (d - *exp_dur as f64).abs() > 1.0 && d < 0.5 {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

async fn split_and_persist<F>(
    video_id: &str,
    video_path: &Path,
    ranges: &[(u64, u64)],
    work_dir: &Path,
    cancellation: &CancellationToken,
    on_event: &mut F,
) -> anyhow::Result<Vec<crate::types::VideoChunk>>
where
    F: FnMut(PipelineEvent) + Send + 'static,
{
    let chunks = split_with_logs(video_path, ranges, work_dir, cancellation, on_event).await?;
    let conn = session::init_db(&session::db_path()?)?;
    session::insert_job_chunks(&conn, video_id, &chunks)?;
    if let Some(mut job) = session::get_video_job(&conn, video_id)? {
        job.total_chunks = chunks.len() as i64;
        job.split_verified = true;
        job.status = "splitting".into();
        job.work_dir = work_dir.to_string_lossy().to_string();
        session::upsert_video_job(&conn, &job)?;
    }
    Ok(chunks)
}

fn get_analyzed_indices(video_id: &str) -> std::collections::HashSet<i64> {
    if let Ok(conn) = session::db_path()
        .and_then(|p| session::init_db(&p).map_err(|e| anyhow::anyhow!(e.to_string())))
    {
        if let Ok(status) = session::get_job_chunk_status(&conn, video_id) {
            return status
                .into_iter()
                .filter(|(_, s)| s == "analyzed")
                .map(|(idx, _)| idx)
                .collect();
        }
    }
    std::collections::HashSet::new()
}

// -------------------------------------------------------------------------------------------------
// Helpers â€” download / split
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
                    tokio::time::sleep(crate::util::exponential_backoff(attempt)).await;
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
        .arg("--newline")
        .arg("--progress")
        .arg("-o")
        .arg(output.to_string_lossy().into_owned());
    if low_res {
        cmd.arg("-f").arg(ytdlp::LOW_RES_FORMAT);
    } else {
        cmd.arg("-f").arg(ytdlp::HIGH_RES_FORMAT);
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
    let stderr = child
        .stderr
        .take()
        .map(|stream| BufReader::new(stream).lines());
    let stdout = child
        .stdout
        .take()
        .map(|stream| BufReader::new(stream).lines());
    let mut stderr_lines = stderr;
    let mut stdout_lines = stdout;
    let mut stderr_done = stderr_lines.is_none();
    let mut stdout_done = stdout_lines.is_none();
    let mut last_pct: Option<f32> = None;
    let mut last_log_at: Option<std::time::Instant> = None;
    let mut last_log_line = String::new();

    while !stderr_done || !stdout_done {
        tokio::select! {
            result = async {
                stderr_lines.as_mut().expect("stderr stream exists").next_line().await
            }, if !stderr_done => {
                match result.context("reading yt-dlp stderr")? {
                    Some(line) => emit_ytdlp_line(&line, &mut last_pct, &mut last_log_at, &mut last_log_line, on_event),
                    None => stderr_done = true,
                }
            }
            result = async {
                stdout_lines.as_mut().expect("stdout stream exists").next_line().await
            }, if !stdout_done => {
                match result.context("reading yt-dlp stdout")? {
                    Some(line) => emit_ytdlp_line(&line, &mut last_pct, &mut last_log_at, &mut last_log_line, on_event),
                    None => stdout_done = true,
                }
            }
            _ = cancellation.cancelled() => {
                let _ = child.kill().await;
                anyhow::bail!("cancelled");
            }
        }
    }
    let status = child.wait().await.context("waiting for yt-dlp")?;
    if !status.success() {
        anyhow::bail!("yt-dlp download failed (exit {status})");
    }
    Ok(())
}

fn emit_ytdlp_line<F>(
    line: &str,
    last_pct: &mut Option<f32>,
    last_log_at: &mut Option<std::time::Instant>,
    last_log_line: &mut String,
    on_event: &mut F,
) where
    F: FnMut(PipelineEvent),
{
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }
    let is_progress = trimmed.contains("[download]") && trimmed.contains('%');
    // Throttle high-frequency download progress lines to ~1 Hz to avoid flooding
    // Slint's invoke_from_event_loop + Text re-layout on every line. Low-res
    // downloads emit hundreds of `frag N/M` lines per second and freeze the UI.
    if is_progress {
        // Deduplicate identical consecutive lines (common with --newline)
        if trimmed == last_log_line {
            // Still allow progress bar to update even if log is suppressed
            if let Some(p) = parse_ytdlp_progress(trimmed) {
                let emit = last_pct.is_none_or(|prev| (p - prev).abs() >= 0.005 || p >= 1.0);
                if emit {
                    *last_pct = Some(p);
                    on_event(PipelineEvent::Progress(
                        p.clamp(0.0, 1.0),
                        format!("Downloading {:.0}%", p * 100.0),
                    ));
                }
            }
            return;
        }
        let now = std::time::Instant::now();
        let throttled = last_log_at.is_some_and(|t| now.duration_since(t).as_millis() < 1000);
        let is_final = trimmed.contains("100%");
        // Always update progress bar (throttled 0.5%) even when log is suppressed
        if let Some(p) = parse_ytdlp_progress(trimmed) {
            let emit = last_pct.is_none_or(|prev| (p - prev).abs() >= 0.005 || p >= 1.0);
            if emit {
                *last_pct = Some(p);
                on_event(PipelineEvent::Progress(
                    p.clamp(0.0, 1.0),
                    format!("Downloading {:.0}%", p * 100.0),
                ));
            }
        }
        // Throttle textual log to ~1 Hz â€” main cause of UI freeze:
        // each Log triggers invoke_from_event_loop + get_analysis_log() clone
        // + set_analysis_log() + Text re-layout (wrap) in app.slint:620.
        if throttled && !is_final {
            return;
        }
        *last_log_at = Some(now);
        *last_log_line = trimmed.to_owned();
        on_event(PipelineEvent::Log(format!("[yt-dlp] {trimmed}")));
        return;
    }
    // Non-progress lines (Destination, Merging, ERROR, etc.) always emitted
    *last_log_line = trimmed.to_owned();
    on_event(PipelineEvent::Log(format!("[yt-dlp] {trimmed}")));
    if let Some(p) = parse_ytdlp_progress(trimmed) {
        let emit = last_pct.is_none_or(|prev| (p - prev).abs() >= 0.005 || p >= 1.0);
        if emit {
            *last_pct = Some(p);
            on_event(PipelineEvent::Progress(
                p.clamp(0.0, 1.0),
                format!("Downloading {:.0}%", p * 100.0),
            ));
        }
    }
}

fn parse_ytdlp_progress(line: &str) -> Option<f32> {
    if !line.contains("[download]") {
        return None;
    }
    let pct_idx = line.find('%')?;
    let bytes = line.as_bytes();
    let mut start = pct_idx;
    while start > 0 {
        let c = bytes[start - 1] as char;
        if c.is_ascii_digit() || c == '.' {
            start -= 1;
        } else {
            break;
        }
    }
    if start >= pct_idx {
        return None;
    }
    let num_str = &line[start..pct_idx];
    let v: f32 = num_str.trim().parse().ok()?;
    if !(0.0..=100.0).contains(&v) {
        return None;
    }
    Some(v / 100.0)
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
        let size = std::fs::metadata(&chunk_path).map(|m| m.len()).unwrap_or(0);
        let dur_actual = ffmpeg::get_duration(&chunk_path).unwrap_or(0.0);
        tracing::info!(
            "chunk split ok {}/{} file={} dur={:.1}s size={}",
            i + 1,
            chunks.len(),
            chunk_path.display(),
            dur_actual,
            size
        );
        video_chunks.push(crate::types::VideoChunk {
            start_seconds: *start,
            file_path: chunk_path.to_string_lossy().to_string(),
        });
        let total = chunks.len() as f32;
        let done = (i + 1) as f32 / total;
        on_event(PipelineEvent::Progress(
            done,
            format!("Splitting {}/{}", i + 1, chunks.len()),
        ));
    }
    Ok(video_chunks)
}

fn check_cancelled(token: &CancellationToken) -> anyhow::Result<()> {
    if token.is_cancelled() {
        anyhow::bail!("cancelled");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: Option<f32>, b: Option<f32>) -> bool {
        match (a, b) {
            (Some(x), Some(y)) => (x - y).abs() < 1e-6,
            (None, None) => true,
            _ => false,
        }
    }

    #[test]
    fn test_parse_ytdlp_progress() {
        assert!(approx_eq(
            parse_ytdlp_progress("[download]  45.2% of 10.00MiB at 1.00MiB/s ETA 00:10"),
            Some(0.452)
        ));
        assert!(approx_eq(
            parse_ytdlp_progress("[download] 100% of 5.12MiB in 00:02"),
            Some(1.0)
        ));
        assert_eq!(
            parse_ytdlp_progress("[download] Destination: video.mp4"),
            None
        );
        assert_eq!(parse_ytdlp_progress("some other line 50%"), None);
        assert!(approx_eq(
            parse_ytdlp_progress("[download]   0.0%"),
            Some(0.0)
        ));
    }
}
