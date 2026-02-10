//! Módulo de compresión de video para YT ShortMaker
//! Optimiza chunks de video para ser más eficientes en el análisis con IA.
//! Este módulo implementa el pipeline alternativo donde se descarga en alta calidad
//! y luego se comprimen los chunks para Gemini.

use anyhow::{anyhow, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::process::Command;

use crate::types::CompressionSettings;

/// Comprime un chunk de video para optimizar el análisis con IA.
/// Reduce resolución, aplica CRF agresivo y usa audio de baja calidad
/// ya que el propósito es solo el análisis, no la calidad final.
pub async fn compress_chunk(
    input_path: &str,
    output_path: &str,
    settings: &CompressionSettings,
    cancellation_token: Arc<AtomicBool>,
) -> Result<()> {
    if cancellation_token.load(Ordering::Relaxed) {
        return Err(anyhow!("Process cancelled by user"));
    }

    let crf = settings.crf.to_string();
    let resolution = format!("-2:{}", settings.target_resolution);
    let scale_filter = format!("scale={}", resolution);

    let args = vec![
        "-hide_banner",
        "-loglevel",
        "error",
        "-i",
        input_path,
        "-vf",
        &scale_filter,
        "-c:v",
        "libx264",
        "-preset",
        &settings.preset,
        "-crf",
        &crf,
        "-c:a",
        "aac",
        "-b:a",
        &settings.audio_bitrate,
        "-ac",
        "1", // Mono para reducir tamaño
        "-g",
        "48", // Keyframe interval (2 seg a 24fps)
        "-y",
        output_path,
    ];

    let mut command = Command::new("ffmpeg");
    command.args(&args);

    let output = crate::video::run_command_with_cancellation(command, cancellation_token).await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Chunk compression failed: {}", stderr.trim()));
    }

    Ok(())
}

/// Divide un video en chunks con compresión aplicada durante el split.
/// Combina split + compresión en un solo paso FFmpeg para mayor eficiencia.
pub async fn split_and_compress(
    input_path: &str,
    output_dir: &str,
    chunks: &[(u64, u64)],
    settings: &CompressionSettings,
    cancellation_token: Arc<AtomicBool>,
) -> Result<Vec<crate::types::VideoChunk>> {
    let mut video_chunks = Vec::new();

    std::fs::create_dir_all(output_dir)?;

    let crf = settings.crf.to_string();
    let resolution = format!("-2:{}", settings.target_resolution);

    for (i, (start, duration)) in chunks.iter().enumerate() {
        if cancellation_token.load(Ordering::Relaxed) {
            return Err(anyhow!("Process cancelled by user"));
        }

        let chunk_path = format!("{}/chunk_{}.mp4", output_dir, i);

        let start_time = crate::video::format_seconds_to_timestamp(*start);
        let duration_time = duration.to_string();

        let args = vec![
            "-hide_banner".to_string(),
            "-loglevel".to_string(),
            "error".to_string(),
            "-ss".to_string(),
            start_time,
            "-i".to_string(),
            input_path.to_string(),
            "-t".to_string(),
            duration_time,
            "-vf".to_string(),
            format!("scale={}", resolution),
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            settings.preset.clone(),
            "-crf".to_string(),
            crf.clone(),
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            settings.audio_bitrate.clone(),
            "-ac".to_string(),
            "1".to_string(),
            "-g".to_string(),
            "48".to_string(),
            "-y".to_string(),
            chunk_path.clone(),
        ];

        let mut command = Command::new("ffmpeg");
        command.args(&args);

        let output =
            crate::video::run_command_with_cancellation(command, cancellation_token.clone())
                .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "Compressed split failed for chunk {}: {}",
                i,
                stderr.trim()
            ));
        }

        video_chunks.push(crate::types::VideoChunk {
            start_seconds: *start,
            file_path: chunk_path,
        });
    }

    Ok(video_chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_settings_default() {
        let settings = CompressionSettings::default();
        assert_eq!(settings.target_resolution, 720);
        assert_eq!(settings.crf, 28);
        assert_eq!(settings.audio_bitrate, "64k");
        assert_eq!(settings.preset, "fast");
    }
}
