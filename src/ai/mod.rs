pub mod google;
pub mod openrouter;

use crate::types::VideoMoment;
use anyhow::Result;

pub use google::GoogleClient;
pub use openrouter::OpenRouterClient;

/// Wrapper enum for different AI providers
pub enum AiClient {
    Google(GoogleClient),
    OpenRouter(OpenRouterClient),
}

impl AiClient {
    pub async fn process_chunk<F>(
        &self,
        file_path: &str,
        chunk_start_offset: u64,
        status_callback: F,
    ) -> Result<Vec<VideoMoment>>
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        match self {
            AiClient::Google(client) => {
                client
                    .process_chunk(file_path, chunk_start_offset, status_callback)
                    .await
            }
            AiClient::OpenRouter(client) => {
                client
                    .process_chunk(file_path, chunk_start_offset, status_callback)
                    .await
            }
        }
    }
}
