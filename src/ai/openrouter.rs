use crate::types::VideoMoment;
use anyhow::{anyhow, Result};
use reqwest::Client;
#[allow(unused_imports)]
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::fs;

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Key wrapper
struct ClientKey {
    key: String,
}

pub struct OpenRouterClient {
    client: Client,
    api_keys: Vec<Arc<ClientKey>>,
    current_key_index: AtomicUsize,
    model: String,
}

#[derive(Deserialize)]
struct OpenRouterResponse {
    choices: Vec<OpenRouterChoice>,
    #[allow(dead_code)]
    error: Option<OpenRouterError>,
}

#[derive(Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterMessage,
}

#[derive(Deserialize)]
struct OpenRouterMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct OpenRouterError {
    #[allow(dead_code)]
    message: String,
}

/// Internal struct for parsing JSON from AI (which returns numbers for times)
#[derive(Deserialize)]
struct RawVideoMoment {
    start_time: f64,
    end_time: f64,
    category: String,
    description: String,
}

impl OpenRouterClient {
    pub fn new(api_keys: Vec<(String, String)>, model: String) -> Self {
        let keys = api_keys
            .into_iter()
            .map(|(_, k)| Arc::new(ClientKey { key: k }))
            .collect();

        Self {
            client: Client::new(),
            api_keys: keys,
            current_key_index: AtomicUsize::new(0),
            model,
        }
    }

    fn get_current_key(&self) -> Result<Arc<ClientKey>> {
        if self.api_keys.is_empty() {
            return Err(anyhow!("No OpenRouter API keys available"));
        }
        let index = self.current_key_index.load(Ordering::Relaxed);
        if index >= self.api_keys.len() {
            // Reset if out of bounds (shouldn't happen unless keys removed dynamically)
            self.current_key_index.store(0, Ordering::Relaxed);
            return Ok(self.api_keys[0].clone());
        }
        Ok(self.api_keys[index].clone())
    }

    fn rotate_key(&self) {
        if self.api_keys.len() > 1 {
            let next = (self.current_key_index.load(Ordering::Relaxed) + 1) % self.api_keys.len();
            self.current_key_index.store(next, Ordering::Relaxed);
        }
    }

    #[allow(deprecated)]
    pub async fn process_chunk<F>(
        &self,
        file_path: &str,
        chunk_start_offset: u64,
        status_callback: F,
    ) -> Result<Vec<VideoMoment>>
    where
        F: Fn(String),
    {
        status_callback("Reading and encoding video for OpenRouter...".to_string());

        // 1. Read file and base64 encode
        let video_data = fs::read(file_path).await?;
        let base64_video = base64::encode(&video_data);
        let data_uri = format!("data:video/mp4;base64,{}", base64_video);

        status_callback("Sending to OpenRouter...".to_string());

        let prompt = r#"
        Analyze this video chunk and identify engaging moments suitable for YouTube Shorts.
        For each moment, provide:
        - start_time: (in seconds, relative to the video start)
        - end_time: (in seconds)
        - category: (e.g., Funny, Insightful, Action)
        - description: Brief description of why it's good.

        Output ONLY JSON in this format:
        [
            {
                "start_time": 10.5,
                "end_time": 25.0,
                "category": "Funny",
                "description": "The host makes a hilarious joke."
            }
        ]
        If no suitable moments are found, return an empty list [].
        "#;

        let payload = json!({
            "model": self.model,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": prompt
                        },
                        {
                            "type": "video_url",
                            "video_url": {
                                "url": data_uri
                            }
                        }
                    ]
                }
            ]
        });

        // Try with retries/rotation
        let mut attempts = 0;
        let max_attempts = self.api_keys.len().max(3); // Try at least 3 times or number of keys

        while attempts < max_attempts {
            let key = self.get_current_key()?;

            let response = self
                .client
                .post(OPENROUTER_API_URL)
                .header("Authorization", format!("Bearer {}", key.key))
                .header(
                    "HTTP-Referer",
                    "https://github.com/vicentefelipechile/yt-shortmaker",
                ) // Required by OpenRouter
                .header("X-Title", "YT ShortMaker")
                .json(&payload)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let body: OpenRouterResponse = resp.json().await?;
                        if let Some(choice) = body.choices.first() {
                            if let Some(content) = &choice.message.content {
                                // Clean up markdown code blocks if present
                                let clean_content = content
                                    .trim()
                                    .trim_start_matches("```json")
                                    .trim_start_matches("```")
                                    .trim_end_matches("```")
                                    .trim();

                                match serde_json::from_str::<Vec<RawVideoMoment>>(clean_content) {
                                    Ok(raw_moments) => {
                                        let mut moments = Vec::new();
                                        for raw in raw_moments {
                                            let start = raw.start_time + chunk_start_offset as f64;
                                            let end = raw.end_time + chunk_start_offset as f64;
                                            moments.push(VideoMoment {
                                                start_time: format!("{:.2}", start),
                                                end_time: format!("{:.2}", end),
                                                category: raw.category,
                                                description: raw.description,
                                                dialogue: Vec::new(),
                                            });
                                        }
                                        return Ok(moments);
                                    }
                                    Err(e) => {
                                        // If JSON parsing fails
                                        log::error!("Failed to parse OpenRouter response: {}", e);
                                        log::debug!("Raw content: {}", content);
                                        return Ok(Vec::new());
                                    }
                                }
                            }
                        }
                        return Ok(Vec::new());
                    } else {
                        let status = resp.status();
                        let error_text = resp.text().await.unwrap_or_default();
                        log::warn!("OpenRouter Error ({}): {}", status, error_text);

                        if status.as_u16() == 429
                            || status.as_u16() == 401
                            || status.as_u16() == 402
                        {
                            // Rotate key
                            self.rotate_key();
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Request failed: {}", e);
                    self.rotate_key();
                }
            }
            attempts += 1;
        }

        Err(anyhow!(
            "Failed to process chunk with OpenRouter after multiple attempts"
        ))
    }
}
