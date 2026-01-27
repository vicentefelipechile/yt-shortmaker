//! Shorts transformation module for YT ShortMaker
//! Converts extracted clips to YouTube Shorts format (9:16) with layered composition

use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::config::{ImageOverlay, ShortsConfig};

/// Get video duration as float (seconds with decimals)
#[allow(dead_code)]
pub fn get_video_duration_float(file_path: &str) -> Result<f64> {
    let output = Command::new("ffprobe")
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

/// Get video resolution (width, height)
#[allow(dead_code)]
pub fn get_video_resolution(file_path: &str) -> Result<(u32, u32)> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0:s=x",
            file_path,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("Failed to run ffprobe for resolution")?;

    let res_str = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = res_str.trim().split('x').collect();

    if parts.len() != 2 {
        return Err(anyhow!("Failed to parse video resolution"));
    }

    let width: u32 = parts[0].parse().context("Invalid width")?;
    let height: u32 = parts[1].parse().context("Invalid height")?;

    Ok((width, height))
}

/// Build the FFmpeg filter_complex string for layered video composition
/// Uses a simplified approach that works with any input resolution
fn build_filter_complex(
    config: &ShortsConfig,
    has_background: bool,
    overlay_count: usize,
) -> String {
    let w = config.output_width;
    let h = config.output_height;
    let blur = config.base_blur;
    let main_h = config.main_video_height;
    let opacity = config.background_opacity;

    let mut filters = Vec::new();

    // Layer 1: Base video (blurred background) - scale to fill, then crop, then blur
    // Use 'scale' with fill mode and 'crop' to center
    filters.push(format!(
        "[0:v]scale={}:{}:force_original_aspect_ratio=increase,\
         crop={}:{},\
         boxblur={}:{}[base]",
        w, h, w, h, blur, blur
    ));

    let mut current_layer = "base".to_string();

    // Layer 2: Background video with transparency (if provided)
    if has_background {
        // Loop the background video, scale to fill, crop, and apply transparency
        filters.push(format!(
            "[1:v]loop=-1:size=32767,setpts=N/FRAME_RATE/TB,\
             scale={}:{}:force_original_aspect_ratio=increase,\
             crop={}:{},\
             format=rgba,colorchannelmixer=aa={}[bg]",
            w, h, w, h, opacity
        ));
        filters.push(format!("[{}][bg]overlay=0:0[layer1]", current_layer));
        current_layer = "layer1".to_string();
    }

    // Layer 3: Main video with zoom (crop center) and positioned with offset
    let main_input = if has_background { 2 } else { 1 };
    let zoom = config.main_video_zoom.clamp(0.3, 1.0);
    let y_offset = config.main_video_y_offset;

    // Calculate base Y position (centered) then apply offset
    let base_y = ((h - main_h) / 2) as i32;
    let final_y = (base_y + y_offset).max(0) as u32;

    // Zoom strategy:
    // 1) Scale video to be larger than target (to enable cropping)
    // 2) Crop center portion based on zoom level
    // zoom=0.7 means show 70% of original, so we scale up by 1/0.7 ≈ 1.43x first
    // then crop to exactly our target dimensions
    let scale_factor = 1.0 / zoom;

    // We need the final video to be exactly w x main_h after cropping
    // So we scale it up enough that crop will work
    // Scale to make sure both width AND height are large enough for crop
    filters.push(format!(
        "[{}:v]scale=w={}:h={}:force_original_aspect_ratio=increase,crop={}:{}[main]",
        main_input,
        (w as f32 * scale_factor) as u32,
        (main_h as f32 * scale_factor) as u32,
        w,
        main_h
    ));

    // Overlay main video at calculated position
    filters.push(format!(
        "[{}][main]overlay=0:{}[layer2]",
        current_layer, final_y
    ));
    current_layer = "layer2".to_string();

    // Layer 4: Image overlays
    let overlay_start_input = if has_background { 3 } else { 2 };
    for i in 0..overlay_count {
        let input_idx = overlay_start_input + i;
        let next_layer = if i == overlay_count - 1 {
            "out".to_string()
        } else {
            format!("layer{}", 3 + i)
        };

        // Apply scale to overlay if dimensions specified (handled via placeholder)
        filters.push(format!(
            "[{}][{}:v]overlay=OVERLAY_X_{}:OVERLAY_Y_{}[{}]",
            current_layer, input_idx, i, i, next_layer
        ));
        current_layer = next_layer;
    }

    // If no overlays, rename layer2 to out
    if overlay_count == 0 {
        let last_filter = filters.pop().unwrap();
        filters.push(last_filter.replace("[layer2]", "[out]"));
    }

    filters.join(";")
}

