//! Short Exporter Module for YT ShortMaker
//! Handles exporting clips to short format using JSON templates ("planos")
//!
//! This module provides a flexible system for defining video compositions
//! through JSON templates, allowing users to create custom layouts for their shorts.

use anyhow::{anyhow, Context, Result};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ============================================================================
// Data Structures for Plano (Template) System
// ============================================================================

/// Embedded example image for preview generation (no external file needed)
/// This is the default preview source image compiled into the binary
const EXAMPLE_IMAGE_DATA: &[u8] = include_bytes!("../example.png");

/// Output resolution for shorts (9:16 aspect ratio)
#[allow(dead_code)]
const OUTPUT_WIDTH: u32 = 1080;
#[allow(dead_code)]
const OUTPUT_HEIGHT: u32 = 1920;

/// Position value that can be pixels, centered, or a special keyword
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum PositionValue {
    /// Absolute pixel position
    Pixels(i32),
    /// Special keyword: "center"
    Keyword(String),
}

impl PositionValue {
    /// Resolve to actual pixel value given the container size and element size
    #[allow(dead_code)]
    pub fn resolve(&self, container_size: u32, element_size: u32) -> i32 {
        match self {
            PositionValue::Pixels(px) => *px,
            PositionValue::Keyword(kw) => {
                let kw_lower = kw.to_lowercase();
                if kw_lower == "center" {
                    ((container_size - element_size) / 2) as i32
                } else if kw_lower.ends_with('%') {
                    // Parse percentage
                    if let Ok(pct) = kw_lower.trim_end_matches('%').parse::<f32>() {
                        ((container_size as f32) * (pct / 100.0)) as i32
                    } else {
                        0
                    }
                } else {
                    0
                }
            }
        }
    }
}

impl Default for PositionValue {
    fn default() -> Self {
        PositionValue::Pixels(0)
    }
}

/// Size value that can be pixels, "full", or percentage
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum SizeValue {
    /// Absolute pixel size
    Pixels(u32),
    /// Special keyword: "full" or percentage like "50%"
    Keyword(String),
}

impl SizeValue {
    /// Resolve to actual pixel value given the reference size
    #[allow(dead_code)]
    pub fn resolve(&self, reference_size: u32) -> u32 {
        match self {
            SizeValue::Pixels(px) => *px,
            SizeValue::Keyword(kw) => {
                let kw_lower = kw.to_lowercase();
                if kw_lower == "full" {
                    reference_size
                } else if kw_lower.ends_with('%') {
                    // Parse percentage
                    if let Ok(pct) = kw_lower.trim_end_matches('%').parse::<f32>() {
                        ((reference_size as f32) * (pct / 100.0)) as u32
                    } else {
                        reference_size
                    }
                } else {
                    reference_size
                }
            }
        }
    }
}

impl Default for SizeValue {
    fn default() -> Self {
        SizeValue::Keyword("full".to_string())
    }
}

/// Position and size of an element in the composition
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Position {
    /// X position (from left)
    #[serde(default)]
    pub x: PositionValue,
    /// Y position (from top)
    #[serde(default)]
    pub y: PositionValue,
    /// Width of the element
    #[serde(default)]
    pub width: SizeValue,
    /// Height of the element
    #[serde(default)]
    pub height: SizeValue,
}

/// Crop configuration for clips
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Crop {
    /// Start X position for crop (from left)
    #[serde(default)]
    pub x_from: Option<i32>,
    /// End X position for crop
    #[serde(default)]
    pub x_to: Option<i32>,
    /// Start Y position for crop (from top)
    #[serde(default)]
    pub y_from: Option<i32>,
    /// End Y position for crop
    #[serde(default)]
    pub y_to: Option<i32>,
}

impl Crop {
    /// Check if any crop values are specified
    #[allow(dead_code)]
    pub fn is_specified(&self) -> bool {
        self.x_from.is_some() || self.x_to.is_some() || self.y_from.is_some() || self.y_to.is_some()
    }
}

