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
    /// User-defined models. Each entry reuses a base provider (e.g. "gemini") but can point
    /// to any model string valid for that provider — e.g. "gemma-3-27b-it" or "gemini-3.7-flash".
    /// This makes the app future-proof: new Google models work without a code update.
    #[serde(default)]
    pub custom_models: Vec<CustomModel>,
    /// Id of the active custom model. If None, falls back to the provider's legacy effective_model.
    #[serde(default)]
    pub active_model_id: Option<String>,
}

impl Default for AiConfig {
    fn default() -> Self {
        let mut providers = HashMap::new();
        providers.insert(
            "gemini".to_owned(),
            ProviderConfig {
                keys: Vec::new(),
                // Legacy fields kept for migration; new code uses custom_models.
                model: default_model(),
                model_pro: default_model_pro(),
                use_fast_model: true,
                temperature: None,
                media_resolution: None,
            },
        );
        providers.insert(
            "openrouter".to_owned(),
            ProviderConfig {
                keys: Vec::new(),
                model: "google/gemini-2.5-flash".to_owned(),
                model_pro: "google/gemini-2.5-flash".to_owned(),
                use_fast_model: true,
                temperature: None,
                media_resolution: None,
            },
        );
        // Defaults verified 2026-08-28 via ai.google.dev/gemini-api/docs/models
        // gemini-3.7-flash is the current stable flagship; gemma models are accessible via the Gemini API.
        let custom_models = vec![
            CustomModel {
                id: "gemini-3.7-flash".to_owned(),
                display_name: "Gemini 3.7 Flash".to_owned(),
                base_provider: "gemini".to_owned(),
                model_id: "gemini-3.7-flash".to_owned(),
                temperature: None,
                media_resolution: None,
                enabled: true,
            },
            CustomModel {
                id: "gemini-3.6-flash".to_owned(),
                display_name: "Gemini 3.6 Flash".to_owned(),
                base_provider: "gemini".to_owned(),
                model_id: "gemini-3.6-flash".to_owned(),
                temperature: None,
                media_resolution: None,
                enabled: true,
            },
            CustomModel {
                id: "gemini-2.5-flash".to_owned(),
                display_name: "Gemini 2.5 Flash (legacy)".to_owned(),
                base_provider: "gemini".to_owned(),
                model_id: "gemini-2.5-flash".to_owned(),
                temperature: None,
                media_resolution: None,
                enabled: true,
            },
            CustomModel {
                id: "gemma-4-26b".to_owned(),
                display_name: "Gemma 4 26B (via Gemini API)".to_owned(),
                base_provider: "gemini".to_owned(),
                model_id: "gemma-4-26b-a4b-it".to_owned(),
                temperature: None,
                media_resolution: None,
                enabled: true,
            },
            CustomModel {
                id: "openrouter-gemini-2.5-flash".to_owned(),
                display_name: "Gemini 2.5 Flash (via OpenRouter)".to_owned(),
                base_provider: "openrouter".to_owned(),
                model_id: "google/gemini-2.5-flash".to_owned(),
                temperature: None,
                media_resolution: None,
                enabled: true,
            },
        ];
        Self {
            active_provider: "gemini".to_owned(),
            providers,
            custom_models,
            active_model_id: Some("gemini-3.7-flash".to_owned()),
        }
    }
}

impl AiConfig {
    /// Returns the active custom model, if any.
    pub fn active_custom_model(&self) -> Option<&CustomModel> {
        let id = self.active_model_id.as_deref()?;
        self.custom_models
            .iter()
            .find(|m| m.id == id && m.enabled)
            .or_else(|| self.custom_models.iter().find(|m| m.id == id))
    }

    /// Resolves the model string that should be sent to the provider.
    /// Prefers the active custom model; falls back to the legacy provider effective_model for
    /// configs that predate custom_models.
    pub fn resolved_model(&self) -> String {
        if let Some(m) = self.active_custom_model() {
            return m.model_id.clone();
        }
        if let Some(p) = self.providers.get(&self.active_provider) {
            return p.effective_model().to_owned();
        }
        "gemini-3.7-flash".to_owned()
    }

