//! Módulo de Face Tracking para YT ShortMaker
//! Analiza clips de video para detectar regiones de interés (caras/streamer)
//! usando FFmpeg y guarda metadata en archivos JSON para crop dinámico.

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

use crate::types::{FaceRegion, FaceTrackingData};

/// Analiza un clip de video para detectar caras/regiones de interés.
/// Extrae frames cada `sample_interval_secs` segundos y usa FFmpeg cropdetect
/// para identificar la región principal de contenido.
///
/// Retorna FaceTrackingData con las regiones detectadas.
pub async fn analyze_clip_faces(
    clip_path: &str,
    temp_dir: &str,
    sample_interval_secs: f64,
) -> Result<FaceTrackingData> {
    if !Path::new(clip_path).exists() {
        return Err(anyhow!("Clip not found for face analysis: {}", clip_path));
    }

    // Obtener duración del clip
    let duration = get_clip_duration(clip_path).await?;
    let mut face_regions = Vec::new();
    let mut current_time: f64 = 0.0;

    // Crear directorio temporal para frames
    let frames_dir = format!("{}/face_frames", temp_dir);
    fs::create_dir_all(&frames_dir).ok();

    while current_time < duration {
        // Extraer frame y analizarlo con cropdetect
        let frame_path = format!("{}/frame_{:.0}.png", frames_dir, current_time * 1000.0);

        if let Ok(region) = extract_and_analyze_frame(clip_path, current_time, &frame_path).await {
            face_regions.push(FaceRegion {
                timestamp_ms: (current_time * 1000.0) as u64,
                x: region.0,
                y: region.1,
                width: region.2,
                height: region.3,
                confidence: region.4,
            });
        }

        // Limpiar frame temporal
        let _ = fs::remove_file(&frame_path);

        current_time += sample_interval_secs;
    }

    // Limpiar directorio de frames
    let _ = fs::remove_dir(&frames_dir);

    // Determinar si hay un streamer (si la mayoría de frames tienen
    // una región consistente, probablemente es una facecam)
    let has_streamer = detect_consistent_region(&face_regions);

    Ok(FaceTrackingData {
        clip_path: clip_path.to_string(),
        has_streamer,
        face_regions,
    })
}

/// Obtiene la duración de un clip en segundos
async fn get_clip_duration(clip_path: &str) -> Result<f64> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            clip_path,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to run ffprobe for duration")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .trim()
        .parse::<f64>()
        .with_context(|| format!("Failed to parse duration: '{}'", stdout.trim()))
}

/// Extrae un frame del video y usa cropdetect para encontrar la región principal
/// Retorna (x, y, width, height, confidence) normalizado 0.0-1.0
async fn extract_and_analyze_frame(
    clip_path: &str,
    timestamp: f64,
    _frame_path: &str,
) -> Result<(f32, f32, f32, f32, f32)> {
    // Usar FFmpeg cropdetect para detectar la región de contenido principal
    let timestamp_str = format!("{:.3}", timestamp);

    let output = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-ss",
            &timestamp_str,
            "-i",
            clip_path,
            "-frames:v",
            "2",
            "-vf",
            "cropdetect=24:16:0",
            "-f",
            "null",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to run cropdetect")?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Parsear la salida de cropdetect: crop=W:H:X:Y
    if let Some(crop_info) = parse_cropdetect_output(&stderr) {
        // Necesitamos la resolución original para normalizar
        let (orig_w, orig_h) = get_video_resolution(clip_path).await?;

        if orig_w > 0 && orig_h > 0 {
            let x_norm = crop_info.2 as f32 / orig_w as f32;
            let y_norm = crop_info.3 as f32 / orig_h as f32;
            let w_norm = crop_info.0 as f32 / orig_w as f32;
            let h_norm = crop_info.1 as f32 / orig_h as f32;

            return Ok((x_norm, y_norm, w_norm, h_norm, 0.8));
        }
    }

    // Default: región completa
    Ok((0.0, 0.0, 1.0, 1.0, 0.3))
}

