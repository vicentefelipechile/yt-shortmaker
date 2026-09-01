// =================================================================================================
// ai::openrouter — OpenRouter provider (video via chat/completions video_url)
// =================================================================================================

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use base64::Engine as _;
use serde_json::json;

use super::provider::{AiProvider, AnalyzeCtx, ProviderCapabilities, ProviderEvent};
use crate::config::ProviderConfig;
use crate::types::{
    format_seconds_to_timestamp, parse_timestamp_to_seconds, DialoguePhrase, VideoMoment,
};

const MAX_OUTPUT_TOKENS: u64 = 4096;
const MAX_TIMESTAMP_SECONDS: u64 = 3600;

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

const OPENROUTER_ID: &str = "openrouter";
const OPENROUTER_DISPLAY: &str = "OpenRouter";
const API_BASE: &str = "https://openrouter.ai/api/v1";
const CHUNK_MIME: &str = "video/mp4";

const SYSTEM_PROMPT: &str = r#"You are a professional video editor assistant. Analyze the provided video and identify the best moments suitable for YouTube Shorts.
1. Duration: 10 to 90 seconds per moment.
2. Each moment must be self-contained (start and end).
3. Prioritize: hooks, funny/emotional moments, strong statements, plot twists, valuable info.
Timestamps are within the provided video where 00:00:00 is the first frame. Return ONLY HH:MM:SS (exactly 8 chars, e.g. 00:01:23), zero-padded, 00:00:00 to 01:00:00. NEVER add milliseconds, decimals or extra zeros (e.g. 00:01:23.000 is FORBIDDEN)."#;

// -------------------------------------------------------------------------------------------------
// Video model detection
// -------------------------------------------------------------------------------------------------

/// Checks if an OpenRouter model id supports video input.
/// Verified via OpenRouter API `GET /api/v1/models` `architecture.input_modalities` includes `video`
/// (74 models as of 2026-08-31). Uses exact families, not generic hints like `gpt-4o`/`claude` which are
/// text/image only on OpenRouter.
pub fn is_video_model(model_id: &str) -> bool {
    let m = model_id.to_lowercase();
    // Strip :batch / :free / :nitro etc. suffix for base match
    let base = m.split(':').next().unwrap_or(&m);
    // Exact video families per OpenRouter API — openai/gpt-4o and anthropic/claude are NOT video
    const VIDEO_PREFIXES: &[&str] = &[
        "google/gemini-",
        "google/gemma-4-",
        "qwen/qwen3.",
        "qwen/qwen3-",
        "z-ai/glm-5",
        "z-ai/glm-4.6",
        "meta/muse-spark-",
        "bytedance-seed/seed-",
        "moonshotai/kimi-",
        "minimax/minimax-",
        "stepfun/step-",
        "perceptron/perceptron-",
        "nvidia/nemotron-",
        "xiaomi/mimo-",
        "rekaai/reka-",
        "amazon/nova-2-lite",
        "openrouter/auto",
    ];
    if VIDEO_PREFIXES.iter().any(|p| base.starts_with(p)) {
        return true;
    }
    // Generic fallback only for explicit video keyword in id (e.g. custom video models)
    m.contains("video")
}

/// Detailed check with category for UI warning.
pub fn video_support_reason(model_id: &str) -> Option<String> {
    if is_video_model(model_id) {
        None
    } else {
        Some(format!(
            "model '{model_id}' does not appear to support video input on OpenRouter. Use a video-capable model like google/gemini-2.5-flash, qwen/qwen3.5-flash, meta/muse-spark-1.2, z-ai/glm-5.3-flash or bytedance-seed/seed-2-1-turbo (see https://openrouter.ai/models?input_modality=video)"
        ))
    }
}

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

pub struct OpenRouterProvider {
    keys: Vec<(String, String)>,
    model: String,
    temperature: Option<f32>,
    disabled: Mutex<std::collections::HashSet<usize>>,
    cursor: AtomicUsize,
}