/// Transform a video clip to YouTube Shorts format
pub async fn transform_to_short(
    input_video: &str,
    output_path: &str,
    config: &ShortsConfig,
    use_gpu: bool,
) -> Result<()> {
    if !Path::new(input_video).exists() {
        return Err(anyhow!("Input video not found: {}", input_video));
    }

    let has_background = config
        .background_video
        .as_ref()
        .map(|p| Path::new(p).exists())
        .unwrap_or(false);

    // Validate overlays
    let valid_overlays: Vec<&ImageOverlay> = config
        .overlays
        .iter()
        .filter(|o| Path::new(&o.path).exists())
        .collect();

    // Build filter complex
    let mut filter = build_filter_complex(config, has_background, valid_overlays.len());

    // Replace overlay position placeholders with actual values
    for (i, overlay) in valid_overlays.iter().enumerate() {
        filter = filter.replace(&format!("OVERLAY_X_{}", i), &overlay.x.to_string());
        filter = filter.replace(&format!("OVERLAY_Y_{}", i), &overlay.y.to_string());
    }

    // Build FFmpeg command
    let mut args: Vec<String> = vec!["-i".to_string(), input_video.to_string()];

    // Add background video input if exists
    if has_background {
        if let Some(ref bg_path) = config.background_video {
            args.push("-i".to_string());
            args.push(bg_path.clone());
        }
    }

    // Add main video again for the centered layer
    args.push("-i".to_string());
    args.push(input_video.to_string());

    // Add overlay images
    for overlay in &valid_overlays {
        args.push("-i".to_string());
        args.push(overlay.path.clone());
    }

    // Add filter complex
    args.push("-filter_complex".to_string());
    args.push(filter);

    // Map output
    args.push("-map".to_string());
    args.push("[out]".to_string());
    args.push("-map".to_string());
    args.push("0:a?".to_string()); // Audio from main video (optional)

    // Output settings
    if use_gpu {
        args.push("-c:v".to_string());
        args.push("h264_nvenc".to_string());
        args.push("-preset".to_string());
        args.push("p4".to_string());
        args.push("-rc".to_string());
        args.push("vbr".to_string());
        args.push("-cq".to_string());
        args.push("23".to_string());
        args.push("-b:v".to_string());
        args.push("0".to_string());
    } else {
        args.push("-c:v".to_string());
        args.push("libx264".to_string());
        args.push("-preset".to_string());
        args.push("medium".to_string());
        args.push("-crf".to_string());
        args.push("23".to_string());
    }

    args.push("-c:a".to_string());
    args.push("aac".to_string());
    args.push("-b:a".to_string());
    args.push("192k".to_string());
    args.push("-y".to_string());
    args.push(output_path.to_string());

    let output = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to execute ffmpeg for transformation")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("FFmpeg transformation failed: {}", stderr));
    }

    Ok(())
}