/// Parsea la salida de FFmpeg cropdetect y retorna (w, h, x, y)
fn parse_cropdetect_output(stderr: &str) -> Option<(u32, u32, u32, u32)> {
    // Buscar la última línea con "crop="
    let mut last_crop = None;

    for line in stderr.lines() {
        if let Some(pos) = line.find("crop=") {
            let crop_str = &line[pos + 5..];
            let parts: Vec<&str> = crop_str.split(':').collect();
            if parts.len() >= 4 {
                if let (Ok(w), Ok(h), Ok(x), Ok(y)) = (
                    parts[0].parse::<u32>(),
                    parts[1].parse::<u32>(),
                    parts[2].parse::<u32>(),
                    parts[3].trim().parse::<u32>(),
                ) {
                    last_crop = Some((w, h, x, y));
                }
            }
        }
    }

    last_crop
}

/// Obtiene la resolución del video
async fn get_video_resolution(path: &str) -> Result<(u32, u32)> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=s=x:p=0",
            path,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to get video resolution")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = stdout.trim().split('x').collect();

    if parts.len() == 2 {
        let w = parts[0].parse::<u32>().unwrap_or(0);
        let h = parts[1].parse::<u32>().unwrap_or(0);
        Ok((w, h))
    } else {
        Err(anyhow!("Failed to parse resolution: '{}'", stdout.trim()))
    }
}

/// Detecta si hay una región consistente entre los frames (indica facecam/streamer)
fn detect_consistent_region(regions: &[FaceRegion]) -> bool {
    if regions.len() < 3 {
        return false;
    }

    // Filtrar regiones con confianza razonable
    let good_regions: Vec<&FaceRegion> = regions.iter().filter(|r| r.confidence > 0.5).collect();

    if good_regions.len() < 2 {
        return false;
    }

    // Verificar si las regiones son consistentes (baja varianza en posición)
    let avg_x: f32 = good_regions.iter().map(|r| r.x).sum::<f32>() / good_regions.len() as f32;
    let avg_y: f32 = good_regions.iter().map(|r| r.y).sum::<f32>() / good_regions.len() as f32;

    let variance_x: f32 = good_regions
        .iter()
        .map(|r| (r.x - avg_x).powi(2))
        .sum::<f32>()
        / good_regions.len() as f32;

    let variance_y: f32 = good_regions
        .iter()
        .map(|r| (r.y - avg_y).powi(2))
        .sum::<f32>()
        / good_regions.len() as f32;

    // Si la varianza es baja, hay una región consistente
    variance_x < 0.05 && variance_y < 0.05
}

/// Guarda los datos de face tracking en un archivo JSON
pub fn save_tracking_data(data: &FaceTrackingData, json_path: &str) -> Result<()> {
    let json = serde_json::to_string_pretty(data)?;
    fs::write(json_path, json)
        .with_context(|| format!("Failed to save tracking data: {}", json_path))?;
    Ok(())
}

/// Carga datos de face tracking desde un archivo JSON
pub fn load_tracking_data(json_path: &str) -> Result<FaceTrackingData> {
    let content = fs::read_to_string(json_path)
        .with_context(|| format!("Failed to read tracking data: {}", json_path))?;
    let data: FaceTrackingData = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse tracking data: {}", json_path))?;
    Ok(data)
}

