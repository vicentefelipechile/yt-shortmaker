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

/// Subtitle segment with timestamps (from whisper-rs transcription)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

/// Face region detected in a video frame
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceRegion {
    /// Timestamp in milliseconds
    pub timestamp_ms: u64,
    /// Normalized X position (0.0-1.0)
    pub x: f32,
    /// Normalized Y position (0.0-1.0)
    pub y: f32,
    /// Normalized width (0.0-1.0)
    pub width: f32,
    /// Normalized height (0.0-1.0)
    pub height: f32,
    /// Detection confidence (0.0-1.0)
    pub confidence: f32,
}

/// Face tracking data for a clip
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceTrackingData {
    pub clip_path: String,
    pub has_streamer: bool,
    pub face_regions: Vec<FaceRegion>,
}

/// Compression settings for optimized chunk pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionSettings {
    /// Target resolution height (default: 720)
    pub target_resolution: u32,
    /// Constant Rate Factor (default: 28)
    pub crf: u32,
    /// Audio bitrate (default: "64k")
    pub audio_bitrate: String,
    /// Encoding preset (default: "fast")
    pub preset: String,
}

impl Default for CompressionSettings {
    fn default() -> Self {
        Self {
            target_resolution: 720,
            crf: 28,
            audio_bitrate: "64k".to_string(),
            preset: "fast".to_string(),
        }
    }
}

/// Application version constant
pub const APP_VERSION: &str = "1.10.30";

/// Application name constant
pub const APP_NAME: &str = "YT ShortMaker";
