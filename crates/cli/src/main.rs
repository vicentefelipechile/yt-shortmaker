// =================================================================================================
// cli::main â€” Headless CLI (preview/transform/batch/analyze)
// =================================================================================================

use std::path::Path;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use tokio_util::sync::CancellationToken;
use yt_shortmaker_core::config::store as config_store;
use yt_shortmaker_core::media::pipeline::{Pipeline, PipelineCtx, PipelineEvent};
use yt_shortmaker_core::plano::schema::{create_default_document, load_document};

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(
    name = "yt-shortmaker-cli",
    version,
    about = "Headless CLI for yt-shortmaker v2"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Preview {
        video: String,
        #[arg(long)]
        plano: Option<String>,
        #[arg(long)]
        output: Option<String>,
    },
    Transform {
        video: String,
        out: Option<String>,
        #[arg(long)]
        plano: Option<String>,
    },
    Batch {
        input_dir: String,
        output_dir: Option<String>,
        #[arg(long)]
        plano: Option<String>,
    },
    Analyze {
        url: String,
        #[arg(long, default_value = "gemini")]
        provider: String,
    },
}

// -------------------------------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------------------------------

fn resolve_document(
    path: Option<String>,
) -> anyhow::Result<yt_shortmaker_core::plano::schema::PlanoDocument> {
    if let Some(p) = path {
        load_document(&p)
    } else {
        Ok(create_default_document())
    }
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_ansi(false).init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Preview {
            video,
            plano,
            output,
        } => {
            let doc = resolve_document(plano)?;
            doc.validate().map_err(|e| anyhow::anyhow!(e))?;
            let plano = doc.to_plano();
            let (filter, inputs) =
                yt_shortmaker_core::plano::compiler::build_ffmpeg_filter(&plano, &video);
            println!("inputs: {inputs:?}");
            println!("filter: {filter}");
            if let Some(out) = output {
                let cmd = yt_shortmaker_core::plano::export::build_frame_command(
                    Path::new(&video),
                    &doc,
                    doc.preview_second,
                    Path::new(&out),
                )?;
                yt_shortmaker_core::plano::export::run_render(&cmd)?;
                println!("Preview written to {out}");
            }
        }
        Commands::Transform { video, out, plano } => {
            let doc = resolve_document(plano)?;
            doc.validate_for_export().map_err(|e| anyhow::anyhow!(e))?;
            let out = out.unwrap_or_else(|| "output.mp4".into());
            let dur = yt_shortmaker_core::media::ffmpeg::get_duration(Path::new(&video))
                .unwrap_or(0.0) as u64;
            let dur = dur.max(1);
            let profile = yt_shortmaker_core::plano::export::ExportProfile::high_1080();
            let cmd = yt_shortmaker_core::plano::export::build_sample_command(
                Path::new(&video),
                &doc,
                0,
                dur.min(30),
                &profile,
                Path::new(&out),
            )?;
            yt_shortmaker_core::plano::export::run_render(&cmd)?;
            println!("Transform {video} -> {out}");
        }
        Commands::Batch {
            input_dir,
            output_dir,
            plano,
        } => {
            let doc = resolve_document(plano)?;
            doc.validate_for_export().map_err(|e| anyhow::anyhow!(e))?;
            let out = output_dir.unwrap_or_else(|| "./output".into());
            std::fs::create_dir_all(&out)?;
            let profile = yt_shortmaker_core::plano::export::ExportProfile::high_1080();
            let entries = std::fs::read_dir(&input_dir)?;
            let mut count = 0;
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "mp4" {
                        let out_path = Path::new(&out).join(format!("short_{count}.mp4"));
                        let dur = yt_shortmaker_core::media::ffmpeg::get_duration(&path)
                            .unwrap_or(0.0) as u64;
                        let dur = dur.clamp(1, 90);
                        // Batch reuses the shared sample/moment compiler with a 0-start window.
                        let cmd = yt_shortmaker_core::plano::export::build_sample_command(
                            &path,
                            &doc,
                            0,
                            dur.min(30),
                            &profile,
                            &out_path,
                        )?;
                        match yt_shortmaker_core::plano::export::run_render(&cmd) {
                            Ok(()) => println!("{} -> {} ok", path.display(), out_path.display()),
                            Err(e) => eprintln!("{} failed: {e:#}", path.display()),
                        }
                        count += 1;
                    }
                }
            }
            println!("Batch: {count} clips rendered");
        }
        Commands::Analyze { url, provider } => {
            yt_shortmaker_core::media::ytdlp::validate_media_url(&url)?;
            let mut cfg = config_store::load().unwrap_or_default();
            if provider != "gemini" {
                cfg.ai.active_provider = provider.clone();
            }
            let vid = yt_shortmaker_core::session::resolve_stable_id(&url);
            let work_dir = yt_shortmaker_core::session::video_work_dir_or_tmp(&vid);
            let output_dir = cfg
                .output_dir
                .clone()
                .unwrap_or_else(|| "output".to_string());
            let pipeline = Pipeline::new();
            let out = pipeline
                .run(
                    PipelineCtx {
                        url,
                        config: Arc::new(cfg),
                        work_dir,
                        output_dir: Path::new(&output_dir).to_path_buf(),
                        cancellation: CancellationToken::new(),
                    },
                    |ev| match ev {
                        PipelineEvent::StageChanged(stage) => {
                            eprintln!("[stage] {stage:?}");
                        }
                        PipelineEvent::Progress(p, msg) => {
                            eprintln!("[{:.0}%] {msg}", p * 100.0);
                        }
                        PipelineEvent::Log(msg) => eprintln!("[log] {msg}"),
                    },
                )
                .await?;
            println!("{}", serde_json::to_string_pretty(&out.moments)?);
            println!("---");
            println!("video: {}", out.video_id);
            println!("chunks: {}", out.chunks.len());
            println!("output: {}", out.video_path.display());
        }
    }

    Ok(())
}
