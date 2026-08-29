// =================================================================================================
// security::keyring — OS credential store helpers
// =================================================================================================

use anyhow::{Context, Result};

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

pub fn set_secret(service: &str, key: &str, value: &str) -> Result<()> {
    let entry = keyring::Entry::new(service, key).context("creating keyring entry")?;
    entry.set_password(value).context("storing secret")?;
    Ok(())
}

pub fn get_secret(service: &str, key: &str) -> Result<String> {
    let entry = keyring::Entry::new(service, key).context("creating keyring entry")?;
    let val = entry.get_password().context("reading secret")?;
    Ok(val)
}

pub fn delete_secret(service: &str, key: &str) -> Result<()> {
    let entry = keyring::Entry::new(service, key).context("creating keyring entry")?;
    entry.delete_credential().context("deleting secret")?;
    Ok(())
}
