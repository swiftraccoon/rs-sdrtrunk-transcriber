//! Mock transcription service for testing

use crate::error::TranscriptionResult;
use crate::service::{
    AudioValidation, ServiceCapabilities, ServiceHealth, TranscriptionService,
};
use crate::types::{
    SpeakerSegment, TranscriptionConfig, TranscriptionRequest, TranscriptionResponse,
    TranscriptionSegment, TranscriptionStats, TranscriptionStatus, WordSegment,
};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};
use uuid::Uuid;

/// Mock transcription service for testing
#[derive(Debug)]
pub struct MockTranscriptionService {
    /// Configuration
    config: TranscriptionConfig,

    /// Whether service is initialized
    initialized: bool,

    /// Mock processing delay
    processing_delay_ms: u64,

    /// Should fail transcriptions
    should_fail: bool,

    /// Failure message
    failure_message: String,

    /// Request tracking
    requests: Arc<Mutex<HashMap<Uuid, TranscriptionStatus>>>,

    /// Statistics
    stats: Arc<Mutex<TranscriptionStats>>,
}

impl MockTranscriptionService {
    /// Create a new mock service
    pub fn new() -> Self {
        Self {
            config: TranscriptionConfig::default(),
            initialized: false,
            processing_delay_ms: 100,
            should_fail: false,
            failure_message: "Mock failure".to_string(),
            requests: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(TranscriptionStats::default())),
        }
    }

    /// Set processing delay for testing
    pub fn with_delay(mut self, delay_ms: u64) -> Self {
        self.processing_delay_ms = delay_ms;
        self
    }

    /// Configure to fail transcriptions
    pub fn with_failure(mut self, message: impl Into<String>) -> Self {
        self.should_fail = true;
        self.failure_message = message.into();
        self
    }

    /// Generate mock transcription text
    fn generate_mock_text(&self, request: &TranscriptionRequest) -> String {
        format!(
            "Mock transcription for file: {}. This is a test transcription with multiple sentences. \
             The audio quality was good and the speakers were clear.",
            request.audio_path.display()
        )
    }

    /// Generate mock segments
    fn generate_mock_segments(&self, text: &str) -> Vec<TranscriptionSegment> {
        let sentences: Vec<&str> = text.split(". ").collect();
        let mut segments = Vec::new();
        let mut current_time = 0.0;

        for (i, sentence) in sentences.iter().enumerate() {
            let duration = 3.5; // Mock 3.5 seconds per sentence
            segments.push(TranscriptionSegment {
                id: i,
                start: current_time,
                end: current_time + duration,
                text: format!("{}.", sentence.trim_end_matches('.')),
                confidence: Some(0.85 + (i as f32 * 0.02)),
                speaker: Some(format!("SPEAKER_{:02}", i % 2)),
                words: None,
            });
            current_time += duration + 0.5; // 0.5 second gap
        }

        segments
    }

    /// Generate mock speaker segments
    fn generate_mock_speaker_segments(&self) -> Vec<SpeakerSegment> {
        vec![
            SpeakerSegment {
                speaker: "SPEAKER_00".to_string(),
                start: 0.0,
                end: 5.0,
                confidence: Some(0.92),
            },
            SpeakerSegment {
                speaker: "SPEAKER_01".to_string(),
                start: 5.5,
                end: 10.0,
                confidence: Some(0.88),
            },
            SpeakerSegment {
                speaker: "SPEAKER_00".to_string(),
                start: 10.5,
                end: 15.0,
                confidence: Some(0.90),
            },
        ]
    }

    /// Generate mock words
    fn generate_mock_words(&self, text: &str) -> Vec<WordSegment> {
        let mut words = Vec::new();
        let mut current_time = 0.0;

        for word_text in text.split_whitespace().take(20) {
            // Just first 20 words for mock
            let duration = 0.3; // Mock 0.3 seconds per word
            words.push(WordSegment {
                word: word_text.to_string(),
                start: current_time,
                end: current_time + duration,
                confidence: Some(0.9),
                speaker: Some(format!("SPEAKER_{:02}", (current_time as i32) % 2)),
            });
            current_time += duration + 0.1; // 0.1 second gap
        }

        words
    }
}

impl Default for MockTranscriptionService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TranscriptionService for MockTranscriptionService {
    async fn initialize(&mut self, config: &TranscriptionConfig) -> TranscriptionResult<()> {
        self.config = config.clone();
        self.initialized = true;
        Ok(())
    }

