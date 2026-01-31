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

/// Google Gemini API client
pub struct GoogleClient {
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

impl GoogleClient {
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

    /// Process a video chunk: Upload and Analyze using the same key (Sticky Session)
    /// This ensures we don't try to analyze a file uploaded by Key A with Key B.
    /// It handles rewries by re-uploading if the key fails.
    pub async fn process_chunk<F>(
        &self,
        file_path: &str,
        chunk_start_offset: u64,
        status_callback: F,
    ) -> Result<Vec<VideoMoment>>
    where
        F: Fn(String),
    {
        loop {
            // Get a key
            let key_arc = self
                .get_active_key()
                .ok_or_else(|| anyhow!("No active API keys available"))?;
            let key_name = key_arc.name.clone();

            status_callback(format!("Uploading with {}...", key_name));

            // 1. Upload
            let file_uri = match self.upload_video_internal(&key_arc, file_path).await {
                Ok(uri) => uri,
                Err(e) => {
                    eprintln!("Upload failed with key {}: {}", key_name, e);
                    // If upload fails, check if it's a quota issue or just network
                    // For now, we rotate and retry loop
                    self.rotate_key();
                    continue;
                }
            };

            status_callback(format!("Analyzing with {}...", key_name));

            // 2. Analyze
            match self
                .analyze_video_internal(&key_arc, &file_uri, chunk_start_offset)
                .await
            {
                Ok(moments) => {
                    // Success!
                    self.rotate_key(); // Rotate for next chunk to spread load
                    return Ok(moments);
                }
                Err(e) => {
                    // Check error type
                    let err_msg = e.to_string();
                    let is_quota = err_msg.contains("quota")
                        || err_msg.contains("429")
                        || err_msg.contains("RESOURCE_EXHAUSTED");

                    if is_quota {
                        self.disable_key(&key_arc.value);
                        eprintln!("Disabling key {} due to quota during analysis.", key_name);
                        status_callback(format!("Key {} exhausted, switching...", key_name));
                        continue;
                    } else {
                        eprintln!("Analysis failed with key {}: {}", key_name, e);
                        self.rotate_key();
                        continue;
                    }
                }
            }
        }
    }

    async fn upload_video_internal(&self, key: &ClientKey, file_path: &str) -> Result<String> {
        let path = Path::new(file_path);
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("video.mp4");

        let file_content = fs::read(file_path).context("Failed to read video file")?;
        let file_size = file_content.len();

        let current_key = &key.value;

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

        // Wait for file to be processed with SAME KEY
        self.wait_for_file_active(key, &upload_result.file.name)
            .await?;

        Ok(upload_result.file.uri)
    }

    async fn wait_for_file_active(&self, key: &ClientKey, file_name: &str) -> Result<()> {
        let current_key = &key.value;
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
                return Ok(());
            }
            if file_info.state == "FAILED" {
                return Err(anyhow!("File processing failed"));
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }

        Err(anyhow!("File processing timed out"))
    }

    async fn analyze_video_internal(
        &self,
        key: &ClientKey,
        file_uri: &str,
        chunk_start_offset: u64,
    ) -> Result<Vec<VideoMoment>> {
        let key_value = &key.value;

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, key_value
        );

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

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to call Gemini API")?;

        let gemini_response: GeminiResponse = response
            .json()
            .await
            .context("Failed to parse Gemini response")?;

        if let Some(error) = gemini_response.error {
            return Err(anyhow!(
                "Gemini API error: {} (Code: {:?}, Status: {:?})",
                error.message,
                error.code,
                error.status
            ));
        }

        let text = gemini_response
            .candidates
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.content.parts.into_iter().next())
            .and_then(|p| p.text)
            .ok_or_else(|| anyhow!("No response from Gemini"))?;

        #[derive(Deserialize)]
        struct AnalysisResponse {
            moments: Vec<VideoMoment>,
        }

        let cleaning = text
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        let analysis_response: AnalysisResponse =
            serde_json::from_str(cleaning).context("Failed to parse structured moments JSON")?;

        let mut moments = analysis_response.moments;

        // Adjust timestamps based on chunk offset
        if chunk_start_offset > 0 {
            for moment in moments.iter_mut() {
                let start_secs = crate::video::parse_timestamp_to_seconds(&moment.start_time)
                    .unwrap_or(0)
                    + chunk_start_offset;
                let end_secs = crate::video::parse_timestamp_to_seconds(&moment.end_time)
                    .unwrap_or(0)
                    + chunk_start_offset;

                moment.start_time = crate::video::format_seconds_to_timestamp(start_secs);
                moment.end_time = crate::video::format_seconds_to_timestamp(end_secs);

                for dia in &mut moment.dialogue {
                    let d_start = crate::video::parse_timestamp_to_seconds(&dia.start_time)
                        .unwrap_or(0)
                        + chunk_start_offset;
                    let d_end = crate::video::parse_timestamp_to_seconds(&dia.end_time)
                        .unwrap_or(0)
                        + chunk_start_offset;
                    dia.start_time = crate::video::format_seconds_to_timestamp(d_start);
                    dia.end_time = crate::video::format_seconds_to_timestamp(d_end);
                }
            }
        }

        Ok(moments)
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
