// =================================================================================================
// ai::gemini — GeminiProvider (native video analysis)
// =================================================================================================

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::json;

use super::provider::{AiProvider, AnalyzeCtx, ProviderCapabilities, ProviderEvent};
use crate::config::ProviderConfig;
use crate::types::{
    format_seconds_to_timestamp, parse_timestamp_to_seconds, DialoguePhrase, VideoMoment,
};

fn send_log(ctx: &AnalyzeCtx, msg: impl Into<String>) {
    if let Some(tx) = &ctx.progress_tx {
        let _ = tx.send(ProviderEvent::Log(msg.into()));
    }
}
fn send_progress(ctx: &AnalyzeCtx, p: f32) {
    if let Some(tx) = &ctx.progress_tx {
        let _ = tx.send(ProviderEvent::Progress(p.clamp(0.0, 1.0)));
    }
}

// -------------------------------------------------------------------------------------------------
// Constants
// -------------------------------------------------------------------------------------------------

const GEMINI_ID: &str = "gemini";
const GEMINI_DISPLAY: &str = "Google Gemini";
const API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";
const UPLOAD_BASE: &str = "https://generativelanguage.googleapis.com/upload/v1beta";
const FILE_ACTIVE_TIMEOUT: Duration = Duration::from_secs(60);
const FILE_POLL_INTERVAL: Duration = Duration::from_secs(2);
const CHUNK_MIME: &str = "video/mp4";

const SYSTEM_PROMPT: &str = r#"You are a professional video editor assistant. Your task is to analyze the provided
video chunk and identify the best moments suitable for YouTube Shorts.
1. Duration: 10 seconds to 90 seconds per moment.
2. Each moment must be self-contained (start and end timestamps).
3. Prioritize: hooks, funny/emotional moments, strong statements, plot twists, valuable info.
4. Return timestamps in HH:MM:SS format, relative to the start of the provided video chunk."#;

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

pub struct GeminiProvider {
    keys: Vec<(String, String)>, // (name, value)
    model: String,
    temperature: Option<f32>,
    media_resolution: Option<String>,
    /// Indices of keys disabled after 429/quota errors.
    disabled: Mutex<std::collections::HashSet<usize>>,
    cursor: AtomicUsize,
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

impl GeminiProvider {
    pub fn new(cfg: &ProviderConfig) -> Self {
        let keys = cfg
            .enabled_keys()
            .map(|k| (k.name.clone(), k.value.clone()))
            .collect();
        Self {
            keys,
            model: cfg.effective_model().to_owned(),
            temperature: cfg.temperature,
            media_resolution: cfg.media_resolution.clone(),
            disabled: Mutex::new(std::collections::HashSet::new()),
            cursor: AtomicUsize::new(0),
        }
    }

    /// Next usable key index (round-robin, skipping disabled keys).
    fn next_key_index(&self) -> Option<usize> {
        let n = self.keys.len();
        if n == 0 {
            return None;
        }
        let disabled = self.disabled.lock().unwrap();
        for _ in 0..n {
            let idx = self.cursor.fetch_add(1, Ordering::Relaxed) % n;
            if !disabled.contains(&idx) {
                return Some(idx);
            }
        }
        None
    }

    fn mark_disabled(&self, idx: usize) {
        self.disabled.lock().unwrap().insert(idx);
    }

    /// Uploads the chunk file and returns (file_name, file_uri).
    async fn upload_file(&self, ctx: &AnalyzeCtx, key: &str) -> Result<(String, String)> {
        send_log(ctx, format!("Uploading {}", ctx.chunk_path.display()));
        let bytes = tokio::fs::read(&ctx.chunk_path)
            .await
            .with_context(|| format!("reading chunk {}", ctx.chunk_path.display()))?;

        let metadata = json!({ "file": { "display_name": "chunk.mp4", "mime_type": CHUNK_MIME } });
        let meta_part = reqwest::multipart::Part::text(metadata.to_string())
            .mime_str("application/json")
            .context("building metadata part")?;
        let file_part = reqwest::multipart::Part::bytes(bytes)
            .file_name("chunk.mp4")
            .mime_str(CHUNK_MIME)
            .context("building file part")?;
        let form = reqwest::multipart::Form::new()
            .part("metadata", meta_part)
            .part("file", file_part);

        let resp = reqwest::Client::new()
            .post(format!("{UPLOAD_BASE}/files?key={key}"))
            .multipart(form)
            .send()
            .await
            .context("uploading chunk to Gemini File API")?;

        let status = resp.status();
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("upload failed ({status}): {}", body);
        }
        let name = body["file"]["name"]
            .as_str()
            .ok_or_else(|| anyhow!("upload response missing file name: {body}"))?;
        let uri = body["file"]["uri"]
            .as_str()
            .ok_or_else(|| anyhow!("upload response missing file uri: {body}"))?;
        send_log(ctx, format!("Upload done: {name}"));
        send_progress(ctx, 0.3);
        Ok((name.to_owned(), uri.to_owned()))
    }