/// Shader effect types
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ShaderEffect {
    /// Blur effect with configurable intensity
    Blur {
        #[serde(default = "default_blur_intensity")]
        intensity: u32,
    },
}

fn default_blur_intensity() -> u32 {
    20
}

/// Scaling mode for the video/clip
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// Cover the area (crop excess), maintaining aspect ratio
    Cover,
    /// Contain within the area (add letterbox/pillarbox), maintaining aspect ratio
    Contain,
    /// Stretch to fill the area (distort), ignoring aspect ratio
    Stretch,
}

fn default_fit() -> Fit {
    Fit::Stretch
}

/// A single object in the plano (template)
/// Order in the array determines layer order (index 0 = back, higher = front)
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PlanoObject {
    /// Original clip from source video
    /// Can be used multiple times (e.g., blurred background + main video)
    Clip {
        position: Position,
        #[serde(default)]
        crop: Option<Crop>,
        /// How to fit the video into the position box
        #[serde(default = "default_fit")]
        fit: Fit,
        /// User comment (ignored during processing)
        #[serde(default)]
        comment: Option<String>,
    },

    /// Static image overlay (frames, watermarks, promo)
    Image {
        /// Path to the image file
        path: String,
        position: Position,
        /// Opacity (0.0 - 1.0, default 1.0)
        #[serde(default = "default_opacity")]
        opacity: f32,
        /// User comment (ignored during processing)
        #[serde(default)]
        comment: Option<String>,
    },

    /// Shader effect applied to previous layer or source
    Shader {
        effect: ShaderEffect,
        position: Position,
        /// User comment (ignored during processing)
        #[serde(default)]
        comment: Option<String>,
    },

    /// Background video (gameplay, animations, etc.)
    Video {
        /// Path to the video file
        path: String,
        position: Position,
        /// Whether to loop the video if shorter than main clip
        #[serde(default = "default_true")]
        loop_video: bool,
        /// Opacity (0.0 - 1.0, default 1.0)
        #[serde(default = "default_opacity")]
        opacity: f32,
        /// How to fit the video into the position box
        #[serde(default = "default_fit")]
        fit: Fit,
        /// User comment (ignored during processing)
        #[serde(default)]
        comment: Option<String>,
    },
}

fn default_opacity() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

// ============================================================================
// Plano (Template) Management
// ============================================================================

/// Load a plano from a JSON file
pub fn load_plano(path: &str) -> Result<Vec<PlanoObject>> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read plano file: {}", path))?;

    // Remove // comments (for tech-savvy users)
    let cleaned = remove_js_comments(&content);

    let plano: Vec<PlanoObject> = serde_json::from_str(&cleaned)
        .with_context(|| format!("Failed to parse plano JSON: {}", path))?;

    Ok(plano)
}

/// Save a plano to a JSON file
pub fn save_plano(path: &str, plano: &[PlanoObject]) -> Result<()> {
    let json = serde_json::to_string_pretty(plano)?;
    fs::write(path, json)?;
    Ok(())
}

