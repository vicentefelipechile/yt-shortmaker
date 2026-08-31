// =================================================================================================
// media::ytdlp — URL validation, info fetching, and downloads
// =================================================================================================

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

pub const LOW_RES_FORMAT: &str = "worst[ext=mp4]/worst";
pub const HIGH_RES_FORMAT: &str = "bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best";

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub video_id: String,
    pub title: String,
    pub duration_secs: f64,
    pub thumbnail: Option<String>,
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

pub fn extract_video_id(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    // 1. YouTube 11-char id (v= or /)
    if let Ok(re) = Regex::new(r"(?:v=|/)([0-9A-Za-z_-]{11})(?:[&#?/]|$)") {
        if let Some(cap) = re.captures(trimmed) {
            if let Some(m) = cap.get(1) {
                return Some(m.as_str().to_string());
            }
        }
    }
    // 2. Twitch VOD: https://www.twitch.tv/videos/2859440809 -> v2859440809 (matches yt-dlp id)
    if let Ok(re) = Regex::new(r"twitch\.tv/videos/(\d{6,})") {
        if let Some(cap) = re.captures(trimmed) {
            if let Some(m) = cap.get(1) {
                // yt-dlp prefixes Twitch VODs with 'v'
                return Some(format!("v{}", m.as_str()));
            }
        }
    }
    // 3. Generic fallback only for http(s) URLs — "not a url" must stay None for validate tests
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return None;
    }
    // Last path segment (e.g. https://example.com/media/abc123?x=1 -> abc123)
    // Extract last non-empty segment before query/fragment
    let without_query = trimmed
        .split('?')
        .next()
        .unwrap_or(trimmed)
        .split('#')
        .next()
        .unwrap_or(trimmed);
    let without_trailing = without_query.trim_end_matches('/');
    if let Some(last) = without_trailing.rsplit('/').next() {
        // Strip extension if any (e.g. video.mp4)
        let candidate = last.split('.').next().unwrap_or(last);
        // Must be plausible id length and charset
        if candidate.len() >= 4
            && candidate.len() <= 64
            && candidate
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            // Avoid returning generic words like "videos" / "watch"
            let lower = candidate.to_ascii_lowercase();
            if lower != "videos"
                && lower != "watch"
                && lower != "video"
                && lower != "embed"
                && lower != "shorts"
                && lower != "live"
            {
                return Some(candidate.to_string());
            }
        }
    }
    // 4. Deterministic hash fallback (FNV-1a 64) -> url_{hex}, stable across runs
    // This is the crucial fix for the "always from 0" bug: previously used chrono timestamp
    // which generated a new video_id per run, breaking folder+DB verification.
    Some(format!("url_{:016x}", fnv1a_hash(trimmed)))
}

pub fn fnv1a_hash(s: &str) -> u64 {
    crate::util::fnv1a_hash(s)
}

pub fn hash_url(url: &str) -> String {
    crate::util::hash_url(url)
}

pub fn validate_media_url(url: &str) -> Result<()> {
    if url.trim().is_empty() {
        anyhow::bail!("URL is empty");
    }
    if extract_video_id(url).is_some() {
        return Ok(());
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        return Ok(());
    }
    Err(anyhow!("Invalid media URL"))
}

