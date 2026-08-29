// =================================================================================================
// media::chunk — Chunk planning and video splitting
// =================================================================================================

use std::path::Path;

use anyhow::{Context, Result};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::types::VideoChunk;

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

/// Calculates chunk ranges `(start_secs, duration_secs)` for a video.
///
/// Splits by `chunk_size_secs`. If the remaining tail is `<= max_last_chunk_secs`
/// it becomes a standalone final chunk (v1 code semantics, now configurable).
pub fn calculate_chunks(
    total_duration_secs: u64,
    chunk_size_secs: u64,
    max_last_chunk_secs: u64,
) -> Vec<(u64, u64)> {
    if total_duration_secs == 0 {
        return Vec::new();
    }
    let mut chunks: Vec<(u64, u64)> = Vec::new();
    let mut current = 0u64;
    while current < total_duration_secs {
        let remaining = total_duration_secs - current;
        if remaining <= max_last_chunk_secs {
            chunks.push((current, remaining));
            break;
        }
        chunks.push((current, chunk_size_secs));
        current += chunk_size_secs;
    }
    chunks
}

/// Splits `input` into chunks using ffmpeg (`-ss`/`-t`, stream copy), writing
/// `chunk_{idx}.mp4` into `output_dir`. Checks cancellation before each chunk.
pub async fn split_video(
    input: &Path,
    chunks: &[(u64, u64)],
    output_dir: &Path,
    cancellation: &CancellationToken,
) -> Result<Vec<VideoChunk>> {
    std::fs::create_dir_all(output_dir).context("creating chunks dir")?;
    let mut video_chunks = Vec::new();

    for (i, (start, duration)) in chunks.iter().enumerate() {
        if cancellation.is_cancelled() {
            anyhow::bail!("cancelled");
        }
        let chunk_path = output_dir.join(format!("chunk_{i}.mp4"));
        let output = Command::new(crate::setup::ffmpeg_bin())
            .args([
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
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    crate::setup::ffmpeg_missing_error()
                } else {
                    anyhow::Error::from(e).context("failed to run ffmpeg")
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr.trim();
            if detail.is_empty() {
                anyhow::bail!("ffmpeg split failed for chunk {i} (exit {})", output.status);
            } else {
                anyhow::bail!(
                    "ffmpeg split failed for chunk {i} (exit {}): {}",
                    output.status,
                    detail
                );
            }
        }

        video_chunks.push(VideoChunk {
            start_seconds: *start,
            file_path: chunk_path.to_string_lossy().to_string(),
        });
    }

    Ok(video_chunks)
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> (u64, u64) {
        (10 * 60, 15 * 60)
    }

    #[test]
    fn test_empty_video() {
        let (size, max_last) = cfg();
        assert!(calculate_chunks(0, size, max_last).is_empty());
    }

    #[test]
    fn test_short_video_single_chunk() {
        // 20 minutes, 30-min chunks, tail <=45 min -> one chunk
        let chunks = calculate_chunks(20 * 60, 30 * 60, 45 * 60);
        assert_eq!(chunks, vec![(0, 20 * 60)]);
    }

    #[test]
    fn test_90_min_video_three_chunks() {
        // 90 minutes, 30-min chunks -> (0,1800),(1800,1800),(3600,1800)
        let chunks = calculate_chunks(90 * 60, 30 * 60, 45 * 60);
        assert_eq!(chunks, vec![(0, 1800), (1800, 1800), (3600, 1800)]);
    }

    #[test]
    fn test_exact_multiples() {
        // 30 minutes, 10-min chunks -> three 10-min chunks
        let chunks = calculate_chunks(30 * 60, 10 * 60, 15 * 60);
        assert_eq!(chunks, vec![(0, 600), (600, 600), (1200, 600)]);
    }

    #[test]
    fn test_custom_sizes() {
        // 25 min video, 5 min chunks, tail <= 2 min -> five 5-min chunks
        let chunks = calculate_chunks(25 * 60, 5 * 60, 2 * 60);
        assert_eq!(
            chunks,
            vec![(0, 300), (300, 300), (600, 300), (900, 300), (1200, 300)]
        );
    }

    #[test]
    fn test_small_tail_standalone() {
        // 22 min, 10 min chunks, tail <= 3 min -> tail chunk stays standalone
        let chunks = calculate_chunks(22 * 60, 10 * 60, 3 * 60);
        assert_eq!(chunks, vec![(0, 600), (600, 600), (1200, 120)]);
    }
}
