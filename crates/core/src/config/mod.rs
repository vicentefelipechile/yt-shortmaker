// =================================================================================================
// config — Application configuration (v2, no v1 compat)
// =================================================================================================

pub mod store;

use serde::{Deserialize, Serialize};

// -------------------------------------------------------------------------------------------------
// Constants
// -------------------------------------------------------------------------------------------------

pub const CONFIG_VERSION: u32 = 2;
pub const APP_DIR_NAME: &str = "yt-shortmaker-v2";
pub const SETTINGS_FILE: &str = "settings.json";

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: u32,
    pub language: String,
    pub output_dir: Option<String>,
    pub ai: AiConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            language: "en".to_owned(),
            output_dir: None,
            ai: AiConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub active_provider: String,
    pub providers: std::collections::HashMap<String, ProviderConfig>,
}

impl Default for AiConfig {
    fn default() -> Self {
        let mut providers = std::collections::HashMap::new();
        providers.insert(
            "gemini".to_owned(),
            ProviderConfig {
                keys: Vec::new(),
                model: "gemini-2.5-flash".to_owned(),
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
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyEntry {
    pub name: String,
    pub value: String,
    pub enabled: bool,
}
