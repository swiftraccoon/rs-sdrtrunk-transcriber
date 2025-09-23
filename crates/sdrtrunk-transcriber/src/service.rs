//! Core transcription service trait and implementation utilities

use crate::error::TranscriptionResult;
use crate::types::{
    TranscriptionConfig, TranscriptionRequest, TranscriptionResponse, TranscriptionStats,
    TranscriptionStatus,
};
use async_trait::async_trait;
use std::path::Path;
use uuid::Uuid;

/// Core trait for transcription service implementations
///
/// This trait defines the interface that all transcription backends must implement,
/// allowing for pluggable transcription services (WhisperX, mock, etc.).
#[async_trait]
pub trait TranscriptionService: Send + Sync {
    /// Initialize the transcription service
    ///
    /// This should load models, start background services, etc.
    async fn initialize(&mut self, config: &TranscriptionConfig) -> TranscriptionResult<()>;

    /// Shutdown the transcription service gracefully
    ///
    /// This should clean up resources, stop background services, etc.
    async fn shutdown(&mut self) -> TranscriptionResult<()>;

    /// Process a transcription request
    ///
    /// This is the main transcription method that processes audio files
    async fn transcribe(
        &self,
        request: &TranscriptionRequest,
    ) -> TranscriptionResult<TranscriptionResponse>;

    /// Check if the service is healthy and ready
    async fn health_check(&self) -> TranscriptionResult<ServiceHealth>;

    /// Get service statistics
    async fn get_stats(&self) -> TranscriptionResult<TranscriptionStats>;

    /// Get the status of a specific transcription request
    async fn get_status(&self, request_id: Uuid) -> TranscriptionResult<TranscriptionStatus>;

    /// Cancel a transcription request
    async fn cancel(&self, request_id: Uuid) -> TranscriptionResult<()>;

    /// Validate that an audio file can be processed
    async fn validate_audio(&self, path: &Path) -> TranscriptionResult<AudioValidation>;

    /// Get service capabilities
    fn capabilities(&self) -> ServiceCapabilities;

    /// Get service name
    fn name(&self) -> &str;
}

/// Service health status
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServiceHealth {
    /// Whether the service is healthy
    pub healthy: bool,

    /// Service status message
    pub status: String,

    /// Model loaded status
    pub model_loaded: bool,

    /// Available memory (bytes)
    pub available_memory: Option<u64>,

    /// GPU available (for GPU-based services)
    pub gpu_available: Option<bool>,

    /// Current queue depth
    pub queue_depth: usize,

    /// Active workers
    pub active_workers: usize,

    /// Last health check timestamp
    pub checked_at: chrono::DateTime<chrono::Utc>,
}

impl ServiceHealth {
    /// Create a healthy status
    pub fn healthy(status: impl Into<String>) -> Self {
        Self {
            healthy: true,
            status: status.into(),
            model_loaded: true,
            available_memory: None,
            gpu_available: None,
            queue_depth: 0,
            active_workers: 0,
            checked_at: chrono::Utc::now(),
        }
    }

    /// Create an unhealthy status
    pub fn unhealthy(status: impl Into<String>) -> Self {
        Self {
            healthy: false,
            status: status.into(),
            model_loaded: false,
            available_memory: None,
            gpu_available: None,
            queue_depth: 0,
            active_workers: 0,
            checked_at: chrono::Utc::now(),
        }
    }
}

/// Audio file validation results
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioValidation {
    /// Whether the audio file is valid
    pub valid: bool,

    /// Audio format (mp3, wav, etc.)
    pub format: Option<String>,

    /// Duration in seconds
    pub duration_seconds: Option<f64>,

    /// Sample rate
    pub sample_rate: Option<u32>,

    /// Number of channels
    pub channels: Option<u16>,

    /// File size in bytes
    pub file_size: u64,

    /// Validation issues (if any)
    pub issues: Vec<String>,
}

impl AudioValidation {
    /// Create a valid result
    pub fn valid(format: String, duration: f64, file_size: u64) -> Self {
        Self {
            valid: true,
            format: Some(format),
            duration_seconds: Some(duration),
            sample_rate: None,
            channels: None,
            file_size,
            issues: Vec::new(),
        }
    }

    /// Create an invalid result
    pub fn invalid(reason: impl Into<String>, file_size: u64) -> Self {
        Self {
            valid: false,
            format: None,
            duration_seconds: None,
            sample_rate: None,
            channels: None,
            file_size,
            issues: vec![reason.into()],
        }
    }

    /// Add an issue to the validation
    pub fn add_issue(&mut self, issue: impl Into<String>) {
        self.issues.push(issue.into());
        if !self.issues.is_empty() {
            self.valid = false;
        }
    }
}

