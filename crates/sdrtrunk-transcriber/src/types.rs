//! Core types for the transcription service

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Transcription status enumeration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionStatus {
    /// Pending transcription
    Pending,
    /// Currently being transcribed
    Processing,
    /// Transcription completed successfully
    Completed,
    /// Transcription failed
    Failed,
    /// Transcription cancelled
    Cancelled,
}

impl Default for TranscriptionStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl std::fmt::Display for TranscriptionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Processing => write!(f, "processing"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Transcription service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionConfig {
    /// Enable transcription service
    pub enabled: bool,

    /// Service backend ("whisperx", "mock")
    pub service: String,

    /// Model size ("tiny", "base", "small", "medium", "large-v2", "large-v3")
    pub model_size: String,

    /// Compute device ("cuda", "cpu", "mps")
    pub device: String,

    /// Batch size for processing
    pub batch_size: usize,

    /// Compute type ("float16", "int8", "float32")
    pub compute_type: String,

    /// Language code (None for auto-detect)
    pub language: Option<String>,

    /// Minimum number of speakers for diarization
    pub min_speakers: Option<usize>,

    /// Maximum number of speakers for diarization
    pub max_speakers: Option<usize>,

    /// Number of worker threads
    pub workers: usize,

    /// Python service path (for WhisperX)
    pub python_path: Option<PathBuf>,

    /// Service port (for HTTP-based services)
    pub service_port: Option<u16>,

    /// Processing timeout in seconds
    pub timeout_seconds: u64,

    /// Maximum retries for failed transcriptions
    pub max_retries: u32,

    /// Queue size limit
    pub queue_size: usize,

    /// Enable VAD (Voice Activity Detection) preprocessing
    pub enable_vad: bool,

    /// Enable word-level timestamps
    pub word_timestamps: bool,

    /// Confidence threshold (0.0-1.0)
    pub confidence_threshold: f32,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            service: "whisperx".to_string(),
            model_size: "large-v3".to_string(),
            device: "cpu".to_string(),
            batch_size: 16,
            compute_type: "float32".to_string(),
            language: None,
            min_speakers: Some(1),
            max_speakers: Some(10),
            workers: 2,
            python_path: None,
            service_port: None, // Must be configured in config.toml
            timeout_seconds: 300,
            max_retries: 3,
            queue_size: 1000,
            enable_vad: true,
            word_timestamps: true,
            confidence_threshold: 0.6,
        }
    }
}

/// Transcription request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionRequest {
    /// Unique request ID
    pub id: Uuid,

    /// Database record ID for the call
    pub call_id: Uuid,

    /// Path to the audio file
    pub audio_path: PathBuf,

    /// Request timestamp
    pub requested_at: DateTime<Utc>,

    /// Processing options
    pub options: TranscriptionOptions,

    /// Retry count
    pub retry_count: u32,

    /// Priority (higher = more important)
    pub priority: i32,
}

impl TranscriptionRequest {
    /// Create a new transcription request
    pub fn new(call_id: Uuid, audio_path: PathBuf) -> Self {
        Self {
            id: Uuid::new_v4(),
            call_id,
            audio_path,
            requested_at: Utc::now(),
            options: TranscriptionOptions::default(),
            retry_count: 0,
            priority: 0,
        }
    }

    /// Create with custom options
    pub fn with_options(
        call_id: Uuid,
        audio_path: PathBuf,
        options: TranscriptionOptions,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            call_id,
            audio_path,
            requested_at: Utc::now(),
            options,
            retry_count: 0,
            priority: 0,
        }
    }

    /// Set priority
    pub const fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
}

/// Transcription processing options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionOptions {
    /// Language hint (None for auto-detect)
    pub language: Option<String>,

    /// Enable speaker diarization
    pub diarize: bool,

    /// Min speakers for diarization
    pub min_speakers: Option<usize>,

    /// Max speakers for diarization
    pub max_speakers: Option<usize>,

    /// Enable VAD preprocessing
    pub vad: bool,

    /// Word-level timestamps
    pub word_timestamps: bool,

    /// Return confidence scores
    pub return_confidence: bool,

    /// Maximum audio duration to process (seconds)
    pub max_duration: Option<f64>,
}

impl Default for TranscriptionOptions {
    fn default() -> Self {
        Self {
            language: None,
            diarize: true,
            min_speakers: Some(1),
            max_speakers: Some(10),
            vad: true,
            word_timestamps: true,
            return_confidence: true,
            max_duration: Some(3600.0), // 1 hour max
        }
    }
}

/// Transcription response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResponse {
    /// Request ID
    pub request_id: Uuid,

    /// Call ID
    pub call_id: Uuid,

    /// Status
    pub status: TranscriptionStatus,

    /// Full transcription text
    pub text: Option<String>,

    /// Detected language
    pub language: Option<String>,

    /// Overall confidence score (0.0-1.0)
    pub confidence: Option<f32>,

    /// Processing duration in milliseconds
    pub processing_time_ms: u64,

    /// Transcription segments with timestamps
    pub segments: Vec<TranscriptionSegment>,

    /// Speaker segments (if diarization enabled)
    pub speaker_segments: Vec<SpeakerSegment>,

    /// Number of detected speakers
    pub speaker_count: Option<usize>,

    /// Word-level segments (if enabled)
    pub words: Vec<WordSegment>,

    /// Error message (if failed)
    pub error: Option<String>,

    /// Completion timestamp
    pub completed_at: DateTime<Utc>,
}