    async fn shutdown(&mut self) -> TranscriptionResult<()> {
        self.initialized = false;
        Ok(())
    }

    async fn transcribe(
        &self,
        request: &TranscriptionRequest,
    ) -> TranscriptionResult<TranscriptionResponse> {
        // Track request
        {
            let mut requests = self.requests.lock().unwrap();
            requests.insert(request.id, TranscriptionStatus::Processing);
        }

        // Update stats
        {
            let mut stats = self.stats.lock().unwrap();
            stats.total_requests += 1;
            stats.processing += 1;
        }

        // Simulate processing delay
        if self.processing_delay_ms > 0 {
            sleep(Duration::from_millis(self.processing_delay_ms)).await;
        }

        // Update stats after processing
        {
            let mut stats = self.stats.lock().unwrap();
            stats.processing = stats.processing.saturating_sub(1);
        }

        // Check if should fail
        if self.should_fail {
            let mut requests = self.requests.lock().unwrap();
            requests.insert(request.id, TranscriptionStatus::Failed);

            let mut stats = self.stats.lock().unwrap();
            stats.failed += 1;

            return Err(crate::error::TranscriptionError::processing_failed(
                &self.failure_message,
            ));
        }

        // Generate mock response
        let text = self.generate_mock_text(request);
        let segments = self.generate_mock_segments(&text);
        let speaker_segments = if request.options.diarize {
            self.generate_mock_speaker_segments()
        } else {
            Vec::new()
        };
        let words = if request.options.word_timestamps {
            self.generate_mock_words(&text)
        } else {
            Vec::new()
        };

        let response = TranscriptionResponse {
            request_id: request.id,
            call_id: request.call_id,
            status: TranscriptionStatus::Completed,
            text: Some(text),
            language: Some("en".to_string()),
            confidence: Some(0.89),
            processing_time_ms: self.processing_delay_ms,
            segments,
            speaker_segments: speaker_segments.clone(),
            speaker_count: if request.options.diarize {
                Some(2)
            } else {
                None
            },
            words,
            error: None,
            completed_at: Utc::now(),
        };

        // Update tracking
        {
            let mut requests = self.requests.lock().unwrap();
            requests.insert(request.id, TranscriptionStatus::Completed);
        }

        // Update stats
        {
            let mut stats = self.stats.lock().unwrap();
            stats.successful += 1;
            stats.avg_processing_time_ms =
                (stats.avg_processing_time_ms * (stats.successful - 1) as f64
                    + self.processing_delay_ms as f64)
                    / stats.successful as f64;
            stats.total_audio_duration += 15.0; // Mock 15 seconds of audio
        }

        Ok(response)
    }

    async fn health_check(&self) -> TranscriptionResult<ServiceHealth> {
        Ok(if self.initialized {
            ServiceHealth::healthy("Mock service operational")
        } else {
            ServiceHealth::unhealthy("Mock service not initialized")
        })
    }

    async fn get_stats(&self) -> TranscriptionResult<TranscriptionStats> {
        Ok(self.stats.lock().unwrap().clone())
    }

    async fn get_status(&self, request_id: Uuid) -> TranscriptionResult<TranscriptionStatus> {
        let requests = self.requests.lock().unwrap();
        Ok(requests
            .get(&request_id)
            .copied()
            .unwrap_or(TranscriptionStatus::Pending))
    }

    async fn cancel(&self, request_id: Uuid) -> TranscriptionResult<()> {
        let mut requests = self.requests.lock().unwrap();
        requests.insert(request_id, TranscriptionStatus::Cancelled);
        Ok(())
    }

    async fn validate_audio(&self, path: &Path) -> TranscriptionResult<AudioValidation> {
        // Simple mock validation
        if !path.exists() {
            return Ok(AudioValidation::invalid("File does not exist", 0));
        }

        let metadata = tokio::fs::metadata(path).await?;
        let file_size = metadata.len();

        // Check extension
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !["mp3", "wav", "flac"].contains(&extension.as_str()) {
            return Ok(AudioValidation::invalid(
                format!("Unsupported format: {extension}"),
                file_size,
            ));
        }

        Ok(AudioValidation::valid(extension, 30.0, file_size))
    }

    fn capabilities(&self) -> ServiceCapabilities {
        ServiceCapabilities {
            diarization: true,
            word_timestamps: true,
            vad: false,
            language_detection: false,
            supported_formats: vec![
                "mp3".to_string(),
                "wav".to_string(),
                "flac".to_string(),
            ],
            max_duration_seconds: Some(3600.0),
            supported_languages: vec!["en".to_string()],
            batch_processing: false,
            streaming: false,
            gpu_acceleration: false,
        }
    }