/// Fetches metadata (title/duration/thumbnail) via `yt-dlp --dump-json`.
pub async fn fetch_info(
    url: &str,
    use_cookies: bool,
    cookies_path: Option<&str>,
    retry_attempts: u32,
    cancellation: &CancellationToken,
) -> Result<VideoInfo> {
    let mut last_err = None;
    for attempt in 0..retry_attempts.max(1) {
        if cancellation.is_cancelled() {
            anyhow::bail!("cancelled");
        }
        match fetch_info_once(url, use_cookies, cookies_path).await {
            Ok(info) => return Ok(info),
            Err(e) => {
                last_err = Some(e);
                if attempt + 1 < retry_attempts.max(1) {
                    tokio::time::sleep(crate::util::exponential_backoff(attempt)).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("fetch_info failed")))
}

async fn fetch_info_once(
    url: &str,
    use_cookies: bool,
    cookies_path: Option<&str>,
) -> Result<VideoInfo> {
    let mut cmd = Command::new(crate::setup::yt_dlp_bin());
    cmd.arg("--dump-json").arg("--no-playlist");
    if use_cookies {
        if let Some(p) = cookies_path.filter(|p| !p.is_empty()) {
            cmd.arg("--cookies").arg(p);
        }
    }
    cmd.arg(url);

    let output = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                crate::setup::yt_dlp_missing_error()
            } else {
                anyhow::Error::from(e).context("failed to run yt-dlp")
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_trim = stderr.trim();
        if stderr_trim.is_empty() {
            anyhow::bail!(
                "yt-dlp --dump-json failed (exit {}). URL may be unsupported or require cookies/auth. Ensure yt-dlp is up to date: yt-dlp --update",
                output.status
            );
        }
        anyhow::bail!("yt-dlp --dump-json failed: {}", stderr_trim);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).context("parsing yt-dlp --dump-json output")?;

    let video_id = json
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("yt-dlp output missing id"))?;
    let title = json
        .get("title")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| video_id.clone());
    let duration_secs = json.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let thumbnail = json
        .get("thumbnail")
        .and_then(|v| v.as_str())
        .map(str::to_owned);

    Ok(VideoInfo {
        video_id,
        title,
        duration_secs,
        thumbnail,
    })
}

/// Downloads a video with yt-dlp (best format by default, low-res when
/// `low_res` is set). Retries with exponential backoff.
pub async fn download_video(
    url: &str,
    output: &Path,
    use_cookies: bool,
    cookies_path: Option<&str>,
    low_res: bool,
    retry_attempts: u32,
    cancellation: &CancellationToken,
) -> Result<()> {
    let mut last_err = None;
    for attempt in 0..retry_attempts.max(1) {
        if cancellation.is_cancelled() {
            anyhow::bail!("cancelled");
        }
        match download_video_once(url, output, use_cookies, cookies_path, low_res).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                if attempt + 1 < retry_attempts.max(1) {
                    tokio::time::sleep(crate::util::exponential_backoff(attempt)).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("download failed")))
}

async fn download_video_once(
    url: &str,
    output: &Path,
    use_cookies: bool,
    cookies_path: Option<&str>,
    low_res: bool,
) -> Result<()> {
    let mut cmd = Command::new(crate::setup::yt_dlp_bin());
    cmd.arg("--no-playlist")
        .arg("-o")
        .arg(output.to_string_lossy().into_owned());
    if low_res {
        cmd.arg("-f").arg(LOW_RES_FORMAT);
    } else {
        cmd.arg("-f").arg(HIGH_RES_FORMAT);
        cmd.arg("--merge-output-format").arg("mp4");
    }
    if use_cookies {
        if let Some(p) = cookies_path.filter(|p| !p.is_empty()) {
            cmd.arg("--cookies").arg(p);
        }
    }
    cmd.arg(url);

    let child = cmd
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                crate::setup::yt_dlp_missing_error()
            } else {
                anyhow::Error::from(e).context("failed to run yt-dlp")
            }
        })?;

    let output = child
        .wait_with_output()
        .await
        .context("failed to wait for yt-dlp")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_trim = stderr.trim();
        if stderr_trim.is_empty() {
            anyhow::bail!(
                "yt-dlp download failed (exit {}). Check URL, cookies or update yt-dlp.",
                output.status
            );
        }
        anyhow::bail!(
            "yt-dlp download failed (exit {}): {}",
            output.status,
            stderr_trim
        );
    }
    Ok(())
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_id() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".to_string())
        );
        assert_eq!(
            extract_video_id("https://youtu.be/dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".to_string())
        );
        assert_eq!(extract_video_id("not a url"), None);
    }

    #[test]
    fn test_validate() {
        assert!(validate_media_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ").is_ok());
        assert!(validate_media_url("").is_err());
        assert!(validate_media_url("not a url").is_err());
    }
}
