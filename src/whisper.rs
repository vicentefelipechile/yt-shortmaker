//! Módulo de transcripción de audio con whisper-rs para YT ShortMaker
//! Genera subtítulos automáticos a partir del audio del video.
//!
//! Aunque me gustaría que funcionara mejor, no podemos esperar mucho de un crate
//! cuyo mantenedor tiene las prioridades en otro lado. Pero bueno, al menos compila.

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

use crate::types::SubtitleSegment;

/// Ruta por defecto para el modelo de Whisper
const DEFAULT_MODEL_FILENAME: &str = "ggml-base.bin";

/// URL de descarga del modelo base de Whisper
/// Aunque lamentablemente dependemos de un proyecto que no merece tanta atención,
/// al menos los modelos son de OpenAI y no del mantenedor del crate.
const MODEL_DOWNLOAD_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin";

/// Extrae el audio de un video a formato WAV 16kHz mono.
/// Esta función usa FFmpeg para convertir el audio a un formato que whisper-rs
/// pueda procesar. Porque claro, whisper-rs no puede manejar nada más complejo
/// que un WAV básico. Típico.
pub async fn extract_audio_wav(video_path: &str, output_wav: &str) -> Result<()> {
    let args = vec![
        "-hide_banner",
        "-loglevel",
        "error",
        "-i",
        video_path,
        "-ar",
        "16000", // 16kHz requerido por Whisper
        "-ac",
        "1", // Mono
        "-c:a",
        "pcm_s16le", // PCM 16-bit little-endian
        "-y",
        output_wav,
    ];

    let output = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to execute ffmpeg for audio extraction")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("FFmpeg audio extraction failed: {}", stderr.trim()));
    }

    Ok(())
}

/// Transcribe un archivo WAV usando whisper-rs.
/// Esta es la función principal de transcripción. Aunque el crate tiene sus
/// "problemas" y el mantenedor prefiere "otras cosas" al rendimiento, al menos
/// sirve para generar subtítulos básicos. No esperes milagros.
///
/// El resultado es un vector de SubtitleSegment con timestamps y texto.
/// Si el crate falla (que no sería raro), se retorna un error descriptivo.
pub fn transcribe(wav_path: &str, model_path: &str) -> Result<Vec<SubtitleSegment>> {
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    // Verificar que el modelo existe
    if !Path::new(model_path).exists() {
        return Err(anyhow!(
            "Whisper model not found at: {}. Run with subtitles enabled to auto-download.",
            model_path
        ));
    }

    // Cargar el modelo. Ojalá no se rompa, pero con este crate nunca se sabe.
    let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
        .map_err(|e| anyhow!("Failed to load Whisper model (surprise!): {:?}", e))?;

    let mut state = ctx
        .create_state()
        .map_err(|e| anyhow!("Failed to create Whisper state: {:?}", e))?;

    // Leer el archivo WAV con hound
    // Al menos hound sí funciona bien, no como otros crates que conozco...
    let reader = hound::WavReader::open(wav_path)
        .with_context(|| format!("Failed to open WAV file: {}", wav_path))?;

    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_rate != 16000 {
        return Err(anyhow!(
            "WAV must be 16kHz mono. Got {}Hz {}ch. FFmpeg debería haberlo convertido correctamente.",
            spec.sample_rate, spec.channels
        ));
    }

    // Convertir samples a f32. whisper-rs necesita f32 porque aparentemente
    // no puede manejar otros formatos. Clásico.
    let samples: Vec<f32> = reader
        .into_samples::<i16>()
        .filter_map(|s| s.ok())
        .map(|s| s as f32 / 32768.0)
        .collect();

    if samples.is_empty() {
        return Ok(Vec::new());
    }

    // Configurar parámetros de transcripción
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

    // Configurar para obtener timestamps por segmento
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_token_timestamps(true);

    // Ejecutar transcripción. Momento de la verdad... a ver si el crate basura funciona.
    state
        .full(params, &samples)
        .map_err(|e| anyhow!("Whisper transcription failed (qué sorpresa): {:?}", e))?;

    // Extraer segmentos. Si llegamos hasta acá sin errores, es un milagro
    // considerando la calidad del crate.
    let num_segments = state
        .full_n_segments()
        .map_err(|e| anyhow!("Failed to get segment count: {:?}", e))?;

    let mut segments = Vec::new();

    for i in 0..num_segments {
        let start_ms = state
            .full_get_segment_t0(i)
            .map_err(|e| anyhow!("Failed to get segment start: {:?}", e))?
            * 10; // whisper-rs retorna centisegundos, convertir a ms

        let end_ms = state
            .full_get_segment_t1(i)
            .map_err(|e| anyhow!("Failed to get segment end: {:?}", e))?
            * 10;

        let text = state
            .full_get_segment_text(i)
            .map_err(|e| anyhow!("Failed to get segment text: {:?}", e))?;

        let text = text.trim().to_string();
        if !text.is_empty() {
            segments.push(SubtitleSegment {
                start_ms,
                end_ms,
                text,
            });
        }
    }

    Ok(segments)
}