/// Generate a single-frame preview image to visualize the composition
pub fn generate_preview(
    input_video: &str,
    output_image: &str,
    config: &ShortsConfig,
    timestamp_secs: f64,
    _use_gpu: bool,
) -> Result<()> {
    if !Path::new(input_video).exists() {
        return Err(anyhow!("Input video not found: {}", input_video));
    }

    let has_background = config
        .background_video
        .as_ref()
        .map(|p| Path::new(p).exists())
        .unwrap_or(false);

    // Validate overlays
    let valid_overlays: Vec<&ImageOverlay> = config
        .overlays
        .iter()
        .filter(|o| Path::new(&o.path).exists())
        .collect();

    // Build filter complex
    let mut filter = build_filter_complex(config, has_background, valid_overlays.len());

    // Replace overlay position placeholders
    for (i, overlay) in valid_overlays.iter().enumerate() {
        filter = filter.replace(&format!("OVERLAY_X_{}", i), &overlay.x.to_string());
        filter = filter.replace(&format!("OVERLAY_Y_{}", i), &overlay.y.to_string());
    }

    // Build FFmpeg command for single frame
    let timestamp = format!("{:.3}", timestamp_secs);

    let mut args: Vec<String> = vec![
        "-ss".to_string(),
        timestamp.clone(),
        "-i".to_string(),
        input_video.to_string(),
    ];

    // Add background video input if exists
    if has_background {
        if let Some(ref bg_path) = config.background_video {
            args.push("-ss".to_string());
            args.push(timestamp.clone());
            args.push("-i".to_string());
            args.push(bg_path.clone());
        }
    }

    // Add main video again for centered layer
    args.push("-ss".to_string());
    args.push(timestamp);
    args.push("-i".to_string());
    args.push(input_video.to_string());

    // Add overlay images
    for overlay in &valid_overlays {
        args.push("-i".to_string());
        args.push(overlay.path.clone());
    }

    // Add filter complex
    args.push("-filter_complex".to_string());
    args.push(filter);

    // Map output and take single frame
    args.push("-map".to_string());
    args.push("[out]".to_string());
    args.push("-frames:v".to_string());
    args.push("1".to_string());

    args.push("-y".to_string());
    args.push(output_image.to_string());

    let output = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to execute ffmpeg for preview")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("FFmpeg preview failed: {}", stderr));
    }

    Ok(())
}

pub type ProgressCallback = Box<dyn Fn(usize, usize, &str) + Send>;

/// Transform all extracted clips in a directory to shorts format
pub async fn transform_batch(
    input_dir: &str,
    output_dir: &str,
    config: &ShortsConfig,
    use_gpu: bool,
    progress_callback: Option<ProgressCallback>,
) -> Result<Vec<String>> {
    use std::fs;

    // Ensure output directory exists
    fs::create_dir_all(output_dir)?;

    // Find all mp4 files in input directory
    let entries: Vec<_> = fs::read_dir(input_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "mp4")
                .unwrap_or(false)
        })
        .collect();

    let total = entries.len();
    let mut output_files = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        let input_path = entry.path();
        let file_name = input_path.file_name().unwrap().to_string_lossy();
        let output_path = format!("{}/short_{}", output_dir, file_name);

        if let Some(ref callback) = progress_callback {
            callback(i + 1, total, &file_name);
        }

        match transform_to_short(input_path.to_str().unwrap(), &output_path, config, use_gpu).await
        {
            Ok(_) => {
                output_files.push(output_path);
            }
            Err(e) => {
                eprintln!("Failed to transform {}: {}", file_name, e);
            }
        }
    }

    Ok(output_files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ShortsConfig;

    #[test]
    fn test_build_filter_no_background() {
        let config = ShortsConfig::default();
        let filter = build_filter_complex(&config, false, 0);
        assert!(filter.contains("[base]"));
        assert!(filter.contains("[main]"));
        assert!(filter.contains("[out]"));
    }

    #[test]
    fn test_build_filter_with_background() {
        let config = ShortsConfig::default();
        let filter = build_filter_complex(&config, true, 0);
        assert!(filter.contains("[bg]"));
        assert!(filter.contains("colorchannelmixer"));
    }

    #[test]
    fn test_build_filter_with_overlays() {
        let config = ShortsConfig::default();
        let filter = build_filter_complex(&config, false, 2);
        assert!(filter.contains("OVERLAY_X_0"));
        assert!(filter.contains("OVERLAY_X_1"));
    }
}
