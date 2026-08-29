// =================================================================================================
// media::ffmpeg — Duration, resolution, and helpers
// =================================================================================================

use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

pub fn get_duration(path: &Path) -> Result<f64> {
    let output = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            &path.to_string_lossy(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("Failed to run ffprobe")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ffprobe failed: {}", stderr.trim());
    }

    let s = String::from_utf8_lossy(&output.stdout);
    let v: f64 = s.trim().parse().context("Failed to parse duration")?;
    Ok(v)
}

pub fn get_duration_u64(path: &Path) -> Result<u64> {
    Ok(get_duration(path)? as u64)
}

pub fn get_resolution(path: &Path) -> Result<(u32, u32)> {
    let output = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0:s=x",
            &path.to_string_lossy(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("Failed to run ffprobe for resolution")?;

    let s = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = s.trim().split('x').collect();
    if parts.len() != 2 {
        anyhow::bail!("Failed to parse resolution: {}", s.trim());
    }
    let w: u32 = parts[0].parse().context("Invalid width")?;
    let h: u32 = parts[1].parse().context("Invalid height")?;
    Ok((w, h))
}

pub fn is_ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn is_ffprobe_available() -> bool {
    std::process::Command::new("ffprobe")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_file() {
        let p = Path::new("/nonexistent/file.mp4");
        assert!(get_duration(p).is_err());
        assert!(get_resolution(p).is_err());
    }
}
