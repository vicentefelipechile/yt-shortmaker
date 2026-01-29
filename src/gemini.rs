//! Google Gemini AI integration for YT ShortMaker
//! Handles video upload and AI analysis for identifying key moments

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::types::VideoMoment;

/// API Key status tracker
#[derive(Debug)]
struct ClientKey {
    name: String,
    value: String,
    enabled: AtomicBool,
}

/// Gemini API client
pub struct GeminiClient {
    client: Client,
    api_keys: Vec<Arc<ClientKey>>,
    current_key_index: AtomicUsize,
    model: String,
}

// Response schema definitions
#[derive(Debug, Serialize)]
struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(rename = "responseMimeType")]
    response_mime_type: String,
    #[serde(rename = "responseSchema")]
    response_schema: Option<ResponseSchema>,
    #[serde(rename = "mediaResolution")]
    media_resolution: Option<String>,
}

#[derive(Debug, Serialize)]
struct ResponseSchema {
    #[serde(rename = "type")]
    schema_type: String,
    properties: serde_json::Value,
    required: Vec<String>,
}

/// System prompt for video analysis
const SYSTEM_PROMPT: &str = r#"You are a professional video editor assistant. Your task is to analyze the provided video chunk and identify the best moments suitable for YouTube Shorts.

Identify moments that fit these categories:
- Funny
- Interesting
- Incredible Play
- Other

Constraints:
1. Duration: 10 seconds to 90 seconds.
2. Provide a brief description.
3. Use timestamp format "HH:MM:SS".
4. Include any memorable dialogue in the 'dialogue' field.

If no suitable moments are found, return an empty array in the moments field."#;

/// Response from Gemini API
#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<Candidate>>,
    error: Option<GeminiError>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: Content,
}

#[derive(Debug, Deserialize)]
struct Content {
    parts: Vec<ContentPart>,
}

#[derive(Debug, Deserialize)]
struct ContentPart {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiError {
    message: String,
    code: Option<i32>,
    status: Option<String>,
}

/// File upload response
#[derive(Debug, Deserialize)]
struct UploadResponse {
    file: FileInfo,
}

#[derive(Debug, Deserialize)]
struct FileInfo {
    uri: String,
    name: String,
    #[allow(dead_code)]
    #[serde(rename = "mimeType")]
    mime_type: String,
    state: String,
}

/// Request body for generate content
#[derive(Debug, Serialize)]
struct GenerateContentRequest {
    contents: Vec<ContentRequest>,
    #[serde(rename = "systemInstruction")]
    system_instruction: SystemInstruction,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Debug, Serialize)]
struct ContentRequest {
    parts: Vec<PartRequest>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum PartRequest {
    Text {
        text: String,
    },
    FileData {
        #[serde(rename = "fileData")]
        file_data: FileData,
    },
}

#[derive(Debug, Serialize)]
struct FileData {
    #[serde(rename = "mimeType")]
    mime_type: String,
    #[serde(rename = "fileUri")]
    file_uri: String,
}

#[derive(Debug, Serialize)]
struct SystemInstruction {
    parts: Vec<TextPart>,
}

#[derive(Debug, Serialize)]
struct TextPart {
    text: String,
}

impl GeminiClient {
    /// Create a new Gemini client
    pub fn new(api_keys: Vec<(String, String)>, use_fast_model: bool) -> Self {
        let model = if use_fast_model {
            "gemini-3-flash-preview".to_string()
        } else {
            "gemini-3-pro-preview".to_string()
        };

        let keys = api_keys
            .into_iter()
            .map(|(name, value)| {
                Arc::new(ClientKey {
                    name,
                    value,
                    enabled: AtomicBool::new(true),
                })
            })
            .collect();

        Self {
            client: Client::new(),
            api_keys: keys,
            current_key_index: AtomicUsize::new(0),
            model,
        }
    }

    /// Get the current active key and rotate to the next active one.
    /// Checks if key is enabled.
    fn get_active_key(&self) -> Option<Arc<ClientKey>> {
        if self.api_keys.is_empty() {
            return None;
        }

        let start_index = self.current_key_index.load(Ordering::SeqCst);
        let mut attempts = 0;
        let total_keys = self.api_keys.len();

        loop {
            if attempts >= total_keys {
                return None; // All keys disabled
            }

            let index = (start_index + attempts) % total_keys;
            let key = &self.api_keys[index];

            if key.enabled.load(Ordering::SeqCst) {
                // Determine if we should rotate for next call (simple round robin among active)
                // But for now, we just return the first active one we find starting from current index
                return Some(key.clone());
            }

            attempts += 1;
        }
    }

    /// Rotate to next key explicitly (e.g. after a success or before next request)
    fn rotate_key(&self) {
        self.current_key_index.fetch_add(1, Ordering::SeqCst);
    }

    /// Disable the specified key
    fn disable_key(&self, key_value: &str) {
        if let Some(key) = self.api_keys.iter().find(|k| k.value == key_value) {
            key.enabled.store(false, Ordering::SeqCst);
            eprintln!(
                "⚠️ WARN: API Key '{}' has been disabled due to errors.",
                key.name
            );
        }
        // Rotate immediately to avoid picking it up again in same loop if race condition
        self.rotate_key();
    }