/// Service capabilities description
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServiceCapabilities {
    /// Supports speaker diarization
    pub diarization: bool,

    /// Supports word-level timestamps
    pub word_timestamps: bool,

    /// Supports VAD preprocessing
    pub vad: bool,

    /// Supports language detection
    pub language_detection: bool,

    /// Supported audio formats
    pub supported_formats: Vec<String>,

    /// Maximum audio duration (seconds)
    pub max_duration_seconds: Option<f64>,

    /// Supported languages
    pub supported_languages: Vec<String>,

    /// Supports batch processing
    pub batch_processing: bool,

    /// Supports streaming/real-time
    pub streaming: bool,

    /// GPU acceleration available
    pub gpu_acceleration: bool,
}

impl Default for ServiceCapabilities {
    fn default() -> Self {
        Self {
            diarization: false,
            word_timestamps: false,
            vad: false,
            language_detection: false,
            supported_formats: vec!["mp3".to_string(), "wav".to_string()],
            max_duration_seconds: Some(3600.0),
            supported_languages: vec!["en".to_string()],
            batch_processing: false,
            streaming: false,
            gpu_acceleration: false,
        }
    }
}

impl ServiceCapabilities {
    /// Create WhisperX capabilities
    pub fn whisperx() -> Self {
        Self {
            diarization: true,
            word_timestamps: true,
            vad: true,
            language_detection: true,
            supported_formats: vec![
                "mp3".to_string(),
                "wav".to_string(),
                "flac".to_string(),
                "m4a".to_string(),
                "ogg".to_string(),
            ],
            max_duration_seconds: None,
            supported_languages: vec![
                "en".to_string(),
                "es".to_string(),
                "fr".to_string(),
                "de".to_string(),
                "it".to_string(),
                "pt".to_string(),
                "ru".to_string(),
                "zh".to_string(),
                "ja".to_string(),
                "ko".to_string(),
            ],
            batch_processing: true,
            streaming: false,
            gpu_acceleration: true,
        }
    }

    /// Check if a format is supported
    pub fn supports_format(&self, format: &str) -> bool {
        self.supported_formats
            .iter()
            .any(|f| f.eq_ignore_ascii_case(format))
    }

    /// Check if a language is supported
    pub fn supports_language(&self, language: &str) -> bool {
        self.supported_languages
            .iter()
            .any(|l| l.eq_ignore_ascii_case(language))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_health() {
        let health = ServiceHealth::healthy("All systems operational");
        assert!(health.healthy);
        assert!(health.model_loaded);

        let unhealthy = ServiceHealth::unhealthy("Model failed to load");
        assert!(!unhealthy.healthy);
        assert!(!unhealthy.model_loaded);
    }

    #[test]
    fn test_audio_validation() {
        let valid = AudioValidation::valid("mp3".to_string(), 30.5, 1024000);
        assert!(valid.valid);
        assert_eq!(valid.format.as_deref(), Some("mp3"));
        assert_eq!(valid.duration_seconds, Some(30.5));
        assert!(valid.issues.is_empty());

        let invalid = AudioValidation::invalid("Unsupported format", 500);
        assert!(!invalid.valid);
        assert_eq!(invalid.issues.len(), 1);
    }

    #[test]
    fn test_audio_validation_add_issue() {
        let mut validation = AudioValidation::valid("mp3".to_string(), 10.0, 1000);
        assert!(validation.valid);

        validation.add_issue("Duration too long");
        assert!(!validation.valid);
        assert_eq!(validation.issues.len(), 1);
    }

    #[test]
    fn test_service_capabilities_default() {
        let caps = ServiceCapabilities::default();
        assert!(!caps.diarization);
        assert!(caps.supports_format("mp3"));
        assert!(caps.supports_format("WAV")); // Case insensitive
        assert!(!caps.supports_format("aac"));
    }

    #[test]
    fn test_service_capabilities_whisperx() {
        let caps = ServiceCapabilities::whisperx();
        assert!(caps.diarization);
        assert!(caps.word_timestamps);
        assert!(caps.vad);
        assert!(caps.language_detection);
        assert!(caps.gpu_acceleration);
        assert!(caps.supports_format("flac"));
        assert!(caps.supports_language("en"));
        assert!(caps.supports_language("ES")); // Case insensitive
    }

    #[test]
    fn test_capabilities_format_check() {
        let caps = ServiceCapabilities::default();
        assert!(caps.supports_format("mp3"));
        assert!(caps.supports_format("MP3"));
        assert!(caps.supports_format("wav"));
        assert!(!caps.supports_format("unknown"));
    }

    #[test]
    fn test_capabilities_language_check() {
        let caps = ServiceCapabilities::whisperx();
        assert!(caps.supports_language("en"));
        assert!(caps.supports_language("EN"));
        assert!(caps.supports_language("fr"));
        assert!(!caps.supports_language("unknown"));
    }
}