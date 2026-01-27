//! Google Gemini AI integration for YT ShortMaker
//! Handles video upload and AI analysis for identifying key moments

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::types::VideoMoment;

/// Gemini API client
pub struct GeminiClient {
    client: Client,
    api_keys: Vec<String>,
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
    pub fn new(api_keys: Vec<String>) -> Self {
        Self {
            client: Client::new(),
            api_keys,
            current_key_index: AtomicUsize::new(0),
            model: "gemini-3-flash-preview".to_string(),
        }
    }

    /// Get the current active key and rotate to the next one for future requests
    fn get_active_key(&self) -> &str {
        if self.api_keys.is_empty() {
            return "";
        }
        // Get current index
        let index = self.current_key_index.fetch_add(1, Ordering::SeqCst);
        // Modulo to wrap around
        &self.api_keys[index % self.api_keys.len()]
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
        let current_key = self.get_active_key();
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

        Ok(file_uri)
    }

    /// Wait for uploaded file to become active
    async fn wait_for_file_active(&self, file_name: &str) -> Result<String> {
        // We use the same key for checking status as we might want consistency,
        // but rotation is fine too as they are global resources.
        // Let's rotate to avoid hitting limits on check loops.
        let current_key = self.get_active_key();
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
        let current_key = self.get_active_key();
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, current_key
        );

        // Construct the JSON Schema for structured output
        // Based on request_example.txt
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
                        text: format!(
                            "Analyze this video chunk and identify the best moments for YouTube Shorts. Return timestamps relative to the start of this provided video chunk (00:00:00).",
                        ),
                    },
                ],
            }],
            system_instruction: SystemInstruction {
                parts: vec![TextPart {
                    text: SYSTEM_PROMPT.to_string(),
                }],
            },
            generation_config: GenerationConfig {
                temperature: Some(0.4), // Slightly creative but focused
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
            return Err(anyhow!("Gemini API error: {}", error.message));
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

        let cleaned = text
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        let analysis_response: AnalysisResponse =
            serde_json::from_str(cleaned).context("Failed to parse structured moments JSON")?;

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

                // Adjust dialogue timestamps too if present
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
