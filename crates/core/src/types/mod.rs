// =================================================================================================
// types — Shared domain types
// =================================================================================================

use serde::{Deserialize, Serialize};

// -------------------------------------------------------------------------------------------------
// Constants
// -------------------------------------------------------------------------------------------------

pub const DEFAULT_CATEGORY_OTHER: &str = "Other";

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialoguePhrase {
    pub text: String,
    pub start_time: String,
    pub end_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VideoMoment {
    pub start_time: String,
    pub end_time: String,
    pub category: String,
    pub description: String,
    #[serde(default)]
    pub dialogue: Vec<DialoguePhrase>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VideoChunk {
    pub start_seconds: u64,
    pub file_path: String,
}

// -------------------------------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------------------------------

/// Parses an `HH:MM:SS` timestamp to total seconds.
///
/// Returns `None` if the format is invalid.
pub fn parse_timestamp_to_seconds(ts: &str) -> Option<u64> {
    let parts: Vec<&str> = ts.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: u64 = parts[0].parse().ok()?;
    let m: u64 = parts[1].parse().ok()?;
    let s: u64 = parts[2].parse().ok()?;
    if m >= 60 || s >= 60 {
        return None;
    }
    Some(h * 3600 + m * 60 + s)
}

/// Formats total seconds as `HH:MM:SS`.
pub fn format_seconds_to_timestamp(total_secs: u64) -> String {
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}
