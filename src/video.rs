//! Video processing module for YT ShortMaker
//! Handles yt-dlp downloads, ffmpeg operations, and chunk management

use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::process::Command;
use tokio::time::Duration;

use crate::types::VideoChunk;
use regex::Regex;

/// Extract video ID from YouTube URL
pub fn extract_video_id(url: &str) -> Option<String> {
    let re = Regex::new(r"(?:v=|\/)([0-9A-Za-z_-]{11}).*").ok()?;
    re.captures(url)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
}

/// Check if required external dependencies are available
pub fn check_dependencies() -> Result<()> {
    let ffmpeg = std::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();
    let ytdlp = std::process::Command::new("yt-dlp")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();

    let mut missing = Vec::new();

    if ffmpeg.is_err() {
        missing.push("ffmpeg");
    }

    if ytdlp.is_err() {
        missing.push("yt-dlp");
    }

    if !missing.is_empty() {
        let os = std::env::consts::OS;
        let mut msg = format!(
            "Missing required dependencies: {}.\nPlease install them first.",
            missing.join(", ")
        );

        if os == "linux" {
            msg.push_str("\n\nOn Linux (Ubuntu/Debian), try:\n  sudo apt update && sudo apt install ffmpeg\n  sudo pip3 install -U yt-dlp");
        } else if os == "macos" {
            msg.push_str("\n\nOn macOS, try:\n  brew install ffmpeg\n  brew install yt-dlp");
        } else if os == "windows" {
            msg.push_str("\n\nOn Windows, ensure ffmpeg and yt-dlp are in your PATH.");
        }

        return Err(anyhow!(msg));
    }

    Ok(())
}

/// Helper to run a command with cancellation support
pub async fn run_command_with_cancellation(
    mut command: Command,
    cancellation_token: Arc<AtomicBool>,
) -> Result<std::process::Output> {
    command.kill_on_drop(true);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let child = command.spawn().context("Failed to spawn command")?;
    let output_future = child.wait_with_output();

    let cancellation_future = async {
        loop {
            if cancellation_token.load(Ordering::Relaxed) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };

    tokio::select! {
        result = output_future => {
            result.context("Failed to wait on child process")
        }
        _ = cancellation_future => {
            // Process will be killed because output_future is dropped and we set kill_on_drop(true)
            log::warn!("Command cancelled by user token. Dropping child process.");
            Err(anyhow!("Process cancelled by user"))
        }
    }
}

/// Get video duration in seconds using ffprobe
pub fn get_video_duration(file_path: &str) -> Result<u64> {
    // Keep synchronous for now as it's fast
    let output = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            file_path,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("Failed to run ffprobe")?;

    let duration_str = String::from_utf8_lossy(&output.stdout);
    let duration: f64 = duration_str
        .trim()
        .parse()
        .context("Failed to parse duration")?;

    Ok(duration as u64)
}

/// Get precise video duration in seconds (f64)
pub fn get_video_duration_precise(file_path: &str) -> Result<f64> {
    let output = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            file_path,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("Failed to run ffprobe")?;

    let duration_str = String::from_utf8_lossy(&output.stdout);
    let duration: f64 = duration_str
        .trim()
        .parse()
        .context("Failed to parse duration")?;

    Ok(duration)
}

/// Download low resolution video for analysis (silent mode)
pub async fn download_low_res(
    url: &str,
    output_path: &str,
    use_cookies: bool,
    cookies_path: &str,
    cancellation_token: Arc<AtomicBool>,
) -> Result<()> {
    let mut args = vec![
        "-f",
        "bestvideo[height<=360][ext=mp4]+bestaudio[ext=m4a]/best[height<=360][ext=mp4]/bestvideo[height<=360]+bestaudio/best[height<=360]/best",
        "--merge-output-format",
        "mp4",
        "--no-warnings",
        "--no-cache-dir",
        "--retries",
        "10",
        "--fragment-retries",
        "10",
        "--progress",
        "--newline",
        "--force-overwrites",
        "--no-part",
        "--no-continue",
    ];

    if use_cookies {
        args.push("--cookies");
        args.push(cookies_path);
    }

    args.push("-o");
    args.push(output_path);

    args.push(url);

    let mut command = Command::new("yt-dlp");
    command.args(&args);

    let output = run_command_with_cancellation(command, cancellation_token).await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        if log::log_enabled!(log::Level::Debug) {
            log::error!("yt-dlp failed to download low-res video");
            log::error!("Command: yt-dlp {}", args.join(" "));
            log::error!("Stdout: {}", stdout);
            log::error!("Stderr: {}", stderr);
        }

        return Err(anyhow!("yt-dlp failed: {}", stderr.trim()));
    }

    // Log output if debug is enabled (checked via log level)
    if log::log_enabled!(log::Level::Debug) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::debug!("yt-dlp stdout: {}", stdout);
        log::debug!("yt-dlp stderr: {}", stderr);
    }

    Ok(())
}

