// =================================================================================================
// ai::provider — Extensible AiProvider trait + registry
// =================================================================================================

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::types::VideoMoment;

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    pub supports_native_video: bool,
    pub supported_mimes: Vec<&'static str>,
    pub max_video_duration: Duration,
    pub max_inline_size_mb: u32,
}

#[derive(Debug, Clone)]
pub struct AnalyzeCtx {
    pub chunk_path: PathBuf,
    pub chunk_start_offset: Duration,
    pub cancellation: CancellationToken,
    pub progress_tx: Option<mpsc::UnboundedSender<ProviderEvent>>,
}

#[derive(Debug, Clone)]
pub enum ProviderEvent {
    Log(String),
    Progress(f32),
}

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn capabilities(&self) -> ProviderCapabilities;
    async fn analyze_chunk(&self, ctx: AnalyzeCtx) -> anyhow::Result<Vec<VideoMoment>>;
    async fn test_connection(&self) -> anyhow::Result<()>;
}

pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn AiProvider>>,
    active_id: String,
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

impl ProviderRegistry {
    pub fn new(active_id: String) -> Self {
        Self {
            providers: HashMap::new(),
            active_id,
        }
    }

    pub fn register(&mut self, provider: Arc<dyn AiProvider>) {
        self.providers.insert(provider.id().to_owned(), provider);
    }

    pub fn active(&self) -> Option<Arc<dyn AiProvider>> {
        self.providers.get(&self.active_id).cloned()
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn AiProvider>> {
        self.providers.get(id).cloned()
    }

    pub fn list(&self) -> Vec<Arc<dyn AiProvider>> {
        self.providers.values().cloned().collect()
    }

    pub fn active_id(&self) -> &str {
        &self.active_id
    }
}
