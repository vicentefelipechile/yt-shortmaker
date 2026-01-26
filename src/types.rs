//! Shared data types for AutoShorts-Rust-CLI

use serde::{Deserialize, Serialize};

/// Represents a video moment identified by AI analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMoment {
    pub start_time: String,
    pub end_time: String,
    pub category: String,
    pub description: String,
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
pub const APP_VERSION: &str = "1.8.0";

/// Application name constant
pub const APP_NAME: &str = "AUTOSHORTS-RUST-CLI";
