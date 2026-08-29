// =================================================================================================
// security — OS keyring helpers (Keyring mode)
// =================================================================================================
// v1 Simple obfuscation and Password (aes-gcm+argon2) modes were removed in v2.
// Only None (plain) and Keyring (OS credential store) remain. Use `keyring::*`.

pub mod keyring;

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityMode {
    None,
    Keyring,
}