/// Download high resolution video for final extraction (silent mode)
pub async fn download_high_res(
    url: &str,
    output_path: &str,
    use_cookies: bool,
    cookies_path: &str,
    custom_format: Option<String>,
    cancellation_token: Arc<AtomicBool>,
) -> Result<()> {
    let default_format =
        "bestvideo[ext=mp4]+bestaudio[ext=m4a]/bestvideo+bestaudio/best".to_string();
    let format = custom_format.unwrap_or(default_format);

    let mut args = vec![
        "-f",
        &format,
        "--merge-output-format",
        "mp4",
        "--no-warnings",
        "--no-cache-dir",
        "--retries",
        "10",
        "--fragment-retries",
        "10",
        "--progress",
        "--newline",
        "--force-overwrites",
        "--no-part",
        "--no-continue",
    ];

    if use_cookies {
        args.push("--cookies");
        args.push(cookies_path);
    }

    args.push("-o");
    args.push(output_path);

    args.push(url);

    let mut attempt = 1;
    let max_retries = 3;

    loop {
        // Check cancellation before retry
        if cancellation_token.load(Ordering::Relaxed) {
            return Err(anyhow!("Process cancelled by user"));
        }

        let mut command = Command::new("yt-dlp");
        command.args(&args);

        // We use the helper, which also checks cancellation during run
        let result = run_command_with_cancellation(command, cancellation_token.clone()).await;

        match result {
            Ok(output) => {
                if output.status.success() {
                    return Ok(());
                }

                let stderr = String::from_utf8_lossy(&output.stderr);

                if attempt >= max_retries {
                    if log::log_enabled!(log::Level::Debug) {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        log::error!("yt-dlp final failure for high-res video");
                        log::error!("Command: yt-dlp {}", args.join(" "));
                        log::error!("Stdout: {}", stdout);
                        log::error!("Stderr: {}", stderr);
                    }

                    return Err(anyhow!(
                        "yt-dlp failed after {} attempts: {}",
                        max_retries,
                        stderr.trim()
                    ));
                }

                log::warn!("yt-dlp attempt {} failed: {}", attempt, stderr.trim());
            }
            Err(e) => {
                // If it was cancelled, return immediately
                if e.to_string().contains("cancelled") {
                    return Err(e);
                }
                // Otherwise treat as error (or retry fallback logic if we wanted)
                return Err(e);
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        attempt += 1;
    }
}

/// Calculate video chunks for processing
/// Logic: Split by 30 mins. If last chunk <= 45 mins, merge it.
pub fn calculate_chunks(total_duration_seconds: u64) -> Vec<(u64, u64)> {
    let chunk_size = 30 * 60; // 30 mins in seconds
    let max_last_chunk = 45 * 60; // 45 mins in seconds
    let mut chunks = Vec::new();
    let mut current_time = 0;

    while current_time < total_duration_seconds {
        let remaining = total_duration_seconds - current_time;

        if remaining <= max_last_chunk {
            // Keep the last part whole if it's within the buffer
            chunks.push((current_time, remaining));
            break;
        } else {
            // Standard split
            chunks.push((current_time, chunk_size));
            current_time += chunk_size;
        }
    }
    chunks
}

/// Split video into chunks using ffmpeg (silent mode)
pub async fn split_video(
    input_path: &str,
    output_dir: &str,
    chunks: &[(u64, u64)],
    cancellation_token: Arc<AtomicBool>,
) -> Result<Vec<VideoChunk>> {
    let mut video_chunks = Vec::new();

    // Ensure output directory exists
    std::fs::create_dir_all(output_dir)?;

    for (i, (start, duration)) in chunks.iter().enumerate() {
        // Check cancellation before each chunk
        if cancellation_token.load(Ordering::Relaxed) {
            return Err(anyhow!("Process cancelled by user"));
        }

        let chunk_path = format!("{}/chunk_{}.mp4", output_dir, i);

        let start_time = format_seconds_to_timestamp(*start);
        let duration_time = duration.to_string();

        let mut args = vec![
            "-hide_banner".to_string(),
            "-loglevel".to_string(),
            "error".to_string(),
            "-ss".to_string(),
            start_time.clone(),
            "-i".to_string(),
            input_path.to_string(),
            "-t".to_string(),
            duration_time.clone(),
        ];

        // Use CPU encoding
        args.extend_from_slice(&[
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            "superfast".to_string(),
            "-c:a".to_string(),
            "aac".to_string(),
        ]);

        args.push("-y".to_string());
        args.push(chunk_path.clone());

        let mut command = Command::new("ffmpeg");
        command.args(&args);

        let output = run_command_with_cancellation(command, cancellation_token.clone()).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "ffmpeg split failed for chunk {}: {}",
                i,
                stderr.trim()
            ));
        }

        video_chunks.push(VideoChunk {
            start_seconds: *start,
            file_path: chunk_path,
        });
    }

    Ok(video_chunks)
}