// -------------------------------------------------------------------------------------------------
// Main
// -------------------------------------------------------------------------------------------------

impl OpenRouterProvider {
    pub fn new(cfg: &ProviderConfig) -> Self {
        Self::new_with_model(cfg, cfg.effective_model().to_owned(), cfg.temperature)
    }

    pub fn new_with_model(cfg: &ProviderConfig, model: String, temperature: Option<f32>) -> Self {
        let keys = cfg
            .enabled_keys()
            .map(|k| (k.name.clone(), k.value.clone()))
            .collect();
        Self {
            keys,
            model,
            temperature,
            disabled: Mutex::new(std::collections::HashSet::new()),
            cursor: AtomicUsize::new(0),
        }
    }

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

    async fn analyze_with_key(&self, ctx: &AnalyzeCtx, key: &str) -> Result<Vec<VideoMoment>> {
        send_log(ctx, format!("Encoding {}", ctx.chunk_path.display()));
        let bytes = tokio::fs::read(&ctx.chunk_path)
            .await
            .with_context(|| format!("reading chunk {}", ctx.chunk_path.display()))?;
        // Warn if file too large for inline base64 (OpenRouter ~20MB payload limit)
        if bytes.len() > 25 * 1024 * 1024 {
            tracing::warn!(
                "chunk {} is {} bytes, base64 will be ~{} MB — may exceed OpenRouter limit",
                ctx.chunk_path.display(),
                bytes.len(),
                bytes.len() * 4 / 3 / 1024 / 1024
            );
            send_log(
                ctx,
                "Warning: chunk large for OpenRouter inline video, may fail — consider smaller chunk_size",
            );
        }
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let data_url = format!("data:{CHUNK_MIME};base64,{b64}");
        send_progress(ctx, 0.2);
        send_log(ctx, "Calling OpenRouter chat/completions (video_url)");

        let user_prompt =
            "Analyze this video and identify the best moments for YouTube Shorts.".to_string();

        let mut payload = json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": SYSTEM_PROMPT },
                {
                    "role": "user",
                    "content": [
                        { "type": "text", "text": user_prompt },
                        { "type": "video_url", "video_url": { "url": data_url } }
                    ]
                }
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "moments",
                    "strict": true,
                    "schema": openrouter_response_schema()
                }
            },
        });
        // Also set max_tokens to mirror gemini maxOutputTokens
        payload["max_tokens"] = json!(MAX_OUTPUT_TOKENS);
        if let Some(t) = self.temperature {
            payload["temperature"] = json!(t);
        }

        let resp = reqwest::Client::new()
            .post(format!("{API_BASE}/chat/completions"))
            .header("Authorization", format!("Bearer {key}"))
            .header("Content-Type", "application/json")
            .header(
                "HTTP-Referer",
                "https://github.com/vicentefelipechile/yt-shortmaker",
            )
            .header("X-Title", "yt-shortmaker")
            .json(&payload)
            .send()
            .await
            .context("calling OpenRouter chat/completions")?;

        let status = resp.status();
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        if !status.is_success() {
            let body_str = body.to_string();
            crate::debug::log_ai_http_error(
                "openrouter",
                &self.model,
                &ctx.chunk_path,
                &status.to_string(),
                &body_str,
            );
            send_log(ctx, format!("OpenRouter failed ({status}): {body_str}"));
            anyhow::bail!("OpenRouter generate failed ({status}): {body}");
        }

        if body["choices"][0]["finish_reason"] == "length" {
            let raw = body.to_string();
            crate::debug::append_ai_log(&format!(
                "provider=openrouter model={} chunk={} offset={}s LENGTH_TRUNCATED raw={}",
                self.model,
                ctx.chunk_path.display(),
                ctx.chunk_start_offset.as_secs(),
                raw
            ));
            anyhow::bail!("structured response truncated: finish reason length");
        }

        // OpenRouter normalizes to OpenAI shape: choices[0].message.content is JSON string
        let content = body["choices"][0]["message"]["content"]
            .as_str()
            .or_else(|| body["choices"][0]["message"]["reasoning"].as_str())
            .ok_or_else(|| anyhow!("OpenRouter response missing content: {body}"))?;

        tracing::info!(
            "OpenRouter raw model={} chunk={} content={}",
            self.model,
            ctx.chunk_path.display(),
            content
        );

        // Content should already be JSON per response_format, but may be wrapped in markdown fence
        let json_str = extract_json(content);
        let parsed: serde_json::Value = match serde_json::from_str(&json_str) {
            Ok(v) => v,
            Err(e) => {
                crate::debug::append_ai_log(&format!(
                    "provider=openrouter model={} chunk={} offset={}s JSON_PARSE_ERROR error={} raw={}",
                    self.model,
                    ctx.chunk_path.display(),
                    ctx.chunk_start_offset.as_secs(),
                    e,
                    json_str
                ));
                return Err(anyhow::Error::from(e).context("parsing structured moments json"));
            }
        };
        let moments = parsed["moments"]
            .as_array()
            .ok_or_else(|| anyhow!("moments array missing in response: {parsed}"))?;

        let offset = ctx.chunk_start_offset.as_secs();
        let mut out = Vec::new();
        let mut rejected: Vec<String> = Vec::new();
        for m in moments {
            let Some(start_str) = m["start_time"].as_str() else {
                rejected.push(format!("missing start_time in {m}"));
                continue;
            };
            let Some(end_str) = m["end_time"].as_str() else {
                rejected.push(format!("missing end_time in {m}"));
                continue;
            };
            let Some(start_secs) = parse_timestamp_to_seconds(start_str) else {
                rejected.push(format!(
                    "bad start_time {start_str:?} in {m} (expected HH:MM:SS)"
                ));
                continue;
            };
            let Some(end_secs) = parse_timestamp_to_seconds(end_str) else {
                rejected.push(format!(
                    "bad end_time {end_str:?} in {m} (expected HH:MM:SS)"
                ));
                continue;
            };
            if !valid_second_range(start_secs, end_secs) {
                rejected.push(format!(
                    "invalid range start={start_secs} end={end_secs} in {m}"
                ));
                continue;
            }
            out.push(VideoMoment {
                start_time: format_seconds_to_timestamp(start_secs + offset),
                end_time: format_seconds_to_timestamp(end_secs + offset),
                category: m["category"].as_str().unwrap_or("Other").to_owned(),
                description: m["description"].as_str().unwrap_or("").to_owned(),
                dialogue: parse_dialogue(m.get("dialogue"), offset),
            });
        }

        let rejected_msg = if rejected.is_empty() {
            None
        } else {
            Some(rejected.join(" | "))
        };
        crate::debug::log_ai_response(
            "openrouter",
            &self.model,
            &ctx.chunk_path,
            offset,
            &json_str,
            Some(&parsed),
            out.len(),
            rejected_msg.as_deref(),
        );
        if !rejected.is_empty() {
            send_log(
                ctx,
                format!(
                    "Rejected {} moments: {}",
                    rejected.len(),
                    rejected.join(" | ")
                ),
            );
        }

        if !moments.is_empty() && out.is_empty() {
            tracing::warn!(
                "all {} moments rejected for chunk {} (offset {}s): {}",
                moments.len(),
                ctx.chunk_path.display(),
                offset,
                rejected_msg.as_deref().unwrap_or("unknown reason")
            );
        }
        send_progress(ctx, 1.0);
        Ok(out)
    }
}

