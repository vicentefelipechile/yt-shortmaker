// =================================================================================================
// ai — Provider registry + module re-exports
// =================================================================================================

pub mod gemini;
pub mod provider;

pub use provider::{AiProvider, AnalyzeCtx, ProviderCapabilities, ProviderEvent, ProviderRegistry};

use std::sync::Arc;

use crate::config::AiConfig;

/// Builds a `ProviderRegistry` from config, registering one provider per id.
/// Uses the active custom model (if any) so any model string valid for the base provider
/// (e.g. "gemma-3-27b-it" via base "gemini") works without code changes.
pub fn build_registry(cfg: &AiConfig) -> ProviderRegistry {
    let mut reg = ProviderRegistry::new(cfg.resolved_base_provider());
    // Resolve the model that should actually be sent to the API
    let resolved_model = cfg.resolved_model();
    let resolved_custom = cfg.active_custom_model().cloned();
    for (id, pcfg) in &cfg.providers {
        match id.as_str() {
            "gemini" => {
                let provider = if id == &cfg.resolved_base_provider() {
                    // Use the active custom model's overrides if it targets this provider
                    if let Some(ref cm) = resolved_custom {
                        gemini::GeminiProvider::new_with_model(
                            pcfg,
                            cm.model_id.clone(),
                            cm.temperature.or(pcfg.temperature),
                            cm.media_resolution
                                .clone()
                                .or_else(|| pcfg.media_resolution.clone()),
                        )
                    } else {
                        gemini::GeminiProvider::new_with_model(
                            pcfg,
                            resolved_model.clone(),
                            pcfg.temperature,
                            pcfg.media_resolution.clone(),
                        )
                    }
                } else {
                    gemini::GeminiProvider::new(pcfg)
                };
                reg.register(Arc::new(provider))
            }
            other => tracing::warn!("unknown provider id: {other}"),
        }
    }
    reg
}