    fn name(&self) -> &str {
        "mock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_mock_service_initialization() {
        let mut service = MockTranscriptionService::new();
        assert!(!service.initialized);

        let config = TranscriptionConfig::default();
        service.initialize(&config).await.unwrap();
        assert!(service.initialized);

        service.shutdown().await.unwrap();
        assert!(!service.initialized);
    }

    #[tokio::test]
    async fn test_mock_service_transcribe() {
        let mut service = MockTranscriptionService::new();
        let config = TranscriptionConfig::default();
        service.initialize(&config).await.unwrap();

        let request = TranscriptionRequest::new(
            Uuid::new_v4(),
            PathBuf::from("/test/audio.mp3"),
        );

        let response = service.transcribe(&request).await.unwrap();
        assert_eq!(response.status, TranscriptionStatus::Completed);
        assert!(response.text.is_some());
        assert!(!response.segments.is_empty());
    }

    #[tokio::test]
    async fn test_mock_service_with_failure() {
        let mut service = MockTranscriptionService::new()
            .with_failure("Test failure");
        let config = TranscriptionConfig::default();
        service.initialize(&config).await.unwrap();

        let request = TranscriptionRequest::new(
            Uuid::new_v4(),
            PathBuf::from("/test/audio.mp3"),
        );

        let result = service.transcribe(&request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_service_stats() {
        let mut service = MockTranscriptionService::new();
        let config = TranscriptionConfig::default();
        service.initialize(&config).await.unwrap();

        let initial_stats = service.get_stats().await.unwrap();
        assert_eq!(initial_stats.total_requests, 0);

        let request = TranscriptionRequest::new(
            Uuid::new_v4(),
            PathBuf::from("/test/audio.mp3"),
        );
        service.transcribe(&request).await.unwrap();

        let stats = service.get_stats().await.unwrap();
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful, 1);
    }

    #[tokio::test]
    async fn test_mock_service_health_check() {
        let mut service = MockTranscriptionService::new();

        let health = service.health_check().await.unwrap();
        assert!(!health.healthy);

        let config = TranscriptionConfig::default();
        service.initialize(&config).await.unwrap();

        let health = service.health_check().await.unwrap();
        assert!(health.healthy);
    }

    #[tokio::test]
    async fn test_mock_service_status_tracking() {
        let mut service = MockTranscriptionService::new();
        let config = TranscriptionConfig::default();
        service.initialize(&config).await.unwrap();

        let request = TranscriptionRequest::new(
            Uuid::new_v4(),
            PathBuf::from("/test/audio.mp3"),
        );
        let request_id = request.id;

        service.transcribe(&request).await.unwrap();

        let status = service.get_status(request_id).await.unwrap();
        assert_eq!(status, TranscriptionStatus::Completed);
    }

    #[tokio::test]
    async fn test_mock_service_capabilities() {
        let service = MockTranscriptionService::new();
        let caps = service.capabilities();

        assert!(caps.diarization);
        assert!(caps.word_timestamps);
        assert!(caps.supports_format("mp3"));
        assert_eq!(caps.supported_languages.len(), 1);
    }

    #[test]
    fn test_mock_text_generation() {
        let service = MockTranscriptionService::new();
        let request = TranscriptionRequest::new(
            Uuid::new_v4(),
            PathBuf::from("/test/audio.mp3"),
        );

        let text = service.generate_mock_text(&request);
        assert!(text.contains("Mock transcription"));
        assert!(text.contains("audio.mp3"));
    }

    #[test]
    fn test_mock_segments_generation() {
        let service = MockTranscriptionService::new();
        let text = "First sentence. Second sentence. Third sentence.";
        let segments = service.generate_mock_segments(text);

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].id, 0);
        assert!(segments[0].speaker.is_some());
    }

    #[test]
    fn test_mock_speaker_segments() {
        let service = MockTranscriptionService::new();
        let segments = service.generate_mock_speaker_segments();

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].speaker, "SPEAKER_00");
        assert_eq!(segments[1].speaker, "SPEAKER_01");
    }

    #[test]
    fn test_mock_words_generation() {
        let service = MockTranscriptionService::new();
        let text = "This is a test transcription with many words";
        let words = service.generate_mock_words(text);

        assert!(!words.is_empty());
        assert_eq!(words[0].word, "This");
        assert!(words[0].confidence.is_some());
    }
}