    /// Base provider that should handle the resolved model (e.g. "gemini" for both gemini-* and gemma-* via Gemini API).
    pub fn resolved_base_provider(&self) -> String {
        if let Some(m) = self.active_custom_model() {
            return m.base_provider.clone();
        }
        self.active_provider.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomModel {
    pub id: String,
    pub display_name: String,
    pub base_provider: String,
    pub model_id: String,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub media_resolution: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl CustomModel {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("model id is empty".into());
        }
        if self.display_name.trim().is_empty() {
            return Err("display name is empty".into());
        }
        if self.model_id.trim().is_empty() {
            return Err("model_id is empty".into());
        }
        if self.base_provider.trim().is_empty() {
            return Err("base_provider is empty".into());
        }
        if let Some(t) = self.temperature {
            if !(0.0..=2.0).contains(&t) {
                return Err("temperature must be 0.0-2.0".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub keys: Vec<ApiKeyEntry>,
    /// Fast model used when `use_fast_model` is true. Kept for migration from pre-custom-model configs.
    #[serde(default = "default_model")]
    pub model: String,
    /// Pro model used when `use_fast_model` is false. Kept for migration.
    #[serde(default = "default_model_pro")]
    pub model_pro: String,
    /// Toggle between fast and pro model. Kept for migration.
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
    /// Model selected by the fast/pro toggle (legacy).
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
    /// Also migrates legacy `model`/`model_pro` into `custom_models` when needed.
    pub fn normalize(&mut self) -> Vec<String> {
        let mut issues = Vec::new();

        if !(60..=7200).contains(&self.processing.chunk_size_secs) {
            self.processing.chunk_size_secs = DEFAULT_CHUNK_SIZE_SECS;
            issues.push("chunk_size out of range (1-120 min), reset to 10 min".to_owned());
        }
        if !(60..=3600).contains(&self.processing.max_last_chunk_secs) {
            self.processing.max_last_chunk_secs = DEFAULT_MAX_LAST_CHUNK_SECS;
            issues.push("max_last_chunk out of range (60-3600s), reset".to_owned());
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

        // Migrate legacy fast/pro into custom_models if none exist yet (v2 -> v2.1)
        if self.ai.custom_models.is_empty() {
            if let Some(p) = self.ai.providers.get(&self.ai.active_provider) {
                let base = self.ai.active_provider.clone();
                let mut migrated = Vec::new();
                if !p.model.trim().is_empty() {
                    migrated.push(CustomModel {
                        id: slugify(&p.model),
                        display_name: format!("{} (migrated)", p.model),
                        base_provider: base.clone(),
                        model_id: p.model.clone(),
                        temperature: p.temperature,
                        media_resolution: p.media_resolution.clone(),
                        enabled: true,
                    });
                }
                if !p.model_pro.trim().is_empty() && p.model_pro != p.model {
                    migrated.push(CustomModel {
                        id: slugify(&p.model_pro),
                        display_name: format!("{} (migrated)", p.model_pro),
                        base_provider: base.clone(),
                        model_id: p.model_pro.clone(),
                        temperature: p.temperature,
                        media_resolution: p.media_resolution.clone(),
                        enabled: true,
                    });
                }
                if !migrated.is_empty() {
                    let active_legacy = p.effective_model().to_owned();
                    self.ai.custom_models = migrated;
                    self.ai.active_model_id = Some(slugify(&active_legacy));
                    issues.push("migrated legacy model/model_pro into custom_models".into());
                }
            }
        }
        // Fallback to defaults if still empty (e.g. fresh install after prune)
        if self.ai.custom_models.is_empty() {
            let def = AiConfig::default();
            self.ai.custom_models = def.custom_models;
            self.ai.active_model_id = def.active_model_id;
            issues.push("custom_models was empty, restored defaults".into());
        }
        // Validate custom models
        let mut seen = std::collections::HashSet::new();
        let mut valid_models = Vec::new();
        for mut m in self.ai.custom_models.drain(..) {
            if m.id.trim().is_empty() {
                m.id = slugify(&m.model_id);
                issues.push(format!(
                    "custom model '{}' had empty id, set to '{}'",
                    m.display_name, m.id
                ));
            }
            if !seen.insert(m.id.clone()) {
                issues.push(format!("duplicate custom model id '{}', skipping", m.id));
                continue;
            }
            if let Err(e) = m.validate() {
                issues.push(format!("custom model '{}' invalid: {e}", m.id));
                continue;
            }
            // Normalize base_provider to lower
            m.base_provider = m.base_provider.to_lowercase();
            if !self.ai.providers.contains_key(&m.base_provider) {
                // Auto-create stub provider entry if user typed a new base
                self.ai.providers.insert(
                    m.base_provider.clone(),
                    ProviderConfig {
                        keys: Vec::new(),
                        ..Default::default()
                    },
                );
                issues.push(format!(
                    "created stub provider '{}' for custom model '{}'",
                    m.base_provider, m.id
                ));
            }
            valid_models.push(m);
        }
        self.ai.custom_models = valid_models;
        if self.ai.custom_models.is_empty() {
            let def = AiConfig::default();
            self.ai.custom_models = def.custom_models;
            self.ai.active_model_id = def.active_model_id;
            issues.push("all custom models invalid, restored defaults".into());
        }
        // Ensure active_model_id points to an existing enabled model
        let active_ok = self
            .ai
            .active_model_id
            .as_deref()
            .map(|id| self.ai.custom_models.iter().any(|m| m.id == id))
            .unwrap_or(false);
        if !active_ok {
            if let Some(first) = self
                .ai
                .custom_models
                .iter()
                .find(|m| m.enabled)
                .or(self.ai.custom_models.first())
            {
                let new_id = first.id.clone();
                self.ai.active_model_id = Some(new_id.clone());
                issues.push(format!(
                    "active_model_id missing/invalid, set to '{new_id}'"
                ));
            }
        }

        issues
    }
}

fn slugify(s: &str) -> String {
    crate::util::slugify(s)
}

pub fn slugify_pub(s: &str) -> String {
    slugify(s)
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
        // New extensible model system defaults to gemini-3.7-flash
        assert_eq!(cfg.ai.resolved_model(), "gemini-3.7-flash");
        assert_eq!(cfg.ai.custom_models.len(), 5);
        assert!(!cfg.cookies.use_cookies);
    }

    #[test]
    fn test_old_config_loads_with_defaults() {
        let old = r#"{"version":2,"language":"es","output_dir":null,"ai":{"active_provider":"gemini","providers":{"gemini":{"keys":[],"model":"gemini-2.5-flash"}}}}"#;
        let mut cfg: AppConfig = serde_json::from_str(old).unwrap();
        // Before normalize, custom_models is empty (old file); after normalize it migrates.
        assert!(cfg.ai.custom_models.is_empty());
        let issues = cfg.normalize();
        assert!(issues.iter().any(|s| s.contains("migrated")));
        assert_eq!(cfg.ai.custom_models.len(), 2);
        assert_eq!(cfg.ai.resolved_model(), "gemini-2.5-flash");
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

    #[test]
    fn test_custom_model_gemma_on_gemini() {
        let mut cfg = AppConfig::default();
        // User creates a custom model "gemma-3.6" on top of gemini provider
        let custom = CustomModel {
            id: "my-gemma".into(),
            display_name: "Gemma 3 27B custom".into(),
            base_provider: "gemini".into(),
            model_id: "gemma-3-27b-it".into(),
            temperature: Some(0.7),
            media_resolution: None,
            enabled: true,
        };
        assert!(custom.validate().is_ok());
        cfg.ai.custom_models.push(custom);
        cfg.ai.active_model_id = Some("my-gemma".into());
        assert_eq!(cfg.ai.resolved_model(), "gemma-3-27b-it");
        assert_eq!(cfg.ai.resolved_base_provider(), "gemini");
        // Also test gemma-4 via gemini
        let gemma4 = CustomModel {
            id: "gemma4".into(),
            display_name: "Gemma 4 26B".into(),
            base_provider: "gemini".into(),
            model_id: "gemma-4-26b-a4b-it".into(),
            temperature: None,
            media_resolution: None,
            enabled: true,
        };
        assert!(gemma4.validate().is_ok());
    }

    #[test]
    fn test_resolved_model_fallback() {
        let mut cfg = AppConfig::default();
        cfg.ai.active_model_id = Some("nonexistent".into());
        let issues = cfg.normalize();
        assert!(issues.iter().any(|s| s.contains("active_model_id")));
        assert!(cfg.ai.active_custom_model().is_some());
    }
}
