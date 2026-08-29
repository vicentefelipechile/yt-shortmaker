// =================================================================================================
// config — Application configuration (v2, no v1 compat)
// =================================================================================================

pub mod store;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// -------------------------------------------------------------------------------------------------
// Constants
// -------------------------------------------------------------------------------------------------

pub const CONFIG_VERSION: u32 = 2;
pub const APP_DIR_NAME: &str = "yt-shortmaker-v2";
pub const SETTINGS_FILE: &str = "settings.json";

pub const DEFAULT_CHUNK_SIZE_SECS: u64 = 600;
pub const DEFAULT_MAX_LAST_CHUNK_SECS: u64 = 900;
pub const DEFAULT_CHUNK_DELAY_SECS: u64 = 2;
pub const DEFAULT_RETRY_ATTEMPTS: u32 = 3;
pub const DEFAULT_NAMING_TEMPLATE: &str = "short_%Y%m%d_%H%M%S";

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CookiesConfig {
    #[serde(default)]
    pub use_cookies: bool,
    #[serde(default)]
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    /// Duration of each chunk in seconds (UI edits this in minutes).
    #[serde(default = "default_chunk_size_secs")]
    pub chunk_size_secs: u64,
    /// If the last chunk is <= this many seconds, merge it into the previous one.
    #[serde(default = "default_max_last_chunk_secs")]
    pub max_last_chunk_secs: u64,
    /// Delay between AI calls for consecutive chunks (rate limiting).
    #[serde(default = "default_chunk_delay_secs")]
    pub chunk_delay_secs: u64,
    /// Retry attempts per network operation (download, upload, generate).
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,
    /// Automatically extract shorts once moments are analyzed.
    #[serde(default)]
    pub auto_extract: bool,
}

fn default_chunk_size_secs() -> u64 {
    DEFAULT_CHUNK_SIZE_SECS
}

fn default_max_last_chunk_secs() -> u64 {
    DEFAULT_MAX_LAST_CHUNK_SECS
}

fn default_chunk_delay_secs() -> u64 {
    DEFAULT_CHUNK_DELAY_SECS
}

fn default_retry_attempts() -> u32 {
    DEFAULT_RETRY_ATTEMPTS
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            chunk_size_secs: DEFAULT_CHUNK_SIZE_SECS,
            max_last_chunk_secs: DEFAULT_MAX_LAST_CHUNK_SECS,
            chunk_delay_secs: DEFAULT_CHUNK_DELAY_SECS,
            retry_attempts: DEFAULT_RETRY_ATTEMPTS,
            auto_extract: false,
        }
    }
}

impl ProcessingConfig {
    /// UI-facing value: chunk size in whole minutes (rounded down, min 1).
    pub fn chunk_size_minutes(&self) -> u64 {
        (self.chunk_size_secs / 60).max(1)
    }

    pub fn set_chunk_size_minutes(&mut self, minutes: u64) {
        self.chunk_size_secs = (minutes.max(1) * 60).min(120 * 60);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    #[serde(default = "default_naming_template")]
    pub naming_template: String,
    /// Optional ffmpeg binary path override (falls back to PATH).
    #[serde(default)]
    pub ffmpeg_path: Option<String>,
}

fn default_naming_template() -> String {
    DEFAULT_NAMING_TEMPLATE.to_owned()
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            naming_template: DEFAULT_NAMING_TEMPLATE.to_owned(),
            ffmpeg_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: u32,
    pub language: String,
    pub output_dir: Option<String>,
    #[serde(default)]
    pub cookies: CookiesConfig,
    #[serde(default)]
    pub processing: ProcessingConfig,
    #[serde(default)]
    pub export: ExportConfig,
    pub ai: AiConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            language: "en".to_owned(),
            output_dir: None,
            cookies: CookiesConfig::default(),
            processing: ProcessingConfig::default(),
            export: ExportConfig::default(),
            ai: AiConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub active_provider: String,
    pub providers: HashMap<String, ProviderConfig>,
}

impl Default for AiConfig {
    fn default() -> Self {
        let mut providers = HashMap::new();
        providers.insert(
            "gemini".to_owned(),
            ProviderConfig {
                keys: Vec::new(),
                model: "gemini-2.5-flash".to_owned(),
                model_pro: "gemini-2.5-pro".to_owned(),
                use_fast_model: true,
                temperature: None,
                media_resolution: None,
            },
        );
        Self {
            active_provider: "gemini".to_owned(),
            providers,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub keys: Vec<ApiKeyEntry>,
    /// Fast model used when `use_fast_model` is true.
    #[serde(default = "default_model")]
    pub model: String,
    /// Pro model used when `use_fast_model` is false.
    #[serde(default = "default_model_pro")]
    pub model_pro: String,
    /// Toggle between fast and pro model.
    #[serde(default = "default_true")]
    pub use_fast_model: bool,
    /// Generation temperature (0.0 - 1.0). None = provider default.
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Gemini media resolution hint (e.g. "720P", "480P"). None = provider default.
    #[serde(default)]
    pub media_resolution: Option<String>,
}

fn default_model() -> String {
    "gemini-2.5-flash".to_owned()
}

fn default_model_pro() -> String {
    "gemini-2.5-pro".to_owned()
}

fn default_true() -> bool {
    true
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            model: default_model(),
            model_pro: default_model_pro(),
            use_fast_model: true,
            temperature: None,
            media_resolution: None,
        }
    }
}

impl ProviderConfig {
    /// Model selected by the fast/pro toggle.
    pub fn effective_model(&self) -> &str {
        if self.use_fast_model {
            &self.model
        } else {
            &self.model_pro
        }
    }