/// Remove JavaScript-style // comments from JSON
/// This allows tech users to add inline comments
fn remove_js_comments(content: &str) -> String {
    let mut result = String::new();
    let mut in_string = false;
    let mut chars = content.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' && !in_string {
            in_string = true;
            result.push(c);
        } else if c == '"' && in_string {
            // Check for escaped quote
            let prev_backslashes = result.chars().rev().take_while(|&x| x == '\\').count();
            if prev_backslashes % 2 == 0 {
                in_string = false;
            }
            result.push(c);
        } else if !in_string && c == '/' && chars.peek() == Some(&'/') {
            // Skip until end of line
            chars.next(); // consume second /
            while let Some(&next) = chars.peek() {
                if next == '\n' {
                    break;
                }
                chars.next();
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Create a default plano template
pub fn create_default_plano() -> Vec<PlanoObject> {
    vec![
        // Layer 0: Blurred background (full screen)
        PlanoObject::Clip {
            position: Position {
                x: PositionValue::Pixels(0),
                y: PositionValue::Pixels(0),
                width: SizeValue::Keyword("full".to_string()),
                height: SizeValue::Keyword("full".to_string()),
            },
            crop: None,
            fit: Fit::Cover,
            comment: Some("Fondo desenfocado del clip original".to_string()),
        },
        // Layer 1: Blur shader on background
        PlanoObject::Shader {
            effect: ShaderEffect::Blur { intensity: 20 },
            position: Position {
                x: PositionValue::Pixels(0),
                y: PositionValue::Pixels(0),
                width: SizeValue::Keyword("full".to_string()),
                height: SizeValue::Keyword("full".to_string()),
            },
            comment: Some("Shader de blur sobre el fondo".to_string()),
        },
        // Layer 2: Main video in center
        PlanoObject::Clip {
            position: Position {
                x: PositionValue::Pixels(0),
                y: PositionValue::Keyword("center".to_string()),
                width: SizeValue::Keyword("full".to_string()),
                height: SizeValue::Pixels(1200),
            },
            crop: None,
            fit: Fit::Cover,
            comment: Some("Video principal del clip".to_string()),
        },
    ]
}

// ============================================================================
// FFmpeg Filter Generation
// ============================================================================

/// Context for building FFmpeg filter chain
#[allow(dead_code)]
struct FilterContext {
    /// List of input files (indices for FFmpeg)
    inputs: Vec<String>,
    /// Current output label
    current_label: String,
    /// Filter chain parts
    filters: Vec<String>,
    /// Counter for generating unique labels
    label_counter: usize,
}

#[allow(dead_code)]
impl FilterContext {
    fn new() -> Self {
        Self {
            inputs: Vec::new(),
            current_label: String::new(),
            filters: Vec::new(),
            label_counter: 0,
        }
    }

    fn next_label(&mut self) -> String {
        let label = format!("layer{}", self.label_counter);
        self.label_counter += 1;
        label
    }
}

/// Build FFmpeg filter_complex string from a plano
/// Returns (filter_string, input_files_needed)
#[allow(dead_code)]
pub fn build_ffmpeg_filter(plano: &[PlanoObject], clip_path: &str) -> (String, Vec<String>) {
    let mut ctx = FilterContext::new();
    let mut inputs: Vec<String> = vec![clip_path.to_string()]; // Main clip is always input 0
    let clip_input_used = 0; // Track which input index to use for clips

    // First pass: collect all additional inputs needed
    let mut additional_inputs: Vec<(usize, String)> = Vec::new(); // (plano_index, path)

    for (idx, obj) in plano.iter().enumerate() {
        match obj {
            PlanoObject::Image { path, .. } => {
                if Path::new(path).exists() {
                    additional_inputs.push((idx, path.clone()));
                }
            }
            PlanoObject::Video { path, .. } => {
                if Path::new(path).exists() {
                    additional_inputs.push((idx, path.clone()));
                }
            }
            _ => {}
        }
    }

    // Add additional inputs
    for (_, path) in &additional_inputs {
        inputs.push(path.clone());
    }

    // Build input index mapping for additional files
    let mut input_mapping: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    for (i, (plano_idx, _)) in additional_inputs.iter().enumerate() {
        input_mapping.insert(*plano_idx, i + 1); // +1 because main clip is 0
    }

    let mut current_label = "base".to_string();

    ctx.filters.push(format!(
        "color=c=black:s={}x{}:r=60:d=36000[base]",
        OUTPUT_WIDTH, OUTPUT_HEIGHT
    ));

    for (idx, obj) in plano.iter().enumerate() {
        let next_label = if idx == plano.len() - 1 {
            "out".to_string()
        } else {
            ctx.next_label()
        };

        match obj {
            PlanoObject::Clip {
                position,
                crop,
                fit,
                ..
            } => {
                let w = position.width.resolve(OUTPUT_WIDTH);
                let h = position.height.resolve(OUTPUT_HEIGHT);

                // Start with input
                let mut base_filter = format!("[{}:v]", clip_input_used);

                // Apply user crop first if specified
                if let Some(c) = crop {
                    if c.is_specified() {
                        let x_from = c.x_from.unwrap_or(0);
                        let x_to = c.x_to.unwrap_or(0);
                        let y_from = c.y_from.unwrap_or(0);
                        let y_to = c.y_to.unwrap_or(0);

                        if x_to > x_from {
                            let crop_w = x_to - x_from;
                            let crop_x = x_from;
                            base_filter =
                                format!("{}crop={}:ih:{}:0,", base_filter, crop_w, crop_x);
                        }
                        if y_to > y_from {
                            let crop_h = y_to - y_from;
                            let crop_y = y_from;
                            base_filter =
                                format!("{}crop=iw:{}:{}:{},", base_filter, crop_h, 0, crop_y);
                        }
                    }
                }

                // Now apply scaling based on Fit mode
                let scale_filter = match fit {
                    Fit::Cover => format!(
                        "{}scale={}:{}:force_original_aspect_ratio=increase,crop={}:{}",
                        base_filter, w, h, w, h
                    ),
                    Fit::Contain => format!(
                        "{}scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2,setsar=1",
                        base_filter, w, h, w, h
                    ),
                    Fit::Stretch => format!(
                        "{}scale={}:{},setsar=1",
                        base_filter, w, h
                    ),
                };

                // Overlay on previous
                let x = position.x.resolve(OUTPUT_WIDTH, w);
                let y = position.y.resolve(OUTPUT_HEIGHT, h);

                ctx.filters.push(format!("{}[tmp{}]", scale_filter, idx));
                ctx.filters.push(format!(
                    "[{}][tmp{}]overlay={}:{}[{}]",
                    current_label, idx, x, y, next_label
                ));

                current_label = next_label;
            }

            PlanoObject::Shader {
                effect, position, ..
            } => {
                let _w = position.width.resolve(OUTPUT_WIDTH);
                let _h = position.height.resolve(OUTPUT_HEIGHT);

                match effect {
                    ShaderEffect::Blur { intensity } => {
                        // Apply blur to current composition
                        ctx.filters.push(format!(
                            "[{}]boxblur={}:{}[{}]",
                            current_label, intensity, intensity, next_label
                        ));
                    }
                }
                current_label = next_label;
            }

            PlanoObject::Image {
                position, opacity, ..
            } => {
                if let Some(&input_idx) = input_mapping.get(&idx) {
                    let w = position.width.resolve(OUTPUT_WIDTH);
                    let h = position.height.resolve(OUTPUT_HEIGHT);
                    let x = position.x.resolve(OUTPUT_WIDTH, w);
                    let y = position.y.resolve(OUTPUT_HEIGHT, h);

                    // Scale and apply opacity to image
                    let mut img_filter = format!("[{}:v]scale={}:{}", input_idx, w, h);

                    if *opacity < 1.0 {
                        img_filter = format!(
                            "{},format=rgba,colorchannelmixer=aa={}",
                            img_filter, opacity
                        );
                    }

                    ctx.filters.push(format!("{}[img{}]", img_filter, idx));
                    ctx.filters.push(format!(
                        "[{}][img{}]overlay={}:{}[{}]",
                        current_label, idx, x, y, next_label
                    ));

                    current_label = next_label;
                }
            }

            PlanoObject::Video {
                position,
                loop_video,
                opacity,
                fit,
                ..
            } => {
                if let Some(&input_idx) = input_mapping.get(&idx) {
                    let w = position.width.resolve(OUTPUT_WIDTH);
                    let h = position.height.resolve(OUTPUT_HEIGHT);
                    let x = position.x.resolve(OUTPUT_WIDTH, w);
                    let y = position.y.resolve(OUTPUT_HEIGHT, h);

                    let mut vid_filter = format!("[{}:v]", input_idx);

                    // Add loop if needed
                    if *loop_video {
                        vid_filter =
                            format!("{}loop=-1:size=32767,setpts=N/FRAME_RATE/TB,", vid_filter);
                    }

                    // Apply Fit scaling
                    vid_filter = match fit {
                        Fit::Cover => format!(
                            "{}scale={}:{}:force_original_aspect_ratio=increase,crop={}:{}",
                            vid_filter, w, h, w, h
                        ),
                        Fit::Contain => format!(
                            "{}scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2,setsar=1",
                            vid_filter, w, h, w, h
                        ),
                        Fit::Stretch => format!(
                            "{}scale={}:{},setsar=1",
                            vid_filter, w, h
                        ),
                    };

                    // Add opacity if < 1.0
                    if *opacity < 1.0 {
                        vid_filter = format!(
                            "{},format=rgba,colorchannelmixer=aa={}",
                            vid_filter, opacity
                        );
                    }

                    ctx.filters.push(format!("{}[vid{}]", vid_filter, idx));
                    ctx.filters.push(format!(
                        "[{}][vid{}]overlay={}:{}[{}]",
                        current_label, idx, x, y, next_label
                    ));

                    current_label = next_label;
                }
            }
        }
    }

    // If loop was empty (no objects), we still have [base] as current_label ("base")
    // We need to output something. If loop finished, next_label was "out" only if len > 0.
    // Ideally user provided objects. If not, output black screen?
    // If plano is empty, we must map base to out
    if plano.is_empty() {
        ctx.filters.push(format!("[base]null[out]"));
    }

    let filter_str = ctx.filters.join(";");

    (filter_str, inputs)
}

// ============================================================================
// Preview Generation
// ============================================================================

/// Generate a preview image using the example.png and a plano
pub fn generate_preview(
    source_image: &str,
    plano: &[PlanoObject],
    output_path: &str,
) -> Result<()> {
    if !Path::new(source_image).exists() {
        return Err(anyhow!("Source image not found: {}", source_image));
    }

    let (filter, inputs) = build_ffmpeg_filter(plano, source_image);
    debug!("Preview Source: {}", source_image);
    debug!("Preview Filter: {}", filter);

    // Build FFmpeg command for image processing
    let mut args: Vec<String> = Vec::new();

    // Add all inputs
    for input in &inputs {
        args.push("-i".to_string());
        args.push(input.clone());
    }

    // Add filter
    args.push("-filter_complex".to_string());
    args.push(filter);

    // Map output
    args.push("-map".to_string());
    args.push("[out]".to_string());

    // Single frame output
    args.push("-frames:v".to_string());
    args.push("1".to_string());

    args.push("-y".to_string());
    args.push(output_path.to_string());

    let output = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to execute ffmpeg for preview")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("FFmpeg preview failed: {}", stderr);
        return Err(anyhow!("FFmpeg preview generation failed: {}", stderr));
    }

    info!("Preview generated at: {}", output_path);
    Ok(())
}

/// Generate a preview using the embedded example image
/// This version uses the built-in example.png without needing external files
pub fn generate_preview_embedded(plano: &[PlanoObject], output_path: &str) -> Result<()> {
    // Write embedded image to temp file (FFmpeg needs a file path)
    let temp_dir = std::env::temp_dir();
    let temp_image_path = temp_dir.join("yt_shortmaker_example.png");

    fs::write(&temp_image_path, EXAMPLE_IMAGE_DATA)
        .context("Failed to write embedded image to temp file")?;

    // Use the temp file for preview generation
    let result = generate_preview(temp_image_path.to_str().unwrap_or(""), plano, output_path);

    // Clean up temp file (ignore errors)
    let _ = fs::remove_file(&temp_image_path);

    result
}

/// Generate a preview using a frame extracted from a video file
pub fn generate_preview_from_video(
    video_path: &str,
    plano: &[PlanoObject],
    output_path: &str,
) -> Result<()> {
    // 1. Extract a frame from the video to a temp file
    let temp_dir = std::env::temp_dir();
    let temp_frame_path = temp_dir.join("yt_shortmaker_frame.png");

    info!("Extracting preview frame from: {}", video_path);

    let status = Command::new("ffmpeg")
        .args([
            "-ss",
            "00:00:05", // Try to get frame at 5 seconds
            "-i",
            video_path,
            "-frames:v",
            "1",
            "-y",
            temp_frame_path.to_str().unwrap(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .context("Failed to execute ffmpeg for frame extraction")?;

    if !status.success() {
        // Fallback: try at 0 seconds if video is short
        let _ = Command::new("ffmpeg")
            .args([
                "-i",
                video_path,
                "-frames:v",
                "1",
                "-y",
                temp_frame_path.to_str().unwrap(),
            ])
            .output();
    }

    if !temp_frame_path.exists() {
        error!("Failed to extract frame from video");
    } else {
        debug!("Frame extracted to: {:?}", temp_frame_path);
    }

    // 2. Use the extracted frame for preview generation
    let result = generate_preview(temp_frame_path.to_str().unwrap_or(""), plano, output_path);

    // 3. Clean up temp frame
    let _ = fs::remove_file(&temp_frame_path);

    result
}

// ============================================================================
// Export Functions
// ============================================================================

/// Export a single clip using the plano template
pub async fn export_clip(
    clip_path: &str,
    plano: &[PlanoObject],
    output_path: &str,
    cancellation_token: Arc<AtomicBool>,
    log_callback: Option<&ExportLogCallback>,
) -> Result<()> {
    if !Path::new(clip_path).exists() {
        let msg = format!("Clip not found: {}", clip_path);
        if let Some(cb) = log_callback {
            cb(ExportLogLevel::Error, msg.clone());
        }
        error!("{}", msg);
        return Err(anyhow!("Clip not found: {}", clip_path));
    }

    let msg = format!("Exporting clip: {} -> {}", clip_path, output_path);
    if let Some(cb) = log_callback {
        cb(ExportLogLevel::Info, msg.clone());
    }
    info!("{}", msg);

    let (filter, inputs) = build_ffmpeg_filter(plano, clip_path);
    debug!("Export Filter: {}", filter);

    // Build FFmpeg command
    let mut args: Vec<String> = Vec::new();

    // Add all inputs
    for input in &inputs {
        args.push("-i".to_string());
        args.push(input.clone());
    }

    // Add filter
    args.push("-filter_complex".to_string());
    args.push(filter);

    // Map output
    args.push("-map".to_string());
    args.push("[out]".to_string());
    args.push("-map".to_string());
    args.push("0:a?".to_string()); // Audio from main clip (optional)

    // Output settings
    args.push("-c:v".to_string());
    args.push("libx264".to_string());
    args.push("-preset".to_string());
    args.push("superfast".to_string());

    args.push("-c:a".to_string());
    args.push("aac".to_string());

    args.push("-b:a".to_string());
    args.push("192k".to_string());

    // 4. Limit output duration to the length of the main clip
    // This prevents infinite loops if background video is looping
    // CRITICAL: We MUST have a duration, otherwise the 10h black canvas will make the video 10h long
    let duration = crate::video::get_video_duration_precise(clip_path).context(
        "Failed to determine clip duration. Cannot safely export without known duration.",
    )?;

    let msg = format!("Detected clip duration: {:.3}s", duration);
    if let Some(cb) = log_callback {
        cb(ExportLogLevel::Info, msg.clone());
    }
    info!("{}", msg);

    args.push("-t".to_string());
    args.push(format!("{:.3}", duration));

    args.push("-y".to_string());
    args.push(output_path.to_string());

    let mut command = tokio::process::Command::new("ffmpeg");
    let cmd_str = format!("FFmpeg command args: {:?}", args);
    // Only log command debug if callback exists (to avoid spamming main log if not debug)
    if let Some(cb) = log_callback {
        cb(ExportLogLevel::Info, cmd_str.clone());
    }
    command.args(&args);

    // Run with cancellation support
    let output = crate::video::run_command_with_cancellation(command, cancellation_token).await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = format!("FFmpeg export failed for {}: {}", clip_path, stderr);
        if let Some(cb) = log_callback {
            cb(ExportLogLevel::Error, msg.clone());
        }
        error!("{}", msg);
        return Err(anyhow!("FFmpeg export failed: {}", stderr));
    }

    let msg = format!("Successfully exported: {}", output_path);
    if let Some(cb) = log_callback {
        cb(ExportLogLevel::Success, msg.clone());
    }
    info!("{}", msg);
    Ok(())
}

/// Log level for export operations
#[derive(Debug, Clone, Copy)]
pub enum ExportLogLevel {
    Info,
    Success,
    #[allow(dead_code)]
    Warning,
    Error,
}

/// Callback for logging export events
pub type ExportLogCallback = Box<dyn Fn(ExportLogLevel, String) + Send + Sync>;

/// Progress callback type for batch exports
pub type ExportProgressCallback = Box<dyn Fn(usize, usize, &str) + Send + Sync>;

/// Export all clips from multiple directories using a plano template
pub async fn export_batch(
    clip_dirs: &[String],
    plano: &[PlanoObject],
    output_dir: &str,
    progress_callback: Option<ExportProgressCallback>,
    log_callback: Option<ExportLogCallback>,
    cancellation_token: Arc<AtomicBool>,
) -> Result<Vec<String>> {
    // Helper for logging
    let log = |level: ExportLogLevel, msg: String| {
        match level {
            ExportLogLevel::Info => info!("{}", msg),
            ExportLogLevel::Success => info!("(SUCCESS) {}", msg),
            ExportLogLevel::Warning => info!("(WARNING) {}", msg),
            ExportLogLevel::Error => error!("{}", msg),
        }
        if let Some(ref cb) = log_callback {
            cb(level, msg);
        }
    };

    // Ensure output directory exists
    fs::create_dir_all(output_dir)?;
    log(
        ExportLogLevel::Info,
        format!("Starting batch export to: {}", output_dir),
    );

    // Collect all clips from all directories
    let mut all_clips: Vec<std::path::PathBuf> = Vec::new();

    for dir in clip_dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    let ext_lower = ext.to_string_lossy().to_lowercase();
                    if ext_lower == "mp4"
                        || ext_lower == "mkv"
                        || ext_lower == "webm"
                        || ext_lower == "mov"
                    {
                        all_clips.push(path);
                    }
                }
            }
        }
    }

    let total = all_clips.len();
    log(
        ExportLogLevel::Info,
        format!("Found {} clips to export", total),
    );
    let mut output_files: Vec<String> = Vec::new();

    for (i, clip_path) in all_clips.iter().enumerate() {
        if cancellation_token.load(Ordering::Relaxed) {
            return Err(anyhow!("Export cancelled by user"));
        }

        let file_name = clip_path.file_name().unwrap().to_string_lossy();
        let output_path = format!("{}/short_{}", output_dir, file_name);

        if let Some(ref callback) = progress_callback {
            callback(i + 1, total, &file_name);
        }

        match export_clip(
            clip_path.to_str().unwrap(),
            plano,
            &output_path,
            cancellation_token.clone(),
            log_callback.as_ref(), // Pass log callback
        )
        .await
        {
            Ok(_) => {
                output_files.push(output_path);
            }
            Err(e) => {
                if e.to_string().contains("cancelled") {
                    return Err(e);
                }
                let msg = format!("Failed to export {}: {}", file_name, e);
                log(ExportLogLevel::Error, msg.clone());
                // eprintln is not needed if we log error
            }
        }
    }

    Ok(output_files)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plano_basic() {
        let json = r#"[
            {
                "type": "clip",
                "position": {"x": 0, "y": 0, "width": "full", "height": "full"}
            }
        ]"#;
        let plano: Vec<PlanoObject> = serde_json::from_str(json).unwrap();
        assert_eq!(plano.len(), 1);
    }

    #[test]
    fn test_parse_plano_with_crop() {
        let json = r#"[
            {
                "type": "clip",
                "position": {"x": 0, "y": 0, "width": "full", "height": "full"},
                "crop": {"x_from": 100, "x_to": 500}
            }
        ]"#;
        let plano: Vec<PlanoObject> = serde_json::from_str(json).unwrap();
        if let PlanoObject::Clip { crop, .. } = &plano[0] {
            assert!(crop.is_some());
            let c = crop.as_ref().unwrap();
            assert_eq!(c.x_from, Some(100));
            assert_eq!(c.x_to, Some(500));
        } else {
            panic!("Expected Clip");
        }
    }

    #[test]
    fn test_parse_plano_with_image() {
        let json = r#"[
            {
                "type": "image",
                "path": "frame.png",
                "position": {"x": 0, "y": 0, "width": 500, "height": 500},
                "opacity": 0.8
            }
        ]"#;
        let plano: Vec<PlanoObject> = serde_json::from_str(json).unwrap();
        if let PlanoObject::Image { opacity, .. } = &plano[0] {
            assert_eq!(*opacity, 0.8);
        } else {
            panic!("Expected Image");
        }
    }

    #[test]
    fn test_parse_plano_with_shader() {
        let json = r#"[
            {
                "type": "shader",
                "effect": {"type": "blur", "intensity": 25},
                "position": {"x": 0, "y": 0, "width": "full", "height": "full"}
            }
        ]"#;
        let plano: Vec<PlanoObject> = serde_json::from_str(json).unwrap();
        match &plano[0] {
            PlanoObject::Shader { effect, .. } => {
                let ShaderEffect::Blur { intensity } = effect;
                assert_eq!(*intensity, 25);
            }
            _ => panic!("Expected Shader"),
        }
    }

    #[test]
    fn test_parse_plano_with_video() {
        let json = r#"[
            {
                "type": "video",
                "path": "background.mp4",
                "position": {"x": 0, "y": 1200, "width": "full", "height": 720},
                "loop_video": true
            }
        ]"#;
        let plano: Vec<PlanoObject> = serde_json::from_str(json).unwrap();
        if let PlanoObject::Video { loop_video, .. } = &plano[0] {
            assert!(*loop_video);
        } else {
            panic!("Expected Video");
        }
    }

    #[test]
    fn test_remove_js_comments() {
        let input = r#"[
            // This is a comment
            {"type": "clip", "position": {"x": 0}}
        ]"#;
        let result = remove_js_comments(input);
        assert!(!result.contains("//"));
        assert!(result.contains("clip"));
    }

    #[test]
    fn test_size_value_resolve() {
        assert_eq!(SizeValue::Pixels(500).resolve(1080), 500);
        assert_eq!(SizeValue::Keyword("full".to_string()).resolve(1080), 1080);
        assert_eq!(SizeValue::Keyword("50%".to_string()).resolve(1000), 500);
    }

    #[test]
    fn test_position_value_resolve() {
        assert_eq!(PositionValue::Pixels(100).resolve(1000, 200), 100);
        assert_eq!(
            PositionValue::Keyword("center".to_string()).resolve(1000, 200),
            400
        );
    }

    #[test]
    fn test_build_ffmpeg_filter_basic() {
        let plano = create_default_plano();
        let (filter, inputs) = build_ffmpeg_filter(&plano, "test.mp4");
        assert!(filter.contains("[out]"));
        assert_eq!(inputs.len(), 1);
    }

    #[test]
    fn test_create_default_plano() {
        let plano = create_default_plano();
        assert_eq!(plano.len(), 3);

        // First should be a clip
        assert!(matches!(plano[0], PlanoObject::Clip { .. }));
        // Second should be a shader
        assert!(matches!(plano[1], PlanoObject::Shader { .. }));
        // Third should be a clip
        assert!(matches!(plano[2], PlanoObject::Clip { .. }));
    }

    #[test]
    fn test_crop_is_specified() {
        let empty = Crop::default();
        assert!(!empty.is_specified());

        let with_x = Crop {
            x_from: Some(100),
            x_to: Some(500),
            y_from: None,
            y_to: None,
        };
        assert!(with_x.is_specified());
    }
}
