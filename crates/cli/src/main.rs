// =================================================================================================
// cli::main — Headless CLI (preview/transform/batch/analyze)
// =================================================================================================

use std::path::Path;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use tokio_util::sync::CancellationToken;
use yt_shortmaker_core::config::store as config_store;
use yt_shortmaker_core::media::pipeline::{Pipeline, PipelineCtx, PipelineEvent};
use yt_shortmaker_core::plano::schema::{create_default_plano, load_plano};

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

fn resolve_plano(
    path: Option<String>,
) -> anyhow::Result<Vec<yt_shortmaker_core::plano::schema::PlanoObject>> {
    if let Some(p) = path {
        load_plano(&p)
    } else {
        Ok(create_default_plano())
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
            let plano = resolve_plano(plano)?;
            let (filter, inputs) =
                yt_shortmaker_core::plano::compiler::build_ffmpeg_filter(&plano, &video);
            println!("inputs: {inputs:?}");
            println!("filter: {filter}");
            if let Some(out) = output {
                println!("Would generate preview at {out} (requires ffmpeg)");
            }
        }
        Commands::Transform { video, out, plano } => {
            let plano = resolve_plano(plano)?;
            let out = out.unwrap_or_else(|| "output.mp4".into());
            let (filter, inputs) =
                yt_shortmaker_core::plano::compiler::build_ffmpeg_filter(&plano, &video);
            println!("Transform {video} -> {out}");
            println!("inputs: {inputs:?}");
            println!("filter: {filter}");
        }
        Commands::Batch {
            input_dir,
            output_dir,
            plano,
        } => {
            let plano = resolve_plano(plano)?;
            let out = output_dir.unwrap_or_else(|| "./output".into());
            let entries = std::fs::read_dir(&input_dir)?;
            let mut count = 0;
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "mp4" {
                        let out_path = Path::new(&out).join(format!("short_{count}.mp4"));
                        let (filter, _) = yt_shortmaker_core::plano::compiler::build_ffmpeg_filter(
                            &plano,
                            &path.to_string_lossy(),
                        );
                        println!(
                            "{} -> {} | filter len {}",
                            path.display(),
                            out_path.display(),
                            filter.len()
                        );
                        count += 1;
                    }
                }
            }
            println!("Batch: {count} clips queued");
        }
        Commands::Analyze { url, provider } => {
            yt_shortmaker_core::media::ytdlp::validate_media_url(&url)?;
            let mut cfg = config_store::load().unwrap_or_default();
            if provider != "gemini" {
                cfg.ai.active_provider = provider.clone();
            }
            let vid = yt_shortmaker_core::media::ytdlp::extract_video_id(&url)
                .unwrap_or_else(|| "video".into());
            let work_dir = yt_shortmaker_core::session::video_work_dir(&vid)
                .unwrap_or_else(|_| std::env::temp_dir().join("yt-shortmaker-v2").join(&vid));
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
