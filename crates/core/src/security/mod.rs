// =================================================================================================
// security — None / Keyring / Password modes (stub)
// =================================================================================================

pub mod keyring;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityMode {
    None,
    Keyring,
    Password,
}
