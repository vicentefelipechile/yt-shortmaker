// =================================================================================================
// media::ytdlp — URL validation and download helpers
// =================================================================================================

use anyhow::{anyhow, Result};
use regex::Regex;

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

pub fn extract_video_id(url: &str) -> Option<String> {
    let re = Regex::new(r"(?:v=|/)([0-9A-Za-z_-]{11}).*").ok()?;
    re.captures(url)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().to_string()))
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

pub fn is_ytdlp_available() -> bool {
    std::process::Command::new("yt-dlp")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
        || std::process::Command::new("yt-dlp.exe")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
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
        assert_eq!(extract_video_id("https://youtu.be/dQw4w9WgXcQ"), Some("dQw4w9WgXcQ".to_string()));
        assert_eq!(extract_video_id("not a url"), None);
    }

    #[test]
    fn test_validate() {
        assert!(validate_media_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ").is_ok());
        assert!(validate_media_url("").is_err());
    }
}