    /// Upload a video file to Gemini File API
    pub async fn upload_video(&self, file_path: &str) -> Result<String> {
        let path = Path::new(file_path);
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("video.mp4");

        let file_content = fs::read(file_path).context("Failed to read video file")?;
        let file_size = file_content.len();

        // Step 1: Initiate resumable upload
        // For upload, we just pick an active key. If it fails, we fail the upload for now.
        // Implementing full failover for upload is possible but analysis is the critical part.
        let key_arc = self
            .get_active_key()
            .ok_or_else(|| anyhow!("No active API keys available"))?;
        let current_key = &key_arc.value;

        let init_url = format!(
            "https://generativelanguage.googleapis.com/upload/v1beta/files?key={}",
            current_key
        );

        let init_response = self
            .client
            .post(&init_url)
            .header("X-Goog-Upload-Protocol", "resumable")
            .header("X-Goog-Upload-Command", "start")
            .header("X-Goog-Upload-Header-Content-Length", file_size.to_string())
            .header("X-Goog-Upload-Header-Content-Type", "video/mp4")
            .header("Content-Type", "application/json")
            .body(format!(
                r#"{{"file": {{"display_name": "{}"}}}}"#,
                file_name
            ))
            .send()
            .await
            .context("Failed to initiate upload")?;

        let upload_url = init_response
            .headers()
            .get("x-goog-upload-url")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("No upload URL in response"))?;

        // Step 2: Upload the file
        let upload_response = self
            .client
            .post(&upload_url)
            .header("X-Goog-Upload-Offset", "0")
            .header("X-Goog-Upload-Command", "upload, finalize")
            .header("Content-Length", file_size.to_string())
            .body(file_content)
            .send()
            .await
            .context("Failed to upload video")?;

        let upload_result: UploadResponse = upload_response
            .json()
            .await
            .context("Failed to parse upload response")?;

        // Wait for file to be processed
        let file_uri = self.wait_for_file_active(&upload_result.file.name).await?;

        // Rotate key after successful heavy operation
        self.rotate_key();

        Ok(file_uri)
    }

    /// Wait for uploaded file to become active
    async fn wait_for_file_active(&self, file_name: &str) -> Result<String> {
        // We use the same key mechanism
        let key_arc = self
            .get_active_key()
            .ok_or_else(|| anyhow!("No active API keys available"))?;
        let current_key = &key_arc.value;

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/{}?key={}",
            file_name, current_key
        );

        for _ in 0..60 {
            let response = self
                .client
                .get(&url)
                .send()
                .await
                .context("Failed to check file status")?;

            let file_info: FileInfo = response
                .json()
                .await
                .context("Failed to parse file status")?;

            if file_info.state == "ACTIVE" {
                return Ok(file_info.uri);
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }

        Err(anyhow!("File processing timed out"))
    }

