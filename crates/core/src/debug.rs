// =================================================================================================
// debug — Persistent AI debug logging to file + tracing
// =================================================================================================

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

// -------------------------------------------------------------------------------------------------
// Constants
// -------------------------------------------------------------------------------------------------

const LOG_FILE_NAME: &str = "ai_debug.log";
const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;

// -------------------------------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------------------------------

fn logs_dir() -> Option<PathBuf> {
    crate::session::db_dir().ok().map(|d| d.join("logs"))
}

fn log_file_path() -> Option<PathBuf> {
    logs_dir().map(|d| d.join(LOG_FILE_NAME))
}

/// Returns the current AI debug log file path (creates dir if missing).
pub fn ai_debug_log_path() -> Option<PathBuf> {
    let dir = logs_dir()?;
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join(LOG_FILE_NAME))
}

fn rotate_if_needed(path: &Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > MAX_LOG_BYTES {
            let rotated = path.with_extension("log.1");
            let _ = std::fs::rename(path, rotated);
        }
    }
}

/// Append a raw entry to the persistent AI debug log (best-effort, never panics).
pub fn append_ai_log(entry: &str) {
    let Some(path) = log_file_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    rotate_if_needed(&path);
    let mut opts = OpenOptions::new();
    opts.create(true).append(true);
    if let Ok(mut f) = opts.open(&path) {
        let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let _ = writeln!(f, "[{ts}] {entry}");
    }
}

/// Log a Gemini/OpenRouter raw response and validation outcome.
///
/// Always emits a `tracing::info/warn` and appends to the persistent file so
/// the user can inspect `.../yt-shortmaker-v2/logs/ai_debug.log` even when
/// the console is not visible (release build without console).
#[allow(clippy::too_many_arguments)]
pub fn log_ai_response(
    provider: &str,
    model: &str,
    chunk_path: &Path,
    chunk_offset_secs: u64,
    raw_text: &str,
    parsed_moments: Option<&serde_json::Value>,
    validated_count: usize,
    error: Option<&str>,
) {
    let preview = if raw_text.len() > 4000 {
        format!(
            "{}... [truncated {} bytes]",
            &raw_text[..4000],
            raw_text.len() - 4000
        )
    } else {
        raw_text.to_owned()
    };

    let parsed_preview = parsed_moments
        .map(|v| {
            let s = v.to_string();
            if s.len() > 3000 {
                format!("{}... [truncated]", &s[..3000])
            } else {
                s
            }
        })
        .unwrap_or_else(|| "<no parsed json>".to_owned());

    let msg = format!(
        "provider={provider} model={model} chunk={} offset={}s validated={} raw_text={} parsed={} {}",
        chunk_path.display(),
        chunk_offset_secs,
        validated_count,
        preview,
        parsed_preview,
        error
            .map(|e| format!("error={e}"))
            .unwrap_or_default()
    );

    if validated_count == 0 && error.is_some() {
        tracing::warn!("{}", msg);
    } else {
        tracing::info!("{}", msg);
    }

    append_ai_log(&msg);

    // Also log each rejected moment detail at debug level for deeper inspection
    if let Some(arr) = parsed_moments
        .and_then(|v| v.get("moments"))
        .and_then(|v| v.as_array())
    {
        for (i, m) in arr.iter().enumerate() {
            let start = m
                .get("start_time")
                .and_then(|v| v.as_str())
                .unwrap_or("<missing>");
            let end = m
                .get("end_time")
                .and_then(|v| v.as_str())
                .unwrap_or("<missing>");
            let cat = m
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("<missing>");
            let detail = format!(
                "  moment[{i}] start_time={start:?} end_time={end:?} category={cat:?} raw={m}"
            );
            tracing::debug!("{}", detail);
            // Only append rejected details to file when nothing validated
            if validated_count == 0 {
                append_ai_log(&detail);
            }
        }
    }
}

/// Helper to log an HTTP-level AI error (e.g. 503) with body preview.
pub fn log_ai_http_error(provider: &str, model: &str, chunk_path: &Path, status: &str, body: &str) {
    let body_preview = if body.len() > 3000 {
        format!("{}... [truncated]", &body[..3000])
    } else {
        body.to_owned()
    };
    let msg = format!(
        "provider={provider} model={model} chunk={} http_error status={} body={}",
        chunk_path.display(),
        status,
        body_preview
    );
    tracing::warn!("{}", msg);
    append_ai_log(&msg);
}