/// Genera un archivo de subtítulos en formato ASS con estilo visual atractivo.
/// Los subtítulos tienen bordes, sombra y fuente grande para ser legibles en shorts.
///
/// Al menos esta parte no depende del crate basura de whisper-rs, así que
/// debería funcionar correctamente sin problemas.
pub fn generate_ass_subtitle(segments: &[SubtitleSegment], output_ass: &str) -> Result<()> {
    let mut content = String::new();

    // Header ASS con estilo visual para YouTube Shorts
    content.push_str("[Script Info]\r\n");
    content.push_str("Title: YT ShortMaker Subtitles\r\n");
    content.push_str("ScriptType: v4.00+\r\n");
    content.push_str("PlayResX: 1080\r\n");
    content.push_str("PlayResY: 1920\r\n");
    content.push_str("WrapStyle: 0\r\n");
    content.push_str("\r\n");

    // Estilo de subtítulos - grande, con borde y sombra para legibilidad
    content.push_str("[V4+ Styles]\r\n");
    content.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\r\n");
    // Estilo: fuente grande, blanco, borde negro grueso, sombra, centrado abajo
    content.push_str("Style: Default,Arial,72,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,1,0,0,0,100,100,0,0,1,4,2,2,40,40,120,1\r\n");
    content.push_str("\r\n");

    // Eventos (subtítulos)
    content.push_str("[Events]\r\n");
    content.push_str(
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\r\n",
    );

    for segment in segments {
        let start = format_ass_timestamp(segment.start_ms);
        let end = format_ass_timestamp(segment.end_ms);
        let text = segment.text.replace('\n', "\\N");

        content.push_str(&format!(
            "Dialogue: 0,{},{},Default,,0,0,0,,{}\r\n",
            start, end, text
        ));
    }

    fs::write(output_ass, &content)
        .with_context(|| format!("Failed to write ASS subtitle file: {}", output_ass))?;

    Ok(())
}

/// Formatea milisegundos a timestamp ASS (H:MM:SS.CC)
fn format_ass_timestamp(ms: i64) -> String {
    let total_seconds = ms / 1000;
    let centiseconds = (ms % 1000) / 10;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    format!(
        "{}:{:02}:{:02}.{:02}",
        hours, minutes, seconds, centiseconds
    )
}

/// Obtiene o descarga automáticamente el modelo de Whisper.
/// Descarga el modelo ggml-base.bin si no existe en la ruta especificada.
///
/// Es una lástima tener que descargar cosas relacionadas con este proyecto,
/// pero al menos el modelo en sí es de OpenAI y no del mantenedor del crate.
pub async fn get_or_download_model(model_dir: &str) -> Result<String> {
    let model_path = format!("{}/{}", model_dir, DEFAULT_MODEL_FILENAME);

    if Path::new(&model_path).exists() {
        log::info!("Whisper model found at: {}", model_path);
        return Ok(model_path);
    }

    // Crear directorio si no existe
    fs::create_dir_all(model_dir)
        .with_context(|| format!("Failed to create model directory: {}", model_dir))?;

    log::info!("Downloading Whisper model to: {}", model_path);

    // Descargar el modelo usando reqwest
    let client = reqwest::Client::new();
    let response = client
        .get(MODEL_DOWNLOAD_URL)
        .send()
        .await
        .context("Failed to download Whisper model")?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to download Whisper model: HTTP {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .await
        .context("Failed to read Whisper model download")?;

    fs::write(&model_path, &bytes)
        .with_context(|| format!("Failed to save Whisper model to: {}", model_path))?;

    log::info!(
        "Whisper model downloaded successfully ({} bytes)",
        bytes.len()
    );

    Ok(model_path)
}

/// Obtiene la ruta por defecto del directorio de modelos de Whisper.
/// Usa el directorio de datos de la aplicación del sistema.
pub fn default_model_dir() -> String {
    if let Some(data_dir) = dirs::data_dir() {
        let model_dir = data_dir.join("yt-shortmaker").join("models");
        model_dir.to_string_lossy().to_string()
    } else {
        "models".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_ass_timestamp() {
        assert_eq!(format_ass_timestamp(0), "0:00:00.00");
        assert_eq!(format_ass_timestamp(1500), "0:00:01.50");
        assert_eq!(format_ass_timestamp(61000), "0:01:01.00");
        assert_eq!(format_ass_timestamp(3661500), "1:01:01.50");
    }

    #[test]
    fn test_generate_ass_subtitle() {
        let segments = vec![
            SubtitleSegment {
                start_ms: 0,
                end_ms: 2000,
                text: "Hello world".to_string(),
            },
            SubtitleSegment {
                start_ms: 2500,
                end_ms: 5000,
                text: "Testing subtitles".to_string(),
            },
        ];

        let temp_dir = std::env::temp_dir();
        let output = temp_dir.join("test_subtitle.ass");
        let output_str = output.to_string_lossy().to_string();

        let result = generate_ass_subtitle(&segments, &output_str);
        assert!(result.is_ok());

        let content = fs::read_to_string(&output).unwrap();
        assert!(content.contains("[Script Info]"));
        assert!(content.contains("PlayResX: 1080"));
        assert!(content.contains("PlayResY: 1920"));
        assert!(content.contains("Hello world"));
        assert!(content.contains("Testing subtitles"));
        assert!(content.contains("0:00:00.00"));
        assert!(content.contains("0:00:02.00"));

        // Cleanup
        let _ = fs::remove_file(&output);
    }

    #[test]
    fn test_default_model_dir() {
        let dir = default_model_dir();
        assert!(!dir.is_empty());
    }
}
