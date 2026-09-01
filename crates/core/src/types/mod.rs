// =================================================================================================
// types — Shared domain types
// =================================================================================================

use serde::{Deserialize, Serialize};

// -------------------------------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DialoguePhrase {
    pub start_time: String,
    pub end_time: String,
    /// Phrase text — accepts both `phrase` (v1) and `text` (v2) on deserialize.
    #[serde(alias = "text")]
    pub phrase: String,
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

pub const MIN_MOMENT_DURATION_SECONDS: u64 = 20;

// -------------------------------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------------------------------

/// Parses `HH:MM:SS` or `MM:SS` timestamp to total seconds.
///
/// Accepts both `00:04:05` and `04:05` (and `4:05`) as produced by Gemini
/// despite schema asking for `HH:MM:SS`. Returns `None` if the format is invalid.
pub fn parse_timestamp_to_seconds(ts: &str) -> Option<u64> {
    // Strip any fractional part (e.g. 00:04:05.000) — robust against model decimals
    let ts = ts.split('.').next().unwrap_or(ts).trim();
    let parts: Vec<&str> = ts.split(':').collect();
    match parts.len() {
        2 => {
            let m: u64 = parts[0].parse().ok()?;
            let s: u64 = parts[1].parse().ok()?;
            if m >= 60 || s >= 60 {
                return None;
            }
            Some(m * 60 + s)
        }
        3 => {
            let h: u64 = parts[0].parse().ok()?;
            let m: u64 = parts[1].parse().ok()?;
            let s: u64 = parts[2].parse().ok()?;
            if m >= 60 || s >= 60 {
                return None;
            }
            Some(h * 3600 + m * 60 + s)
        }
        _ => None,
    }
}

/// Formats total seconds as `HH:MM:SS`.
pub fn format_seconds_to_timestamp(total_secs: u64) -> String {
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

pub fn format_duration(secs: u64) -> String {
    crate::util::format_duration(secs)
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_timestamp_valid() {
        assert_eq!(parse_timestamp_to_seconds("00:00:00"), Some(0));
        assert_eq!(parse_timestamp_to_seconds("00:01:30"), Some(90));
        assert_eq!(parse_timestamp_to_seconds("01:02:03"), Some(3723));
        assert_eq!(parse_timestamp_to_seconds("04:05"), Some(245));
        assert_eq!(parse_timestamp_to_seconds("4:05"), Some(245));
        assert_eq!(parse_timestamp_to_seconds("00:04:05.000"), Some(245));
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        assert_eq!(parse_timestamp_to_seconds("bad"), None);
        assert_eq!(parse_timestamp_to_seconds("00:60:00"), None);
        assert_eq!(parse_timestamp_to_seconds("00:00:60"), None);
    }

    #[test]
    fn test_format_roundtrip() {
        assert_eq!(format_seconds_to_timestamp(0), "00:00:00");
        assert_eq!(format_seconds_to_timestamp(3723), "01:02:03");
        assert_eq!(
            parse_timestamp_to_seconds(&format_seconds_to_timestamp(9999)),
            Some(9999)
        );
    }

    #[test]
    fn test_dialogue_alias() {
        let j1 = r#"{"start_time":"00:00:01","end_time":"00:00:02","phrase":"hello"}"#;
        let j2 = r#"{"start_time":"00:00:01","end_time":"00:00:02","text":"hello"}"#;
        let d1: DialoguePhrase = serde_json::from_str(j1).unwrap();
        let d2: DialoguePhrase = serde_json::from_str(j2).unwrap();
        assert_eq!(d1.phrase, "hello");
        assert_eq!(d2.phrase, "hello");
    }

    #[test]
    fn test_moment_serde() {
        let m = VideoMoment {
            start_time: "00:00:10".into(),
            end_time: "00:00:20".into(),
            category: "hook".into(),
            description: "desc".into(),
            dialogue: vec![],
        };
        let j = serde_json::to_string(&m).unwrap();
        let back: VideoMoment = serde_json::from_str(&j).unwrap();
        assert_eq!(m, back);
    }
}
