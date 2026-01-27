//! Configuration management for YT ShortMaker
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

/// API Key configuration with name and status
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApiKey {
    /// The actual API key string
    pub value: String,
    /// User-friendly name for identification
    #[serde(default = "default_key_name")]
    pub name: String,
    /// Whether this key is enabled for use
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_key_name() -> String {
    "Gemini Key".to_string()
}

fn default_true() -> bool {
    true
}

use crate::security::{EncryptionMode, SecuredConfig};

/// Application configuration stored in settings.json
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    /// Google Gemini API Keys (Rotated)
    #[serde(default, deserialize_with = "deserialize_api_keys")]
    pub google_api_keys: Vec<ApiKey>,
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

    // --- Google Drive Integration ---
    #[serde(default)]
    pub drive_enabled: bool,
    #[serde(default)]
    pub drive_auto_upload: bool,
    #[serde(default)]
    pub drive_folder_id: Option<String>,

    // Internal State for Security (Not saved to JSON body)
    #[serde(skip)]
    pub active_encryption_mode: EncryptionMode,
    #[serde(skip)]
    pub active_password: Option<String>,
}

fn default_cookies_path() -> String {
    "./cookies.json".to_string()
}

impl AppConfig {
    /// Configuration file name
    pub const CONFIG_PATH: &'static str = "settings.json";

    /// Load configuration from file (first attempt)
    pub fn load() -> Result<Self> {
        Self::load_with_password(None)
    }

    /// Load configuration with optional password
    pub fn load_with_password(password: Option<&str>) -> Result<Self> {
        if !Path::new(Self::CONFIG_PATH).exists() {
            return Err(anyhow::anyhow!(
                "Configuration file not found. Please create settings.json"
            ));
        }

        let content = fs::read_to_string(Self::CONFIG_PATH)?;

        // Try to parse as SecuredConfig first
        if let Ok(secured) = serde_json::from_str::<SecuredConfig>(&content) {
            match secured.decrypt(password) {
                Ok(decrypted) => {
                    let mut config: AppConfig = serde_json::from_str(&decrypted.content)
                        .map_err(|e| anyhow::anyhow!("Failed to parse decrypted config: {}", e))?;

                    config.active_encryption_mode = decrypted.mode;
                    config.active_password = password.map(|s| s.to_string());

                    Ok(config)
                }
                Err(e) => {
                    // Propagate error (e.g., "Password required")
                    Err(e)
                }
            }
        } else {
            // Fallback to legacy/plain JSON
            let mut config: AppConfig = serde_json::from_str(&content)
                .map_err(|e| anyhow::anyhow!("Failed to parse settings.json: {}", e))?;

            config.active_encryption_mode = EncryptionMode::None;
            config.active_password = None;

            // Validate basic sanity
            if config.google_api_keys.is_empty() {
                return Err(anyhow::anyhow!("No API keys found in configuration."));
            }
            Ok(config)
        }
    }

    /// Create a default configuration file
    pub fn create_default() -> Result<()> {
        let default_config = AppConfig {
            google_api_keys: vec![ApiKey {
                value: "YOUR_API_KEY_HERE".to_string(),
                name: "Primary Key".to_string(),
                enabled: true,
            }],
            default_output_dir: "./output".to_string(),
            extract_shorts_when_finished_moments: false,
            use_cookies: false,
            cookies_path: "./cookies.json".to_string(),
            shorts_config: ShortsConfig::default(),
            gpu_acceleration: None,
            drive_enabled: false,
            drive_auto_upload: false,
            drive_folder_id: None,
            active_encryption_mode: EncryptionMode::None,
            active_password: None,
        };

        // Save as plain text by default for new files
        default_config.save()?;

        Ok(())
    }

    /// Save configuration to file using active encryption mode
    pub fn save(&self) -> Result<()> {
        let json_content = serde_json::to_string_pretty(self)?;
        let secured = SecuredConfig::new(
            json_content,
            self.active_encryption_mode,
            self.active_password.as_deref(),
        )?;
        let file_content = serde_json::to_string_pretty(&secured)?;
        fs::write(Self::CONFIG_PATH, file_content)?;
        Ok(())
    }

    /// Helper to save current state preserving current mode would require knowing the current mode
    /// For now, we'll assume the caller knows the mode, or we default to 'Simple' if not specified?
    /// Actually, in the app flow we should store the 'active encryption mode' in memory.
    /// Ensure output directory exists
    pub fn ensure_output_dir(&self) -> Result<()> {
        if !Path::new(&self.default_output_dir).exists() {
            fs::create_dir_all(&self.default_output_dir)?;
        }
        Ok(())
    }
}

/// Custom deserializer to handle both Vec<String> (legacy) and Vec<ApiKey> (new)
fn deserialize_api_keys<'de, D>(deserializer: D) -> Result<Vec<ApiKey>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ApiKeysData {
        New(Vec<ApiKey>),
        Old(Vec<String>),
    }

    let data = ApiKeysData::deserialize(deserializer)?;
    match data {
        ApiKeysData::New(keys) => Ok(keys),
        ApiKeysData::Old(strings) => Ok(strings
            .into_iter()
            .enumerate()
            .map(|(i, s)| ApiKey {
                value: s,
                name: format!("Gemini Key {}", i + 1),
                enabled: true,
            })
            .collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_serialization() {
        let config = AppConfig {
            google_api_keys: vec![
                ApiKey {
                    value: "test-key-1".to_string(),
                    name: "Key 1".to_string(),
                    enabled: true,
                },
                ApiKey {
                    value: "test-key-2".to_string(),
                    name: "Key 2".to_string(),
                    enabled: true,
                },
            ],
            default_output_dir: "./output".to_string(),
            extract_shorts_when_finished_moments: false,
            use_cookies: false,
            cookies_path: "./cookies.json".to_string(),
            shorts_config: ShortsConfig::default(),
            gpu_acceleration: None,
            drive_enabled: false,
            drive_auto_upload: false,
            drive_folder_id: None,
            active_encryption_mode: EncryptionMode::None,
            active_password: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.google_api_keys[0].value,
            config.google_api_keys[0].value
        );
    }

    #[test]
    fn test_legacy_api_keys_migration() {
        let json = r#"{
            "google_api_keys": ["legacy_key_1", "legacy_key_2"],
            "default_output_dir": "./output"
        }"#;
        let parsed: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.google_api_keys.len(), 2);
        assert_eq!(parsed.google_api_keys[0].value, "legacy_key_1");
        assert_eq!(parsed.google_api_keys[0].name, "Gemini Key 1");
        assert_eq!(parsed.google_api_keys[0].enabled, true);
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
