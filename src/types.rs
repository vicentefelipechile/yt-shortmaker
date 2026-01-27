//! Shared data types for YT ShortMaker

use serde::{Deserialize, Serialize};

/// Represents a single line of dialogue with timestamps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialoguePhrase {
    pub start_time: String,
    pub end_time: String,
    pub phrase: String,
}

/// Represents a video moment identified by AI analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMoment {
    pub start_time: String,
    pub end_time: String,
    pub category: String,
    pub description: String,
    #[serde(default)]
    pub dialogue: Vec<DialoguePhrase>,
}

/// Represents a video chunk with start time and duration
#[derive(Debug, Clone)]
pub struct VideoChunk {
    pub start_seconds: u64,
    pub file_path: String,
}

/// Represents the session state for resuming after interruption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub youtube_url: String,
    pub moments: Vec<VideoMoment>,
    pub temp_dir: String,
}

/// Application version constant
pub const APP_VERSION: &str = "1.10.0";

/// Application name constant
pub const APP_NAME: &str = "YT ShortMaker";
