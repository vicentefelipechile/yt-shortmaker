//! Configuration management for AutoShorts-Rust-CLI
//! Handles loading and saving settings to settings.json

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Image overlay configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImageOverlay {
    /// Path to the image file
    pub path: String,
    /// X position (from left)
    pub x: i32,
    /// Y position (from top)
    pub y: i32,
    /// Optional width (scales image)
    #[serde(default)]
    pub width: Option<u32>,
    /// Optional height (scales image)
    #[serde(default)]
    pub height: Option<u32>,
}

/// Shorts transformation configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShortsConfig {
    /// Path to background video (looped if shorter than main video)
    #[serde(default)]
    pub background_video: Option<String>,
    /// Background video opacity (0.0 - 1.0), default 0.4
    #[serde(default = "default_bg_opacity")]
    pub background_opacity: f32,
    /// Output resolution width (default 1080)
    #[serde(default = "default_output_width")]
    pub output_width: u32,
    /// Output resolution height (default 1920)
    #[serde(default = "default_output_height")]
    pub output_height: u32,
    /// Blur amount for the base layer (default 20)
    #[serde(default = "default_blur")]
    pub base_blur: u32,
    /// Height of the main video in the center (default 1400)
    #[serde(default = "default_main_video_height")]
    pub main_video_height: u32,
    /// Zoom level for main video (0.5 = 50%, 0.7 = 70%, 1.0 = 100%)
    /// Shows this percentage of the center of the video
    #[serde(default = "default_zoom")]
    pub main_video_zoom: f32,
    /// Vertical offset for main video (negative = up, positive = down)
    /// Value in pixels from center position
    #[serde(default)]
    pub main_video_y_offset: i32,
    /// Image overlays with positions
    #[serde(default)]
    pub overlays: Vec<ImageOverlay>,
}

fn default_bg_opacity() -> f32 {
    0.4
}

fn default_output_width() -> u32 {
    1080
}

fn default_output_height() -> u32 {
    1920
}

fn default_blur() -> u32 {
    20
}

fn default_main_video_height() -> u32 {
    1400
}

fn default_zoom() -> f32 {
    0.7
}

impl Default for ShortsConfig {
    fn default() -> Self {
        Self {
            background_video: None,
            background_opacity: 0.4,
            output_width: 1080,
            output_height: 1920,
            base_blur: 20,
            main_video_height: 1400,
            main_video_zoom: 0.7,
            main_video_y_offset: -150,
            overlays: Vec::new(),
        }
    }
}

/// Application configuration stored in settings.json
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    /// Google Gemini API Keys (Rotated)
    #[serde(default)]
    pub google_api_keys: Vec<String>,
    /// Default output directory for generated shorts
    pub default_output_dir: String,
    /// Whether to automatically start extraction when moments are finished
    #[serde(default)]
    pub extract_shorts_when_finished_moments: bool,
    /// Whether to use cookies for yt-dlp
    #[serde(default)]
    pub use_cookies: bool,
    /// Path to the cookies file
    #[serde(default = "default_cookies_path")]
    pub cookies_path: String,
    /// Shorts transformation configuration
    #[serde(default)]
    pub shorts_config: ShortsConfig,
    /// Whether to use GPU acceleration (NVENC) for FFmpeg
    #[serde(default)]
    pub gpu_acceleration: Option<bool>,
}

fn default_cookies_path() -> String {
    "./cookies.json".to_string()
}

impl AppConfig {
    /// Configuration file name
    const CONFIG_PATH: &'static str = "settings.json";

    /// Load configuration from file
    pub fn load() -> Result<Self> {
        if !Path::new(Self::CONFIG_PATH).exists() {
            return Err(anyhow::anyhow!(
                "Configuration file not found. Please create settings.json"
            ));
        }

        let content = fs::read_to_string(Self::CONFIG_PATH)?;
        let config: AppConfig = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse settings.json: {}", e))?;

        if config.google_api_keys.is_empty() {
            return Err(anyhow::anyhow!(
                "No API keys found in configuration. Please add google_api_keys to settings.json"
            ));
        }

        Ok(config)
    }

    /// Create a default configuration file
    pub fn create_default() -> Result<()> {
        let default_config = AppConfig {
            google_api_keys: vec!["YOUR_API_KEY_HERE".to_string()],
            default_output_dir: "./output".to_string(),
            extract_shorts_when_finished_moments: false,
            use_cookies: false,
            cookies_path: "./cookies.json".to_string(),
            shorts_config: ShortsConfig::default(),
            gpu_acceleration: None,
        };

        let json = serde_json::to_string_pretty(&default_config)?;
        fs::write(Self::CONFIG_PATH, json)?;

        Ok(())
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(Self::CONFIG_PATH, json)?;
        Ok(())
    }

    /// Ensure output directory exists
    pub fn ensure_output_dir(&self) -> Result<()> {
        if !Path::new(&self.default_output_dir).exists() {
            fs::create_dir_all(&self.default_output_dir)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let config = AppConfig {
            google_api_keys: vec!["test-key-1".to_string(), "test-key-2".to_string()],
            default_output_dir: "./output".to_string(),
            extract_shorts_when_finished_moments: false,
            use_cookies: false,
            cookies_path: "./cookies.json".to_string(),
            shorts_config: ShortsConfig::default(),
            gpu_acceleration: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.google_api_keys, config.google_api_keys);
    }

    #[test]
    fn test_shorts_config_defaults() {
        let config = ShortsConfig::default();
        assert_eq!(config.background_opacity, 0.4);
        assert_eq!(config.output_width, 1080);
        assert_eq!(config.output_height, 1920);
    }

    #[test]
    fn test_image_overlay() {
        let overlay = ImageOverlay {
            path: "./frame.png".to_string(),
            x: 100,
            y: 200,
            width: Some(500),
            height: None,
        };
        let json = serde_json::to_string(&overlay).unwrap();
        let parsed: ImageOverlay = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.x, 100);
        assert_eq!(parsed.width, Some(500));
    }
}
