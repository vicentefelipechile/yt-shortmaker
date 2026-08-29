// =================================================================================================
// config::store — Load/save helpers
// =================================================================================================

use std::path::PathBuf;

use anyhow::{Context, Result};

use super::AppConfig;

// -------------------------------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------------------------------

fn config_path() -> Result<PathBuf> {
    let dir = dirs::config_dir().context("could not resolve config_dir")?;
    Ok(dir.join(super::APP_DIR_NAME).join(super::SETTINGS_FILE))
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

pub fn load() -> Result<AppConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("reading config at {}", path.display()))?;
    let mut cfg: AppConfig = serde_json::from_str(&raw).context("parsing config json")?;
    cfg.normalize();
    Ok(cfg)
}

pub fn save(cfg: &AppConfig) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("creating config dir")?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(cfg).context("serializing config")?;
    std::fs::write(&tmp, json).context("writing tmp config")?;
    std::fs::rename(&tmp, &path).context("atomically moving config")?;
    Ok(())
}