    /// Polls until the uploaded file is ACTIVE (ready for analysis).
    async fn wait_for_file_active(
        &self,
        ctx: &AnalyzeCtx,
        file_name: &str,
        key: &str,
    ) -> Result<()> {
        send_log(
            ctx,
            format!("Waiting for file {file_name} to become ACTIVE"),
        );
        let client = reqwest::Client::new();
        let deadline = tokio::time::Instant::now() + FILE_ACTIVE_TIMEOUT;
        loop {
            let resp = client
                .get(format!("{API_BASE}/files/{file_name}?key={key}"))
                .send()
                .await
                .context("querying file state")?;
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let state = body["file"]["state"].as_str().unwrap_or("");
            match state {
                "ACTIVE" => {
                    send_log(ctx, "File ACTIVE");
                    send_progress(ctx, 0.6);
                    return Ok(());
                }
                "FAILED" => anyhow::bail!("file processing failed: {body}"),
                _ => {
                    if tokio::time::Instant::now() >= deadline {
                        anyhow::bail!("timed out waiting for file to become ACTIVE");
                    }
                    tokio::time::sleep(FILE_POLL_INTERVAL).await;
                }
            }
        }
    }

    /// Runs generateContent for one chunk; returns parsed, offset moments.
    async fn generate_moments(
        &self,
        ctx: &AnalyzeCtx,
        file_uri: &str,
        key: &str,
    ) -> Result<Vec<VideoMoment>> {
        send_log(ctx, "Calling generateContent");
        let user_prompt =
            "Analyze this video chunk and identify the best moments for YouTube Shorts. \
             Return timestamps relative to the start of this provided video chunk (00:00:00)."
                .to_string();

        let mut generation_config = json!({
            "responseMimeType": "application/json",
            "responseSchema": response_schema(),
        });
        if let Some(t) = self.temperature {
            generation_config["temperature"] = json!(t);
        }
        if let Some(res) = &self.media_resolution {
            generation_config["mediaResolution"] = json!(res);
        }

        let payload = json!({
            "contents": [{
                "role": "user",
                "parts": [
                    { "text": user_prompt },
                    { "inlineData": { "fileUri": file_uri, "mimeType": CHUNK_MIME } }
                ]
            }],
            "systemInstruction": { "parts": [{ "text": SYSTEM_PROMPT }] },
            "generationConfig": generation_config,
        });

        let resp = reqwest::Client::new()
            .post(format!(
                "{API_BASE}/models/{}:generateContent?key={key}",
                self.model
            ))
            .json(&payload)
            .send()
            .await
            .context("calling generateContent")?;

        let status = resp.status();
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("generateContent failed ({status}): {body}");
        }

        let text = body["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| anyhow!("generateContent response missing text: {body}"))?;

        let parsed: serde_json::Value =
            serde_json::from_str(text).context("parsing structured moments json")?;
        let moments = parsed["moments"]
            .as_array()
            .ok_or_else(|| anyhow!("moments array missing in response"))?;

        let offset = ctx.chunk_start_offset.as_secs();
        let mut out = Vec::new();
        for m in moments {
            let start_time = m["start_time"].as_str().unwrap_or("");
            let end_time = m["end_time"].as_str().unwrap_or("");
            if start_time.is_empty() || end_time.is_empty() {
                continue;
            }
            out.push(VideoMoment {
                start_time: offset_timestamp(start_time, offset),
                end_time: offset_timestamp(end_time, offset),
                category: m["category"].as_str().unwrap_or("Other").to_owned(),
                description: m["description"].as_str().unwrap_or("").to_owned(),
                dialogue: parse_dialogue(m.get("dialogue")),
            });
        }
        Ok(out)
    }

    async fn analyze_with_key(&self, ctx: &AnalyzeCtx, key: &str) -> Result<Vec<VideoMoment>> {
        let (file_name, file_uri) = self.upload_file(ctx, key).await?;
        self.wait_for_file_active(ctx, &file_name, key).await?;
        let moments = self.generate_moments(ctx, &file_uri, key).await?;
        send_progress(ctx, 1.0);
        Ok(moments)
    }
}

fn offset_timestamp(ts: &str, offset_secs: u64) -> String {
    match parse_timestamp_to_seconds(ts) {
        Some(secs) => format_seconds_to_timestamp(secs + offset_secs),
        None => ts.to_owned(),
    }
}

fn parse_dialogue(value: Option<&serde_json::Value>) -> Vec<DialoguePhrase> {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|d| {
            let start = d["start_time"].as_str()?;
            let end = d["end_time"].as_str()?;
            let phrase = d["phrase"].as_str().or_else(|| d["text"].as_str())?;
            Some(DialoguePhrase {
                start_time: start.to_owned(),
                end_time: end.to_owned(),
                phrase: phrase.to_owned(),
            })
        })
        .collect()
}