/// Extract a clip from source video (fast mode using stream copy)
pub async fn extract_clip(
    source_path: &str,
    start_time: &str,
    end_time: &str,
    output_path: &str,
    cancellation_token: Arc<AtomicBool>,
) -> Result<()> {
    if cancellation_token.load(Ordering::Relaxed) {
        return Err(anyhow!("Process cancelled by user"));
    }

    // Calculate duration for -t argument
    let start_sec = parse_timestamp_to_seconds(start_time).context("Failed to parse start time")?;
    let end_sec = parse_timestamp_to_seconds(end_time).context("Failed to parse end time")?;

    if end_sec <= start_sec {
        return Err(anyhow!("End time must be greater than start time"));
    }

    let duration = end_sec - start_sec;

    let mut args = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-ss".to_string(),
        start_time.to_string(),
        "-i".to_string(),
        source_path.to_string(),
        "-t".to_string(),
        duration.to_string(),
    ];

    // Always use CPU
    args.extend_from_slice(&[
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "ultrafast".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
    ]);

    args.push("-y".to_string());
    args.push(output_path.to_string());

    let mut command = Command::new("ffmpeg");
    command.args(&args);

    let output = run_command_with_cancellation(command, cancellation_token).await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("ffmpeg extraction failed: {}", stderr.trim()));
    }

    Ok(())
}

/// Format seconds to HH:MM:SS timestamp
pub fn format_seconds_to_timestamp(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, secs)
}

/// Parse HH:MM:SS timestamp to seconds
pub fn parse_timestamp_to_seconds(timestamp: &str) -> Result<u64> {
    let parts: Vec<&str> = timestamp.split(':').collect();
    if parts.len() != 3 {
        return Err(anyhow!("Invalid timestamp format: {}", timestamp));
    }

    let hours: u64 = parts[0].parse().context("Invalid hours")?;
    let minutes: u64 = parts[1].parse().context("Invalid minutes")?;
    let seconds: u64 = parts[2].parse().context("Invalid seconds")?;

    Ok(hours * 3600 + minutes * 60 + seconds)
}

/// Validate Media URL
pub fn validate_media_url(url: &str) -> bool {
    let url_lower = url.to_lowercase();
    url_lower.starts_with("http://") || url_lower.starts_with("https://")
}

/// Clean up temporary files
#[allow(dead_code)]
pub fn cleanup_temp_files(paths: &[&str]) -> Result<()> {
    for path in paths {
        if Path::new(path).exists() {
            std::fs::remove_file(path).ok();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_chunks_short_video() {
        // 20 minutes video - should be one chunk
        let chunks = calculate_chunks(20 * 60);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], (0, 20 * 60));
    }

    #[test]
    fn test_calculate_chunks_long_video() {
        // 90 minutes video - should be 3 chunks (30, 30, 30)
        let chunks = calculate_chunks(90 * 60);
        assert_eq!(chunks.len(), 3);
    }

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_seconds_to_timestamp(3661), "01:01:01");
        assert_eq!(format_seconds_to_timestamp(0), "00:00:00");
    }

    #[test]
    fn test_parse_timestamp() {
        assert_eq!(parse_timestamp_to_seconds("01:01:01").unwrap(), 3661);
        assert_eq!(parse_timestamp_to_seconds("00:00:00").unwrap(), 0);
    }

    #[test]
    fn test_validate_media_url() {
        assert!(validate_media_url("https://www.youtube.com/watch?v=abc123"));
        assert!(validate_media_url("https://youtu.be/abc123"));
        assert!(validate_media_url("https://vimeo.com/video"));
        assert!(validate_media_url("http://example.com/video.mp4"));

        // Negative cases
        assert!(!validate_media_url("not_a_url"));
        assert!(!validate_media_url("ftp://server/file.mp4"));
        assert!(!validate_media_url("file:///local/path"));
    }
}
