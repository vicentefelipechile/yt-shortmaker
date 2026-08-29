// =================================================================================================
// ai — Provider registry + module re-exports
// =================================================================================================

pub mod gemini;
pub mod provider;

pub use provider::{AiProvider, AnalyzeCtx, ProviderCapabilities, ProviderEvent, ProviderRegistry};

use std::sync::Arc;

use crate::config::AiConfig;

/// Builds a `ProviderRegistry` from config, registering one provider per id.
pub fn build_registry(cfg: &AiConfig) -> ProviderRegistry {
    let mut reg = ProviderRegistry::new(cfg.active_provider.clone());
    for (id, pcfg) in &cfg.providers {
        match id.as_str() {
            "gemini" => reg.register(Arc::new(gemini::GeminiProvider::new(pcfg))),
            other => tracing::warn!("unknown provider id: {other}"),
        }
    }
    reg
}
