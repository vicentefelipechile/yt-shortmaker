// =================================================================================================
// ai::gemini — GeminiProvider (native video)
// =================================================================================================

use std::time::Duration;

use async_trait::async_trait;

use super::provider::{AiProvider, AnalyzeCtx, ProviderCapabilities};
use crate::types::VideoMoment;

// -------------------------------------------------------------------------------------------------
// Constants
// -------------------------------------------------------------------------------------------------

const GEMINI_ID: &str = "gemini";
const GEMINI_DISPLAY: &str = "Google Gemini";

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

pub struct GeminiProvider {
    pub api_key: String,
    pub model: String,
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

impl GeminiProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
        }
    }
}

#[async_trait]
impl AiProvider for GeminiProvider {
    fn id(&self) -> &'static str {
        GEMINI_ID
    }

    fn display_name(&self) -> &'static str {
        GEMINI_DISPLAY
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_native_video: true,
            supported_mimes: vec!["video/mp4", "video/webm", "video/quicktime", "video/x-msvideo"],
            max_video_duration: Duration::from_secs(60 * 60),
            max_inline_size_mb: 20,
        }
    }

    async fn analyze_chunk(&self, ctx: AnalyzeCtx) -> anyhow::Result<Vec<VideoMoment>> {
        let _ = ctx;
        // TODO(M1): implement File API upload + generateContent with structured output.
        // For M0 this is a stub returning empty to allow wiring.
        Ok(Vec::new())
    }

    async fn test_connection(&self) -> anyhow::Result<()> {
        // TODO(M1): call models/{model}:generateContent with a trivial prompt.
        if self.api_key.is_empty() {
            anyhow::bail!("api key is empty");
        }
        Ok(())
    }
}
