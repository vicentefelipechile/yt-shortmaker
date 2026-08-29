// =================================================================================================
// cli::main — Headless CLI (preview/transform/batch/analyze)
// =================================================================================================

use clap::{Parser, Subcommand};

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "yt-shortmaker-cli", version, about = "Headless CLI for yt-shortmaker v2")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate a preview image from a video + plano.
    Preview {
        video: String,
        #[arg(long)]
        plano: Option<String>,
    },
    /// Transform a single video via plano.
    Transform {
        video: String,
        out: Option<String>,
        #[arg(long)]
        plano: Option<String>,
    },
    /// Batch transform all clips in a directory.
    Batch {
        input_dir: String,
        output_dir: Option<String>,
        #[arg(long)]
        plano: Option<String>,
    },
    /// Analyze a URL via configured AI provider.
    Analyze {
        url: String,
        #[arg(long, default_value = "gemini")]
        provider: String,
    },
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Preview { video, plano } => {
            println!("preview: video={video} plano={plano:?} (not yet implemented)");
        }
        Commands::Transform { video, out, plano } => {
            println!("transform: video={video} out={out:?} plano={plano:?} (not yet implemented)");
        }
        Commands::Batch {
            input_dir,
            output_dir,
            plano,
        } => {
            println!("batch: in={input_dir} out={output_dir:?} plano={plano:?} (not yet implemented)");
        }
        Commands::Analyze { url, provider } => {
            println!("analyze: url={url} provider={provider} (not yet implemented)");
        }
    }

    Ok(())
}