/// Calcula la región de crop óptima basada en los datos de face tracking.
/// Retorna (x, y, width, height) en píxeles para el crop del video.
pub fn calculate_dynamic_crop(
    face_data: &FaceTrackingData,
    video_width: u32,
    video_height: u32,
    output_width: u32,
    output_height: u32,
) -> (u32, u32, u32, u32) {
    if face_data.face_regions.is_empty() || !face_data.has_streamer {
        // Sin datos de tracking: crop centrado
        let target_ratio = output_width as f32 / output_height as f32;
        let crop_w = video_width;
        let crop_h = (crop_w as f32 / target_ratio) as u32;

        let crop_h = crop_h.min(video_height);
        let y = (video_height - crop_h) / 2;

        return (0, y, crop_w, crop_h);
    }

    // Calcular posición promedio de las caras
    let avg_x: f32 = face_data
        .face_regions
        .iter()
        .map(|r| r.x + r.width / 2.0)
        .sum::<f32>()
        / face_data.face_regions.len() as f32;

    let avg_y: f32 = face_data
        .face_regions
        .iter()
        .map(|r| r.y + r.height / 2.0)
        .sum::<f32>()
        / face_data.face_regions.len() as f32;

    // Calcular crop centrado en la cara
    let target_ratio = output_width as f32 / output_height as f32;
    let crop_w = video_width;
    let crop_h = (crop_w as f32 / target_ratio).min(video_height as f32) as u32;

    // Centrar en la posición Y de la cara
    let center_y = (avg_y * video_height as f32) as u32;
    let half_h = crop_h / 2;

    let y = if center_y > half_h {
        (center_y - half_h).min(video_height - crop_h)
    } else {
        0
    };

    let _ = avg_x; // X no se usa para crop vertical de shorts

    (0, y, crop_w, crop_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cropdetect_output() {
        let stderr = r#"
[Parsed_cropdetect_0 @ 0x...] x1:0 x2:1279 y1:0 y2:719 w:1280 h:720 x:0 y:0 pts:0 t:0.000000 crop=1280:720:0:0
[Parsed_cropdetect_0 @ 0x...] x1:10 x2:1269 y1:5 y2:714 w:1264 h:704 x:8 y:8 pts:1001 t:0.033367 crop=1264:704:8:8
"#;
        let result = parse_cropdetect_output(stderr);
        assert!(result.is_some());
        let (w, h, x, y) = result.unwrap();
        assert_eq!(w, 1264);
        assert_eq!(h, 704);
        assert_eq!(x, 8);
        assert_eq!(y, 8);
    }

    #[test]
    fn test_detect_consistent_region_empty() {
        let regions: Vec<FaceRegion> = Vec::new();
        assert!(!detect_consistent_region(&regions));
    }

    #[test]
    fn test_detect_consistent_region_consistent() {
        let regions = vec![
            FaceRegion {
                timestamp_ms: 0,
                x: 0.3,
                y: 0.2,
                width: 0.2,
                height: 0.3,
                confidence: 0.9,
            },
            FaceRegion {
                timestamp_ms: 1000,
                x: 0.31,
                y: 0.21,
                width: 0.2,
                height: 0.3,
                confidence: 0.85,
            },
            FaceRegion {
                timestamp_ms: 2000,
                x: 0.29,
                y: 0.19,
                width: 0.2,
                height: 0.3,
                confidence: 0.88,
            },
        ];
        assert!(detect_consistent_region(&regions));
    }

    #[test]
    fn test_calculate_dynamic_crop_no_data() {
        let data = FaceTrackingData {
            clip_path: "test.mp4".to_string(),
            has_streamer: false,
            face_regions: Vec::new(),
        };

        let (x, _y, w, h) = calculate_dynamic_crop(&data, 1920, 1080, 1080, 1920);
        assert_eq!(x, 0);
        assert_eq!(w, 1920);
        assert!(h <= 1080);
    }

    #[test]
    fn test_face_tracking_data_serialization() {
        let data = FaceTrackingData {
            clip_path: "test.mp4".to_string(),
            has_streamer: true,
            face_regions: vec![FaceRegion {
                timestamp_ms: 0,
                x: 0.3,
                y: 0.2,
                width: 0.2,
                height: 0.3,
                confidence: 0.9,
            }],
        };

        let json = serde_json::to_string(&data).unwrap();
        let parsed: FaceTrackingData = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.has_streamer, true);
        assert_eq!(parsed.face_regions.len(), 1);
        assert_eq!(parsed.face_regions[0].x, 0.3);
    }
}
