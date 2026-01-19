//! Configuration management for AutoShorts-Rust-CLI
//! Handles loading and saving settings to settings.json

use anyhow::Result;
use dialoguer::{theme::ColorfulTheme, Input};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Application configuration stored in settings.json
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    /// Google Gemini API Keys (Rotated)
    #[serde(default)] // Default to empty if missing
    pub google_api_keys: Vec<String>,
    /// Default output directory for generated shorts
    pub default_output_dir: String,
    /// Whether to automatically start extraction when moments are finished
    pub extract_shorts_when_finished_moments: bool,
    /// Whether to use cookies for yt-dlp
    pub use_cookies: bool,
    /// Path to the cookies file
    pub cookies_path: String,
}

impl AppConfig {
    /// Configuration file name
    const CONFIG_PATH: &'static str = "settings.json";

    /// Load configuration from file or create new one interactively
    pub fn load_or_create() -> Result<Self> {
        // Check if file exists
        if Path::new(Self::CONFIG_PATH).exists() {
            let content = fs::read_to_string(Self::CONFIG_PATH)?;
            // Attempt to deserialize. If it fails (due to old format), we might want to prompt recreation or handle migration.
            // For simplicity in this CLI tool, if it fails to parse because of the key change, we'll suggest deleting it or let it fail.
            // However, let's try to be smart: if we can't parse as new config, maybe we can parse as old and migrate?
            // For now, let's stick to standard deserialization and if it fails, the user will likely see an error and we can guide them.
            // But wait, I can use a untyped Value to check? No, let's just make the user interaction robust.

            // NOTE: If the user has an old config, serde might fail if we don't alias.
            // Let's just expect the user to re-configure if they have the old version or handle the error gracefully elsewhere.
            // Actually, let's just implement the new structure.
            let config: AppConfig = serde_json::from_str(&content).map_err(|_| anyhow::anyhow!("Configuration file format has changed. Please delete settings.json and restart to re-configure."))?;

            if config.google_api_keys.is_empty() {
                return Err(anyhow::anyhow!(
                    "No API keys found in configuration. Please delete settings.json and restart."
                ));
            }

            return Ok(config);
        }

        // If not, prompt user for configuration
        println!("\n📋 Configuration file not found. Let's set it up!\n");

        let api_keys_input: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("🔑 Please enter your Google Gemini API Key(s) (separated by comma)")
            .interact_text()?;

        let api_keys: Vec<String> = api_keys_input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if api_keys.is_empty() {
            return Err(anyhow::anyhow!("At least one API key is required."));
        }

        let output_dir: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("📁 Enter default output folder")
            .default("./output".to_string())
            .interact_text()?;

        let new_config = AppConfig {
            google_api_keys: api_keys,
            default_output_dir: output_dir,
            extract_shorts_when_finished_moments: false,
            use_cookies: false,
            cookies_path: "./cookies.json".to_string(),
        };

        // Save to file
        new_config.save()?;

        println!("\n✅ Settings saved to {}\n", Self::CONFIG_PATH);
        Ok(new_config)
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
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.google_api_keys, config.google_api_keys);
    }
}
