// =================================================================================================
// security — Encryption modes and helpers
// =================================================================================================

pub mod keyring;

use zeroize::Zeroize;

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityMode {
    None,
    Keyring,
    Password,
}

#[derive(Debug, Clone)]
pub struct Secret(pub String);

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}