    pub fn enabled_keys(&self) -> impl Iterator<Item = &ApiKeyEntry> {
        self.keys.iter().filter(|k| k.enabled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyEntry {
    pub name: String,
    pub value: String,
    pub enabled: bool,
}

// -------------------------------------------------------------------------------------------------
// Validation
// -------------------------------------------------------------------------------------------------

impl AppConfig {
    /// Normalizes values into valid ranges; returns a list of issues found.
    pub fn normalize(&mut self) -> Vec<String> {
        let mut issues = Vec::new();

        if !(60..=7200).contains(&self.processing.chunk_size_secs) {
            self.processing.chunk_size_secs = DEFAULT_CHUNK_SIZE_SECS;
            issues.push("chunk_size out of range (1-120 min), reset to 10 min".to_owned());
        }
        if self.processing.max_last_chunk_secs > self.processing.chunk_size_secs {
            self.processing.max_last_chunk_secs = DEFAULT_MAX_LAST_CHUNK_SECS;
            issues.push("max_last_chunk > chunk_size, reset".to_owned());
        }
        if self.processing.retry_attempts == 0 {
            self.processing.retry_attempts = DEFAULT_RETRY_ATTEMPTS;
            issues.push("retry_attempts must be >= 1, reset".to_owned());
        }
        if self.export.naming_template.trim().is_empty() {
            self.export.naming_template = DEFAULT_NAMING_TEMPLATE.to_owned();
            issues.push("naming_template empty, reset".to_owned());
        }
        for (id, provider) in &mut self.ai.providers {
            if provider.model.trim().is_empty() {
                provider.model = default_model();
                issues.push(format!("provider {id}: empty model, reset"));
            }
            if provider.model_pro.trim().is_empty() {
                provider.model_pro = default_model_pro();
                issues.push(format!("provider {id}: empty model_pro, reset"));
            }
            if let Some(t) = provider.temperature {
                if !(0.0..=1.0).contains(&t) {
                    provider.temperature = None;
                    issues.push(format!("provider {id}: temperature out of range, reset"));
                }
            }
        }
        issues
    }
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.version, 2);
        assert_eq!(cfg.processing.chunk_size_secs, 600);
        assert_eq!(cfg.processing.chunk_size_minutes(), 10);
        assert_eq!(cfg.ai.active_provider, "gemini");
        let p = &cfg.ai.providers["gemini"];
        assert_eq!(p.effective_model(), "gemini-2.5-flash");
        assert!(!cfg.cookies.use_cookies);
    }

    #[test]
    fn test_old_config_loads_with_defaults() {
        let old = r#"{"version":2,"language":"es","output_dir":null,"ai":{"active_provider":"gemini","providers":{"gemini":{"keys":[],"model":"gemini-2.5-flash"}}}}"#;
        let cfg: AppConfig = serde_json::from_str(old).unwrap();
        assert_eq!(cfg.language, "es");
        assert_eq!(cfg.processing.chunk_size_secs, 600);
        assert_eq!(cfg.export.naming_template, "short_%Y%m%d_%H%M%S");
        let p = &cfg.ai.providers["gemini"];
        assert_eq!(p.model_pro, "gemini-2.5-pro");
        assert!(p.use_fast_model);
        assert!(p.temperature.is_none());
    }

    #[test]
    fn test_chunk_minutes_roundtrip() {
        let mut p = ProcessingConfig::default();
        p.set_chunk_size_minutes(25);
        assert_eq!(p.chunk_size_secs, 1500);
        assert_eq!(p.chunk_size_minutes(), 25);
        p.set_chunk_size_minutes(0);
        assert_eq!(p.chunk_size_minutes(), 1);
        p.set_chunk_size_minutes(999);
        assert_eq!(p.chunk_size_secs, 120 * 60);
    }

    #[test]
    fn test_fast_pro_models() {
        let p = ProviderConfig {
            use_fast_model: false,
            ..Default::default()
        };
        assert_eq!(p.effective_model(), "gemini-2.5-pro");
    }

    #[test]
    fn test_normalize() {
        let mut cfg = AppConfig::default();
        cfg.processing.chunk_size_secs = 5;
        cfg.processing.retry_attempts = 0;
        cfg.ai.providers.get_mut("gemini").unwrap().temperature = Some(5.0);
        let issues = cfg.normalize();
        assert!(!issues.is_empty());
        assert_eq!(cfg.processing.chunk_size_secs, 600);
        assert_eq!(cfg.processing.retry_attempts, 3);
        assert!(cfg.ai.providers["gemini"].temperature.is_none());
    }

    #[test]
    fn test_enabled_keys_filter() {
        let p = ProviderConfig {
            keys: vec![
                ApiKeyEntry {
                    name: "a".into(),
                    value: "1".into(),
                    enabled: true,
                },
                ApiKeyEntry {
                    name: "b".into(),
                    value: "2".into(),
                    enabled: false,
                },
            ],
            ..Default::default()
        };
        let enabled: Vec<_> = p.enabled_keys().map(|k| k.name.as_str()).collect();
        assert_eq!(enabled, vec!["a"]);
    }
}