fn response_schema() -> serde_json::Value {
    json!({
        "type": "OBJECT",
        "properties": {
            "moments": {
                "type": "ARRAY",
                "items": {
                    "type": "OBJECT",
                    "properties": {
                        "start_time": { "type": "STRING" },
                        "end_time": { "type": "STRING" },
                        "category": { "type": "STRING" },
                        "description": { "type": "STRING" },
                        "dialogue": {
                            "type": "ARRAY",
                            "items": {
                                "type": "OBJECT",
                                "properties": {
                                    "start_time": { "type": "STRING" },
                                    "end_time": { "type": "STRING" },
                                    "phrase": { "type": "STRING" }
                                }
                            }
                        }
                    },
                    "required": ["start_time", "end_time", "category", "description"]
                }
            }
        }
    })
}

// -------------------------------------------------------------------------------------------------
// Trait impl
// -------------------------------------------------------------------------------------------------

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
            supported_mimes: vec![
                "video/mp4",
                "video/webm",
                "video/quicktime",
                "video/x-msvideo",
            ],
            max_video_duration: Duration::from_secs(60 * 60),
            max_inline_size_mb: 20,
        }
    }

    async fn analyze_chunk(&self, ctx: AnalyzeCtx) -> Result<Vec<VideoMoment>> {
        // Sticky key per attempt; rotate to another key on quota errors (429).
        let mut last_err: Option<anyhow::Error> = None;
        while let Some(idx) = self.next_key_index() {
            if ctx.cancellation.is_cancelled() {
                anyhow::bail!("cancelled");
            }
            let key = &self.keys[idx].1;
            match self.analyze_with_key(&ctx, key).await {
                Ok(moments) => return Ok(moments),
                Err(e) => {
                    if is_quota_error(&e) {
                        self.mark_disabled(idx);
                        last_err = Some(e);
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow!("no API keys available")))
    }

    async fn test_connection(&self) -> Result<()> {
        let Some(idx) = self.next_key_index() else {
            anyhow::bail!("no API keys configured");
        };
        let key = &self.keys[idx].1;
        if key.is_empty() {
            anyhow::bail!("api key is empty");
        }
        let payload = json!({
            "contents": [{ "role": "user", "parts": [{ "text": "ping" }] }]
        });
        let resp = reqwest::Client::new()
            .post(format!(
                "{API_BASE}/models/{}:generateContent?key={key}",
                self.model
            ))
            .json(&payload)
            .send()
            .await
            .context("calling generateContent")?;
        let status = resp.status();
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("connection test failed ({status}): {body}");
        }
        Ok(())
    }
}

fn is_quota_error(e: &anyhow::Error) -> bool {
    let msg = e.to_string();
    msg.contains("429") || msg.contains("RESOURCE_EXHAUSTED")
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_timestamp() {
        assert_eq!(offset_timestamp("00:01:00", 600), "00:11:00");
        assert_eq!(offset_timestamp("00:00:00", 0), "00:00:00");
        assert_eq!(offset_timestamp("garbage", 600), "garbage");
    }

    #[test]
    fn test_parse_dialogue_phrase_and_text_alias() {
        let v = json!([
            { "start_time": "00:00:01", "end_time": "00:00:02", "phrase": "hello" },
            { "start_time": "00:00:03", "end_time": "00:00:04", "text": "world" }
        ]);
        let phrases = parse_dialogue(Some(&v));
        assert_eq!(phrases.len(), 2);
        assert_eq!(phrases[0].phrase, "hello");
        assert_eq!(phrases[1].phrase, "world");
    }

    #[test]
    fn test_quota_detection() {
        let e429 = anyhow!("request failed with status 429 RESOURCE_EXHAUSTED");
        assert!(is_quota_error(&e429));
        let e500 = anyhow!("request failed with status 500");
        assert!(!is_quota_error(&e500));
    }

    #[test]
    fn test_key_rotation_skips_disabled() {
        let cfg = ProviderConfig {
            keys: vec![
                crate::config::ApiKeyEntry {
                    name: "a".into(),
                    value: "k1".into(),
                    enabled: true,
                },
                crate::config::ApiKeyEntry {
                    name: "b".into(),
                    value: "k2".into(),
                    enabled: true,
                },
                crate::config::ApiKeyEntry {
                    name: "c".into(),
                    value: "k3".into(),
                    enabled: false,
                },
            ],
            model: "m".into(),
            model_pro: "p".into(),
            use_fast_model: true,
            temperature: None,
            media_resolution: None,
        };
        let p = GeminiProvider::new(&cfg);
        let first = p.next_key_index().unwrap();
        assert!(first < 2);
        let second = p.next_key_index().unwrap();
        assert!(second < 2);
        assert_ne!(first, second);
        // Disable both enabled keys -> no key available
        p.mark_disabled(first);
        p.mark_disabled(second);
        assert!(p.next_key_index().is_none());
    }
}