fn extract_json(s: &str) -> String {
    let t = s.trim();
    // Strip ```json ... ``` fence if present
    if t.starts_with("```") {
        let inner = t
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        return inner.to_owned();
    }
    t.to_owned()
}

fn parse_dialogue(value: Option<&serde_json::Value>, offset: u64) -> Vec<DialoguePhrase> {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|d| {
            let start_str = d["start_time"].as_str()?;
            let end_str = d["end_time"].as_str()?;
            let start = parse_timestamp_to_seconds(start_str)?;
            let end = parse_timestamp_to_seconds(end_str)?;
            if !valid_second_range(start, end) {
                return None;
            }
            let phrase = d["phrase"].as_str().or_else(|| d["text"].as_str())?;
            Some(DialoguePhrase {
                start_time: format_seconds_to_timestamp(start + offset),
                end_time: format_seconds_to_timestamp(end + offset),
                phrase: phrase.to_owned(),
            })
        })
        .collect()
}

fn valid_second_range(start: u64, end: u64) -> bool {
    start <= MAX_TIMESTAMP_SECONDS && end <= MAX_TIMESTAMP_SECONDS && end > start
}

fn openrouter_response_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "moments": {
                "type": "array",
                "description": "List of best moments suitable for YouTube Shorts (max 5)",
                "maxItems": 5,
                "items": {
                    "type": "object",
                    "properties": {
                        "start_time": {
                            "type": "string",
                            "description": "Start timestamp within the provided video as HH:MM:SS (e.g. 00:01:23) from 00:00:00 (first frame). Exactly 8 chars."
                        },
                        "end_time": {
                            "type": "string",
                            "description": "End timestamp within the provided video as HH:MM:SS (e.g. 00:01:45), 10-90s after start_time. Exactly HH:MM:SS."
                        },
                        "category": {
                            "type": "string",
                            "description": "Category of the moment",
                            "enum": ["Hook", "Funny", "Emotional", "Information", "Plot Twist", "Valuable Info", "Other"]
                        },
                        "description": {
                            "type": "string",
                            "description": "Brief description of why this moment is good for a Short"
                        },
                        "dialogue": {
                            "type": "array",
                            "description": "Key spoken phrases within the moment (max 3)",
                            "maxItems": 3,
                            "items": {
                                "type": "object",
                                "properties": {
                                    "start_time": {
                                        "type": "string",
                                        "description": "Phrase start within the provided video as HH:MM:SS from 00:00:00, no milliseconds"
                                    },
                                    "end_time": {
                                        "type": "string",
                                        "description": "Phrase end within the provided video as HH:MM:SS from 00:00:00, no milliseconds"
                                    },
                                    "phrase": {
                                        "type": "string",
                                        "description": "Exact spoken phrase"
                                    }
                                },
                                "required": ["start_time", "end_time", "phrase"],
                                "additionalProperties": false
                            }
                        }
                    },
                    "required": ["start_time", "end_time", "category", "description"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["moments"],
        "additionalProperties": false
    })
}

