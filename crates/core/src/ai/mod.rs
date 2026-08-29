// =================================================================================================
// ai — Provider registry + module re-exports
// =================================================================================================

pub mod gemini;
pub mod provider;

pub use provider::{AiProvider, AnalyzeCtx, ProviderCapabilities, ProviderEvent, ProviderRegistry};
