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
use crate::types::{format_seconds_to_timestamp, DialoguePhrase, VideoMoment};

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
const FILE_ACTIVE_TIMEOUT: Duration = Duration::from_secs(180);
const FILE_POLL_INTERVAL: Duration = Duration::from_secs(2);
const CHUNK_MIME: &str = "video/mp4";
const MAX_OUTPUT_TOKENS: u64 = 4096;
const MAX_TIMESTAMP_SECONDS: u64 = 3600;

const SYSTEM_PROMPT: &str = r#"You are a professional video editor assistant. Analyze the provided video chunk and identify the best moments suitable for YouTube Shorts.
1. Duration: 10 to 90 seconds per moment.
2. Each moment must be self-contained (start and end).
3. Prioritize: hooks, funny/emotional moments, strong statements, plot twists, valuable info.
Return all positions as integer seconds. Never return timestamp strings or decimal values. Timestamps are formatted by the application."#;

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
        Self::new_with_model(
            cfg,
            cfg.effective_model().to_owned(),
            cfg.temperature,
            cfg.media_resolution.clone(),
        )
    }

    pub fn new_with_model(
        cfg: &ProviderConfig,
        model: String,
        temperature: Option<f32>,
        media_resolution: Option<String>,
    ) -> Self {
        let keys = cfg
            .enabled_keys()
            .map(|k| (k.name.clone(), k.value.clone()))
            .collect();
        Self {
            keys,
            model,
            temperature,
            media_resolution,
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
        tracing::info!(
            "Gemini poll start file={} timeout={:?}",
            file_name,
            FILE_ACTIVE_TIMEOUT
        );
        let client = reqwest::Client::new();
        let deadline = tokio::time::Instant::now() + FILE_ACTIVE_TIMEOUT;
        // file_name from upload is typically "files/abc123" — handle both forms
        // to avoid double prefix "files/files/abc123" which returns 404 and
        // caused the generic timeout loop.
        let normalized = if file_name.starts_with("files/") {
            file_name.to_owned()
        } else {
            format!("files/{file_name}")
        };
        let url = format!("{API_BASE}/{normalized}?key={key}");
        let mut poll_count: u32 = 0;
        loop {
            poll_count += 1;
            let resp = client.get(&url).send().await.with_context(|| {
                format!("querying file state for {file_name} (poll #{poll_count})")
            })?;
            let status = resp.status();
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            if !status.is_success() {
                let msg = format!("file state query failed ({status}): {body}");
                tracing::warn!("{}", msg);
                // Retry transient errors (429/5xx) until deadline; bail on hard 4xx like 404
                if status.as_u16() == 404 || status.as_u16() == 400 || status.as_u16() == 403 {
                    anyhow::bail!(msg);
                }
                if tokio::time::Instant::now() >= deadline {
                    anyhow::bail!(
                        "timed out waiting for file to become ACTIVE (last error: {msg})"
                    );
                }
                tracing::debug!("poll #{poll_count} transient error, retrying");
                tokio::time::sleep(FILE_POLL_INTERVAL).await;
                continue;
            }
            // GET /v1beta/files/{id} returns top-level fields (no "file" wrapper),
            // while upload returns { "file": {...} }. Handle both shapes.
            let state = body
                .get("state")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    body.get("file")
                        .and_then(|f| f.get("state"))
                        .and_then(|v| v.as_str())
                })
                .unwrap_or("");
            tracing::debug!(
                "poll #{poll_count} file={} state={:?} body={}",
                file_name,
                state,
                body
            );
            match state {
                "ACTIVE" => {
                    send_log(ctx, "File ACTIVE");
                    send_progress(ctx, 0.6);
                    tracing::info!("file ACTIVE after {} polls", poll_count);
                    return Ok(());
                }
                "FAILED" => anyhow::bail!("file processing failed: {body}"),
                _ => {
                    // Covers "PROCESSING" and empty string (API still indexing)
                    // No send_log here — polling cada 2s generaria spam en la UI
                    // (hasta 90 lineas por chunk). Solo tracing para debug.
                    if poll_count == 1 || poll_count % 10 == 0 {
                        tracing::debug!(
                            "still waiting for ACTIVE poll={poll_count} state={state:?}"
                        );
                    }
                    if tokio::time::Instant::now() >= deadline {
                        anyhow::bail!(
                            "timed out waiting for file to become ACTIVE after {} polls (last state={:?}, body={})",
                            poll_count,
                            state,
                            body
                        );
                    }
                    // Gracefully handle cancellation during long poll
                    if ctx.cancellation.is_cancelled() {
                        anyhow::bail!("cancelled while waiting for file ACTIVE");
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
            "Analyze this video chunk and identify the best moments for YouTube Shorts."
                .to_string();

        let mut generation_config = json!({
            "responseMimeType": "application/json",
            "responseSchema": response_schema(),
            "maxOutputTokens": MAX_OUTPUT_TOKENS,
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
                    { "fileData": { "fileUri": file_uri, "mimeType": CHUNK_MIME } }
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

        if body["candidates"][0]["finishReason"] == "MAX_TOKENS" {
            anyhow::bail!("structured response truncated: finish reason MAX_TOKENS");
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
            let Some(start_seconds) = m["start_seconds"].as_u64() else {
                continue;
            };
            let Some(end_seconds) = m["end_seconds"].as_u64() else {
                continue;
            };
            if !valid_second_range(start_seconds, end_seconds) {
                continue;
            }
            out.push(VideoMoment {
                start_time: format_seconds_to_timestamp(start_seconds + offset),
                end_time: format_seconds_to_timestamp(end_seconds + offset),
                category: m["category"].as_str().unwrap_or("Other").to_owned(),
                description: m["description"].as_str().unwrap_or("").to_owned(),
                dialogue: parse_dialogue(m.get("dialogue")),
            });
        }
        if !moments.is_empty() && out.is_empty() {
            anyhow::bail!("structured response contained no valid timestamp ranges");
        }
        Ok(out)
    }

    async fn delete_file(&self, file_name: &str, key: &str) {
        let normalized = if file_name.starts_with("files/") {
            file_name.to_owned()
        } else {
            format!("files/{file_name}")
        };
        let url = format!("{API_BASE}/{normalized}?key={key}");
        let res = reqwest::Client::new().delete(&url).send().await;
        match res {
            Ok(r) if r.status().is_success() => {
                tracing::debug!("deleted remote file {file_name}");
            }
            Ok(r) => {
                let st = r.status();
                let body: serde_json::Value = r.json().await.unwrap_or_default();
                tracing::debug!("delete file {file_name} failed {st}: {body}");
            }
            Err(e) => tracing::debug!("delete file {file_name} error: {e:#}"),
        }
    }

    async fn analyze_with_key(&self, ctx: &AnalyzeCtx, key: &str) -> Result<Vec<VideoMoment>> {
        let (file_name, file_uri) = self.upload_file(ctx, key).await?;
        let wait_res = self.wait_for_file_active(ctx, &file_name, key).await;
        if let Err(e) = wait_res {
            // Best-effort cleanup to avoid leaking files on quota/timeout
            self.delete_file(&file_name, key).await;
            return Err(e);
        }
        let gen_res = self.generate_moments(ctx, &file_uri, key).await;
        // Always try to delete remote file (quota) — ignore errors
        self.delete_file(&file_name, key).await;
        let moments = gen_res?;
        send_progress(ctx, 1.0);
        Ok(moments)
    }
}

fn parse_dialogue(value: Option<&serde_json::Value>) -> Vec<DialoguePhrase> {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|d| {
            let start = d["start_seconds"].as_u64()?;
            let end = d["end_seconds"].as_u64()?;
            if !valid_second_range(start, end) {
                return None;
            }
            let phrase = d["phrase"].as_str().or_else(|| d["text"].as_str())?;
            Some(DialoguePhrase {
                start_time: format_seconds_to_timestamp(start),
                end_time: format_seconds_to_timestamp(end),
                phrase: phrase.to_owned(),
            })
        })
        .collect()
}

fn valid_second_range(start: u64, end: u64) -> bool {
    start <= MAX_TIMESTAMP_SECONDS && end <= MAX_TIMESTAMP_SECONDS && end > start
}

fn response_schema() -> serde_json::Value {
    json!({
        "type": "OBJECT",
        "properties": {
            "moments": {
                "type": "ARRAY",
                "description": "List of best moments suitable for YouTube Shorts (max 5)",
                "maxItems": 5,
                "items": {
                    "type": "OBJECT",
                    "properties": {
                        "start_seconds": {
                            "type": "INTEGER",
                            "description": "Start position in whole seconds relative to chunk start",
                            "minimum": 0,
                            "maximum": MAX_TIMESTAMP_SECONDS
                        },
                        "end_seconds": {
                            "type": "INTEGER",
                            "description": "End position in whole seconds relative to chunk start and after start_seconds",
                            "minimum": 0,
                            "maximum": MAX_TIMESTAMP_SECONDS
                        },
                        "category": {
                            "type": "STRING",
                            "description": "Category of the moment",
                            "enum": ["Hook", "Funny", "Emotional", "Information", "Plot Twist", "Valuable Info", "Other"]
                        },
                        "description": {
                            "type": "STRING",
                            "description": "Brief description of why this moment is good for a Short"
                        },
                        "dialogue": {
                            "type": "ARRAY",
                            "description": "Key spoken phrases within the moment (max 3)",
                            "maxItems": 3,
                            "items": {
                                "type": "OBJECT",
                                "properties": {
                                    "start_seconds": {
                                        "type": "INTEGER",
                                        "description": "Phrase start position in whole seconds relative to chunk start",
                                        "minimum": 0,
                                        "maximum": MAX_TIMESTAMP_SECONDS
                                    },
                                    "end_seconds": {
                                        "type": "INTEGER",
                                        "description": "Phrase end position in whole seconds relative to chunk start",
                                        "minimum": 0,
                                        "maximum": MAX_TIMESTAMP_SECONDS
                                    },
                                    "phrase": {
                                        "type": "STRING",
                                        "description": "Exact spoken phrase"
                                    }
                                },
                                "required": ["start_seconds", "end_seconds", "phrase"],
                                "propertyOrdering": ["start_seconds", "end_seconds", "phrase"]
                            }
                        }
                    },
                    "required": ["start_seconds", "end_seconds", "category", "description"],
                    "propertyOrdering": ["start_seconds", "end_seconds", "category", "description", "dialogue"]
                }
            }
        },
        "required": ["moments"],
        "propertyOrdering": ["moments"]
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
        // Error handling tiers:
        // - permanent (400/401/403/404): fail fast
        // - quota (429/RESOURCE_EXHAUSTED): disable key and try next key
        // - transient (500/502/503/504/UNAVAILABLE/high demand/EOF): retry same key with exponential backoff
        const MAX_TRANSIENT_RETRIES: u32 = 3;
        let mut last_err: Option<anyhow::Error> = None;
        let mut tried: std::collections::HashSet<usize> = std::collections::HashSet::new();
        while let Some(idx) = self.next_key_index() {
            // Prevent infinite loop with single key + transient (not disabled)
            if !tried.insert(idx) {
                break;
            }
            if ctx.cancellation.is_cancelled() {
                anyhow::bail!("cancelled");
            }
            let key = &self.keys[idx].1;
            let mut attempt: u32 = 0;
            loop {
                match self.analyze_with_key(&ctx, key).await {
                    Ok(moments) => return Ok(moments),
                    Err(e) => {
                        if is_quota_error(&e) {
                            self.mark_disabled(idx);
                            tracing::warn!("quota exhausted on key {idx}, rotating: {e:#}");
                            send_log(&ctx, format!("Quota hit (key {idx}), switching key..."));
                            last_err = Some(e);
                            break; // next key
                        }
                        if is_transient_error(&e) {
                            if attempt >= MAX_TRANSIENT_RETRIES {
                                tracing::warn!(
                                    "transient error exhausted retries ({MAX_TRANSIENT_RETRIES}) on key {idx}: {e:#}"
                                );
                                last_err = Some(e);
                                break; // try next key if available
                            }
                            attempt += 1;
                            let backoff = crate::util::exponential_backoff(attempt)
                                .min(Duration::from_secs(30));
                            let msg = format!(
                                "Transient error, retry {attempt}/{MAX_TRANSIENT_RETRIES} after {}s: {}",
                                backoff.as_secs(),
                                short_error(&e)
                            );
                            send_log(&ctx, msg.clone());
                            tracing::warn!("{} (key {idx}): {e:#}", msg);
                            // Respect cancellation during backoff
                            tokio::select! {
                                _ = tokio::time::sleep(backoff) => {},
                                _ = ctx.cancellation.cancelled() => anyhow::bail!("cancelled"),
                            }
                            if ctx.cancellation.is_cancelled() {
                                anyhow::bail!("cancelled");
                            }
                            continue; // retry same key
                        }
                        // Permanent error — fail fast, no retry across keys
                        return Err(e);
                    }
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
    msg.contains("429") || msg.contains("RESOURCE_EXHAUSTED") || msg.contains("QUOTA_EXCEEDED")
}

fn is_transient_error(e: &anyhow::Error) -> bool {
    let msg = e.to_string();
    // 503 UNAVAILABLE (high demand), 500 INTERNAL, 502/504 gateway — all retryable
    // also JSON truncation due to MAX_TOKENS (EOF while parsing)
    msg.contains("503")
        || msg.contains("500")
        || msg.contains("502")
        || msg.contains("504")
        || msg.contains("UNAVAILABLE")
        || msg.contains("INTERNAL")
        || msg.contains("DEADLINE_EXCEEDED")
        || msg.contains("high demand")
        || msg.contains("MAX_TOKENS")
        || msg.contains("overloaded")
        || msg.contains("temporarily")
        || msg.contains("try again later")
        || msg.contains("EOF while parsing")
        || msg.contains("parsing structured moments json")
        || msg.contains("no valid timestamp ranges")
}

fn short_error(e: &anyhow::Error) -> String {
    let s = e.to_string();
    // Truncate to first line / 180 chars to avoid dumping huge JSON in log
    let first = s.lines().next().unwrap_or(&s);
    if first.len() > 180 {
        format!("{}...", &first[..180])
    } else {
        first.to_owned()
    }
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_second_range() {
        assert!(valid_second_range(1, 2));
        assert!(!valid_second_range(2, 2));
        assert!(!valid_second_range(0, MAX_TIMESTAMP_SECONDS + 1));
    }

    #[test]
    fn test_parse_dialogue_phrase_and_text_alias() {
        let v = json!([
            { "start_seconds": 1, "end_seconds": 2, "phrase": "hello" },
            { "start_seconds": 3, "end_seconds": 4, "text": "world" }
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