    /// Analyze video and extract key moments
    pub async fn analyze_video(
        &self,
        file_uri: &str,
        chunk_start_offset: u64,
    ) -> Result<Vec<VideoMoment>> {
        // Construct the JSON Schema for structured output
        let response_schema = ResponseSchema {
            schema_type: "object".to_string(),
            properties: serde_json::json!({
                "moments": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "start_time": { "type": "string" },
                            "end_time": { "type": "string" },
                            "category": {
                                "type": "string",
                                "enum": ["Funny", "Interesting", "Incredible Play", "Cinematic", "Other"]
                            },
                            "description": { "type": "string" },
                            "dialogue": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "start_time": { "type": "string" },
                                        "end_time": { "type": "string" },
                                        "phrase": { "type": "string" }
                                    },
                                    "required": ["start_time", "end_time", "phrase"]
                                }
                            }
                        },
                        "required": ["start_time", "end_time", "category", "description"]
                    }
                }
            }),
            required: vec!["moments".to_string()],
        };

        let request = GenerateContentRequest {
            contents: vec![ContentRequest {
                parts: vec![
                    PartRequest::FileData {
                        file_data: FileData {
                            mime_type: "video/mp4".to_string(),
                            file_uri: file_uri.to_string(),
                        },
                    },
                    PartRequest::Text {
                        text: "Analyze this video chunk and identify the best moments for YouTube Shorts. Return timestamps relative to the start of this provided video chunk (00:00:00).".to_string(),
                    },
                ],
            }],
            system_instruction: SystemInstruction {
                parts: vec![TextPart {
                    text: SYSTEM_PROMPT.to_string(),
                }],
            },
            generation_config: GenerationConfig {
                temperature: Some(0.4),
                response_mime_type: "application/json".to_string(),
                response_schema: Some(response_schema),
                media_resolution: Some("MEDIA_RESOLUTION_LOW".to_string()),
            },
        };

        // Retry loop for API keys
        loop {
            // Get a key
            let (key_value, key_name) = match self.get_active_key() {
                Some(k) => (k.value.clone(), k.name.clone()),
                None => return Err(anyhow!("No API keys available")),
            };

            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
                self.model, key_value
            );

            let response_result = self
                .client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&request)
                .send()
                .await;

            match response_result {
                Ok(response) => {
                    let gemini_response: GeminiResponse = match response.json().await {
                        Ok(r) => r,
                        Err(e) => {
                            // JSON parse error might mean weird response text or server error
                            eprintln!("Failed to parse Gemini response: {}", e);
                            self.rotate_key(); // Try next key just in case
                            continue;
                        }
                    };

                    if let Some(error) = gemini_response.error {
                        // Check for 429 or similar
                        // 429 Resource Exhausted
                        // code 429 or status "RESOURCE_EXHAUSTED"
                        let is_limit = error.code == Some(429)
                            || error.status.as_deref() == Some("RESOURCE_EXHAUSTED")
                            || error.message.contains("quota");

                        if is_limit {
                            self.disable_key(&key_value);
                            eprintln!(
                                "Disabling key {} due to quota limit: {}",
                                key_name, error.message
                            );
                            continue; // Retry loop will pick next key
                        } else {
                            // Other error
                            return Err(anyhow!("Gemini API error: {}", error.message));
                        }
                    }

                    match gemini_response
                        .candidates
                        .and_then(|c| c.into_iter().next())
                        .and_then(|c| c.content.parts.into_iter().next())
                        .and_then(|p| p.text)
                    {
                        Some(text) => {
                            let cleaning = text
                                .trim()
                                .trim_start_matches("```json")
                                .trim_start_matches("```")
                                .trim_end_matches("```")
                                .trim();

                            #[derive(Deserialize)]
                            struct AnalysisResponse {
                                moments: Vec<VideoMoment>,
                            }

                            if let Ok(analysis_response) =
                                serde_json::from_str::<AnalysisResponse>(cleaning)
                            {
                                let mut moments = analysis_response.moments;
                                // Adjust timestamps
                                if chunk_start_offset > 0 {
                                    for moment in moments.iter_mut() {
                                        let start_secs = crate::video::parse_timestamp_to_seconds(
                                            &moment.start_time,
                                        )
                                        .unwrap_or(0)
                                            + chunk_start_offset;
                                        let end_secs = crate::video::parse_timestamp_to_seconds(
                                            &moment.end_time,
                                        )
                                        .unwrap_or(0)
                                            + chunk_start_offset;

                                        moment.start_time =
                                            crate::video::format_seconds_to_timestamp(start_secs);
                                        moment.end_time =
                                            crate::video::format_seconds_to_timestamp(end_secs);

                                        for dia in &mut moment.dialogue {
                                            let d_start = crate::video::parse_timestamp_to_seconds(
                                                &dia.start_time,
                                            )
                                            .unwrap_or(0)
                                                + chunk_start_offset;
                                            let d_end = crate::video::parse_timestamp_to_seconds(
                                                &dia.end_time,
                                            )
                                            .unwrap_or(0)
                                                + chunk_start_offset;
                                            dia.start_time =
                                                crate::video::format_seconds_to_timestamp(d_start);
                                            dia.end_time =
                                                crate::video::format_seconds_to_timestamp(d_end);
                                        }
                                    }
                                }
                                self.rotate_key(); // Success, rotate for next time
                                return Ok(moments);
                            } else {
                                // Parse error
                                eprintln!("Failed to parse structured moments JSON");
                                self.rotate_key();
                                continue;
                            }
                        }
                        None => {
                            // Empty response
                            return Err(anyhow!("No response content from Gemini"));
                        }
                    }
                }
                Err(e) => {
                    // Network error (timeout, etc)
                    // We assume timeout might be key related or transient?
                    // Usually timeouts are not key related, but let's try next key.
                    eprintln!("Network error with key {}: {}", key_name, e);
                    self.rotate_key();
                    continue;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_moments_json() {
        let json = r#"[{"start_time": "00:05:20", "end_time": "00:06:10", "category": "Funny", "description": "Player falls."}]"#;
        let moments: Vec<VideoMoment> = serde_json::from_str(json).unwrap();
        assert_eq!(moments.len(), 1);
        assert_eq!(moments[0].category, "Funny");
        assert!(moments[0].dialogue.is_empty());
    }

    #[test]
    fn test_parse_structured_response() {
        let json = r#"{
          "moments": [
            {
              "start_time": "02:33",
              "end_time": "03:01",
              "category": "Cinematic",
              "description": "Description text.",
              "dialogue": [
                {
                  "start_time": "02:33.500",
                  "end_time": "02:37.000",
                  "phrase": "Hello world"
                }
              ]
            }
          ]
        }"#;

        #[derive(Deserialize)]
        struct AnalysisResponse {
            moments: Vec<VideoMoment>,
        }

        let response: AnalysisResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.moments.len(), 1);
        assert_eq!(response.moments[0].category, "Cinematic");
        assert_eq!(response.moments[0].dialogue.len(), 1);
        assert_eq!(response.moments[0].dialogue[0].phrase, "Hello world");
    }
}
