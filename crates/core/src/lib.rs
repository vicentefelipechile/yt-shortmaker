// =================================================================================================
// yt-shortmaker-core — UI-agnostic business logic for v2
// =================================================================================================

rust_i18n::i18n!("../../locales", fallback = "en");

pub mod ai;
pub mod config;
pub mod debug;
pub mod media;
pub mod plano;
pub mod security;
pub mod session;
pub mod setup;
pub mod types;
pub mod util;

// Re-export commonly used items at crate root for convenience.

// -------------------------------------------------------------------------------------------------
// Constants
// -------------------------------------------------------------------------------------------------

pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

// -------------------------------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------------------------------

/// Returns the core crate version.
pub fn version() -> &'static str {
    CORE_VERSION
}
