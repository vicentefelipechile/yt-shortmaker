// =================================================================================================
// media::ytdlp — yt-dlp helpers (stubs for M0)
// =================================================================================================

use anyhow::Result;

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

pub fn extract_video_id(url: &str) -> Option<String> {
    // Minimal stub: delegate to regex in M1, here naive.
    if url.contains("youtube.com") || url.contains("youtu.be") {
        return Some(url.to_owned());
    }
    None
}

pub async fn validate_media_url(_url: &str) -> Result<()> {
    anyhow::bail!("not implemented")
}
