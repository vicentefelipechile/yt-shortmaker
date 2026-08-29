// =================================================================================================
// security::keyring — OS keyring helpers (stub)
// =================================================================================================

use anyhow::Result;

pub fn set_secret(_service: &str, _key: &str, _value: &str) -> Result<()> {
    Ok(())
}

pub fn get_secret(_service: &str, _key: &str) -> Result<String> {
    anyhow::bail!("not implemented")
}
