// =================================================================================================
// setup — Dependency checks and tool provisioning
// =================================================================================================

use anyhow::{anyhow, Result};

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

pub fn check_dependencies() -> Result<()> {
    let mut missing = Vec::new();

    if !is_available("ffmpeg", &["-version"]) {
        missing.push("ffmpeg");
    }
    if !is_available("yt-dlp", &["--version"]) && !is_available("yt-dlp.exe", &["--version"]) {
        missing.push("yt-dlp");
    }

    if missing.is_empty() {
        return Ok(());
    }

    let os = std::env::consts::OS;
    let mut msg = format!("Missing dependencies: {}.", missing.join(", "));
    match os {
        "linux" => msg.push_str(" Install with: sudo apt update && sudo apt install ffmpeg; pip install -U yt-dlp"),
        "windows" => msg.push_str(" Ensure ffmpeg and yt-dlp are in PATH or use auto-download."),
        "macos" => msg.push_str(" Install with: brew install ffmpeg yt-dlp"),
        _ => {}
    }
    Err(anyhow!(msg))
}

fn is_available(bin: &str, args: &[&str]) -> bool {
    std::process::Command::new(bin)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .is_ok_and(|o| o.status.success())
}