// -------------------------------------------------------------------------------------------------
// Trait impl
// -------------------------------------------------------------------------------------------------

#[async_trait]
impl AiProvider for OpenRouterProvider {
    fn id(&self) -> &'static str {
        OPENROUTER_ID
    }

    fn display_name(&self) -> &'static str {
        OPENROUTER_DISPLAY
    }

    fn capabilities(&self) -> ProviderCapabilities {
        let supports_video = is_video_model(&self.model);
        ProviderCapabilities {
            supports_native_video: supports_video,
            supported_mimes: vec![
                "video/mp4",
                "video/webm",
                "video/quicktime",
                "video/x-msvideo",
            ],
            max_video_duration: Duration::from_secs(60 * 60),
            max_inline_size_mb: if supports_video { 25 } else { 0 },
        }
    }

    async fn analyze_chunk(&self, ctx: AnalyzeCtx) -> Result<Vec<VideoMoment>> {
        if !self.capabilities().supports_native_video {
            anyhow::bail!(
                "OpenRouter model '{}' does not support video input — use a video-capable model like google/gemini-2.5-flash, openai/gpt-4o, qwen/qwen2.5-vl via OpenRouter",
                self.model
            );
        }
        const MAX_TRANSIENT_RETRIES: u32 = 3;
        let mut last_err: Option<anyhow::Error> = None;
        let mut tried: std::collections::HashSet<usize> = std::collections::HashSet::new();
        while let Some(idx) = self.next_key_index() {
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
                            tracing::warn!(
                                "OpenRouter quota exhausted on key {idx}, rotating: {e:#}"
                            );
                            send_log(&ctx, format!("Quota hit (key {idx}), switching key..."));
                            last_err = Some(e);
                            break;
                        }
                        if is_transient_error(&e) {
                            if attempt >= MAX_TRANSIENT_RETRIES {
                                tracing::warn!(
                                    "transient error exhausted retries ({MAX_TRANSIENT_RETRIES}) on key {idx}: {e:#}"
                                );
                                last_err = Some(e);
                                break;
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
                            tokio::select! {
                                _ = tokio::time::sleep(backoff) => {},
                                _ = ctx.cancellation.cancelled() => anyhow::bail!("cancelled"),
                            }
                            if ctx.cancellation.is_cancelled() {
                                anyhow::bail!("cancelled");
                            }
                            continue;
                        }
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
        // Minimal chat request to verify key/model
        let payload = json!({
            "model": self.model,
            "messages": [{ "role": "user", "content": "ping" }],
            "max_tokens": 5
        });
        let resp = reqwest::Client::new()
            .post(format!("{API_BASE}/chat/completions"))
            .header("Authorization", format!("Bearer {key}"))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .context("calling OpenRouter chat/completions")?;
        let status = resp.status();
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("OpenRouter connection test failed ({status}): {body}");
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
    // "no valid timestamp ranges" is NOT transient — would loop 3x with same bad hallucination
    msg.contains("503")
        || msg.contains("500")
        || msg.contains("502")
        || msg.contains("504")
        || msg.contains("UNAVAILABLE")
        || msg.contains("INTERNAL")
        || msg.contains("DEADLINE_EXCEEDED")
        || msg.contains("high demand")
        || msg.contains("finish reason length")
        || msg.contains("overloaded")
        || msg.contains("temporarily")
        || msg.contains("try again later")
        || msg.contains("EOF while parsing")
        || msg.contains("parsing structured moments json")
        || msg.contains("MAX_TOKENS")
}

fn short_error(e: &anyhow::Error) -> String {
    let s = e.to_string();
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
    fn test_is_video_model() {
        assert!(is_video_model("google/gemini-2.5-flash"));
        assert!(is_video_model("google/gemini-2.0-flash"));
        assert!(is_video_model("google/gemini-2.5-flash:batch"));
        assert!(is_video_model("qwen/qwen3.5-flash"));
        assert!(is_video_model("qwen/qwen3.8-flash"));
        assert!(is_video_model("meta/muse-spark-1.2"));
        assert!(is_video_model("z-ai/glm-5.3-flash"));
        assert!(is_video_model("bytedance-seed/seed-2-1-turbo"));
        assert!(!is_video_model("openai/gpt-4o"));
        assert!(!is_video_model("openai/gpt-4o-mini"));
        assert!(!is_video_model("anthropic/claude-3.5-sonnet"));
        assert!(!is_video_model("anthropic/claude-opus-4"));
        assert!(!is_video_model("meta-llama/llama-3.1-8b-instruct"));
        assert!(!is_video_model("openai/gpt-3.5-turbo"));
        assert!(!is_video_model("mistral/mistral-7b-instruct"));
    }

    #[test]
    fn test_video_support_reason() {
        assert!(video_support_reason("google/gemini-2.5-flash").is_none());
        assert!(video_support_reason("meta-llama/llama-3.1-8b").is_some());
    }
}