/// Transcription segment with timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSegment {
    /// Segment ID
    pub id: usize,

    /// Start time in seconds
    pub start: f64,

    /// End time in seconds
    pub end: f64,

    /// Transcribed text
    pub text: String,

    /// Confidence score (0.0-1.0)
    pub confidence: Option<f32>,

    /// Speaker ID (if diarization enabled)
    pub speaker: Option<String>,

    /// Words in this segment
    pub words: Option<Vec<WordSegment>>,
}

/// Speaker segment from diarization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerSegment {
    /// Speaker identifier (e.g., "SPEAKER_00")
    pub speaker: String,

    /// Start time in seconds
    pub start: f64,

    /// End time in seconds
    pub end: f64,

    /// Confidence score for speaker identification
    pub confidence: Option<f32>,
}

/// Word-level segment with precise timing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordSegment {
    /// Word text
    pub word: String,

    /// Start time in seconds
    pub start: f64,

    /// End time in seconds
    pub end: f64,

    /// Confidence score (0.0-1.0)
    pub confidence: Option<f32>,

    /// Speaker ID (if available)
    pub speaker: Option<String>,
}

/// Statistics for transcription operations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TranscriptionStats {
    /// Total requests processed
    pub total_requests: u64,

    /// Successful transcriptions
    pub successful: u64,

    /// Failed transcriptions
    pub failed: u64,

    /// Currently processing
    pub processing: usize,

    /// Queue depth
    pub queue_depth: usize,

    /// Average processing time in ms
    pub avg_processing_time_ms: f64,

    /// Total audio duration processed (seconds)
    pub total_audio_duration: f64,

    /// Service uptime
    pub uptime_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcription_status() {
        assert_eq!(TranscriptionStatus::default(), TranscriptionStatus::Pending);
        assert_eq!(format!("{}", TranscriptionStatus::Completed), "completed");
    }

    #[test]
    fn test_transcription_request() {
        let call_id = Uuid::new_v4();
        let path = PathBuf::from("/test/audio.mp3");

        let request = TranscriptionRequest::new(call_id, path.clone());
        assert_eq!(request.call_id, call_id);
        assert_eq!(request.audio_path, path);
        assert_eq!(request.retry_count, 0);
        assert_eq!(request.priority, 0);
    }

    #[test]
    fn test_transcription_request_with_priority() {
        let call_id = Uuid::new_v4();
        let path = PathBuf::from("/test/audio.mp3");

        let request = TranscriptionRequest::new(call_id, path).with_priority(10);
        assert_eq!(request.priority, 10);
    }

    #[test]
    fn test_transcription_options_default() {
        let options = TranscriptionOptions::default();
        assert!(options.diarize);
        assert!(options.vad);
        assert!(options.word_timestamps);
        assert_eq!(options.max_duration, Some(3600.0));
    }

    #[test]
    fn test_transcription_config_default() {
        let config = TranscriptionConfig::default();
        assert!(config.enabled);
        assert_eq!(config.service, "whisperx");
        assert_eq!(config.model_size, "large-v3");
        assert_eq!(config.device, "cpu");
        assert_eq!(config.workers, 2);
    }

    #[test]
    fn test_segment_creation() {
        let segment = TranscriptionSegment {
            id: 1,
            start: 0.0,
            end: 5.5,
            text: "Hello world".to_string(),
            confidence: Some(0.95),
            speaker: Some("SPEAKER_00".to_string()),
            words: None,
        };

        assert_eq!(segment.id, 1);
        assert_eq!(segment.start, 0.0);
        assert_eq!(segment.end, 5.5);
        assert_eq!(segment.text, "Hello world");
    }

    #[test]
    fn test_speaker_segment() {
        let segment = SpeakerSegment {
            speaker: "SPEAKER_01".to_string(),
            start: 10.0,
            end: 15.5,
            confidence: Some(0.88),
        };

        assert_eq!(segment.speaker, "SPEAKER_01");
        assert_eq!(segment.start, 10.0);
        assert_eq!(segment.end, 15.5);
    }

    #[test]
    fn test_word_segment() {
        let word = WordSegment {
            word: "transcription".to_string(),
            start: 1.2,
            end: 1.8,
            confidence: Some(0.92),
            speaker: Some("SPEAKER_00".to_string()),
        };

        assert_eq!(word.word, "transcription");
        assert_eq!(word.start, 1.2);
        assert_eq!(word.end, 1.8);
    }

    #[test]
    fn test_stats_default() {
        let stats = TranscriptionStats::default();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.successful, 0);
        assert_eq!(stats.failed, 0);
    }
}