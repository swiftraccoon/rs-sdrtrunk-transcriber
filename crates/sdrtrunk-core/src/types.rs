//! Core data types for `SDRTrunk` transcriber

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// System identifier type
pub type SystemId = String;

/// Talkgroup identifier type
pub type TalkgroupId = i32;

/// Radio identifier type
pub type RadioId = i32;

/// Transcription status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    /// No transcription requested
    None,
}

impl Default for TranscriptionStatus {
    fn default() -> Self {
        Self::None
    }
}

impl std::fmt::Display for TranscriptionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Processing => write!(f, "processing"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::None => write!(f, "none"),
        }
    }
}

/// Represents a radio call from `SDRTrunk`
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RadioCall {
    /// Unique identifier for the call
    pub id: Option<Uuid>,

    /// When the call was created in our system
    pub created_at: DateTime<Utc>,

    /// When the call occurred
    pub call_timestamp: DateTime<Utc>,

    /// System identification
    #[validate(length(min = 1, max = 50))]
    pub system_id: SystemId,

    /// System label/name
    #[validate(length(max = 255))]
    pub system_label: Option<String>,

    /// Frequency in Hz
    pub frequency: Option<i64>,

    /// Talkgroup ID
    pub talkgroup_id: Option<TalkgroupId>,

    /// Talkgroup label/name
    #[validate(length(max = 255))]
    pub talkgroup_label: Option<String>,

    /// Talkgroup group
    #[validate(length(max = 255))]
    pub talkgroup_group: Option<String>,

    /// Talkgroup tag
    #[validate(length(max = 255))]
    pub talkgroup_tag: Option<String>,

    /// Source radio ID
    pub source_radio_id: Option<RadioId>,

    /// Talker alias
    #[validate(length(max = 255))]
    pub talker_alias: Option<String>,

    /// Audio file information
    pub audio_filename: Option<String>,
    /// Path to the audio file
    pub audio_file_path: Option<String>,
    /// Size of audio file in bytes
    pub audio_size_bytes: Option<i64>,
    /// Duration of audio in seconds
    pub duration_seconds: Option<f64>,

    /// Upload tracking
    pub upload_ip: Option<String>,
    /// When the call was uploaded
    pub upload_timestamp: DateTime<Utc>,
    /// API key used for upload
    pub upload_api_key_id: Option<String>,

    /// Additional metadata from `SDRTrunk`
    pub patches: Option<serde_json::Value>,
    /// Frequency information
    pub frequencies: Option<serde_json::Value>,
    /// Source information
    pub sources: Option<serde_json::Value>,

    /// Transcription information
    pub transcription_status: TranscriptionStatus,
    /// Transcribed text content
    pub transcription_text: Option<String>,
    /// Confidence score of transcription (0.0-1.0)
    pub transcription_confidence: Option<f32>,
    /// Error message if transcription failed
    pub transcription_error: Option<String>,
    /// When transcription started
    pub transcription_started_at: Option<DateTime<Utc>>,
    /// When transcription completed
    pub transcription_completed_at: Option<DateTime<Utc>>,

    /// Speaker diarization results
    pub speaker_count: Option<i32>,
    /// Speaker segment information
    pub speaker_segments: Option<serde_json::Value>,
    /// Transcription segments with timestamps
    pub transcription_segments: Option<serde_json::Value>,
}

impl Default for RadioCall {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: None,
            created_at: now,
            call_timestamp: now,
            system_id: String::new(),
            system_label: None,
            frequency: None,
            talkgroup_id: None,
            talkgroup_label: None,
            talkgroup_group: None,
            talkgroup_tag: None,
            source_radio_id: None,
            talker_alias: None,
            audio_filename: None,
            audio_file_path: None,
            audio_size_bytes: None,
            duration_seconds: None,
            upload_ip: None,
            upload_timestamp: now,
            upload_api_key_id: None,
            patches: None,
            frequencies: None,
            sources: None,
            transcription_status: TranscriptionStatus::default(),
            transcription_text: None,
            transcription_confidence: None,
            transcription_error: None,
            transcription_started_at: None,
            transcription_completed_at: None,
            speaker_count: None,
            speaker_segments: None,
            transcription_segments: None,
        }
    }
}

/// File data extracted from `SDRTrunk` filename
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileData {
    /// Date in YYYYMMDD format
    pub date: String,

    /// Time in HHMMSS format
    pub time: String,

    /// Unix timestamp
    pub unixtime: i64,

    /// Talkgroup ID
    pub talkgroup_id: TalkgroupId,

    /// Talkgroup name
    pub talkgroup_name: String,

    /// Radio ID
    pub radio_id: RadioId,

    /// Duration string
    pub duration: String,

    /// Original filename
    pub filename: String,

    /// Full file path
    pub filepath: String,
}

/// Upload status for tracking file processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UploadStatus {
    /// File received and pending processing
    Pending,
    /// File is being processed
    Processing,
    /// File processed successfully
    Completed,
    /// File processing failed
    Failed(String),
}

/// API key configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// Unique key ID
    pub id: String,

    /// API key (hashed)
    pub key_hash: String,

    /// Description of the key
    pub name: String,

    /// When the key was created
    pub created_at: DateTime<Utc>,

    /// When the key expires (if applicable)
    pub expires_at: Option<DateTime<Utc>>,

    /// Allowed IP addresses
    pub allowed_ips: Vec<String>,

    /// Allowed system IDs
    pub allowed_systems: Vec<SystemId>,

    /// Is the key active?
    pub active: bool,
}

/// System statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    /// System ID
    pub system_id: SystemId,

    /// System label
    pub system_label: Option<String>,

    /// Total number of calls
    pub total_calls: i64,

    /// Calls received today
    pub calls_today: i64,

    /// Calls received this hour
    pub calls_this_hour: i64,

    /// When the system was first seen
    pub first_seen: Option<DateTime<Utc>>,

    /// When the system was last seen
    pub last_seen: Option<DateTime<Utc>>,

    /// Top talkgroups
    pub top_talkgroups: Option<serde_json::Value>,

    /// Last update time
    pub last_updated: DateTime<Utc>,
}

/// Audio format information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioInfo {
    /// File format (wav, mp3, flac, etc.)
    pub format: String,

    /// Sample rate in Hz
    pub sample_rate: u32,

    /// Number of channels
    pub channels: u16,

    /// Bit depth
    pub bit_depth: Option<u16>,

    /// Duration in seconds
    pub duration_seconds: f64,

    /// File size in bytes
    pub file_size: u64,
}

/// Processing result for audio files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingResult {
    /// Whether processing was successful
    pub success: bool,

    /// Error message if processing failed
    pub error: Option<String>,

    /// Audio information
    pub audio_info: Option<AudioInfo>,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,

    /// Checksum of processed file
    pub checksum: Option<String>,
}

/// Pagination information for API responses
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaginationInfo {
    /// Current page number (1-based)
    pub page: u32,

    /// Number of items per page
    pub per_page: u32,

    /// Total number of items
    pub total_count: u64,

    /// Total number of pages
    pub total_pages: u32,

    /// Whether there are more pages
    pub has_next: bool,

    /// Whether there are previous pages
    pub has_prev: bool,
}

/// API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// Response data
    pub data: T,

    /// Success status
    pub success: bool,

    /// Optional message
    pub message: Option<String>,

    /// Request timestamp
    pub timestamp: DateTime<Utc>,

    /// Pagination info (for paginated responses)
    pub pagination: Option<PaginationInfo>,
}

impl<T> ApiResponse<T> {
    /// Create a successful response
    pub fn success(data: T) -> Self {
        Self {
            data,
            success: true,
            message: None,
            timestamp: Utc::now(),
            pagination: None,
        }
    }

    /// Create a successful response with message
    pub fn success_with_message(data: T, message: impl Into<String>) -> Self {
        Self {
            data,
            success: true,
            message: Some(message.into()),
            timestamp: Utc::now(),
            pagination: None,
        }
    }

    /// Create a paginated response
    pub fn paginated(data: T, pagination: PaginationInfo) -> Self {
        Self {
            data,
            success: true,
            message: None,
            timestamp: Utc::now(),
            pagination: Some(pagination),
        }
    }
}

/// Error response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,

    /// Error code
    pub code: String,

    /// Success status (always false for errors)
    pub success: bool,

    /// Error timestamp
    pub timestamp: DateTime<Utc>,

    /// Optional additional details
    pub details: Option<serde_json::Value>,
}

impl ErrorResponse {
    /// Create a new error response
    pub fn new(error: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            code: code.into(),
            success: false,
            timestamp: Utc::now(),
            details: None,
        }
    }

    /// Create an error response with details
    pub fn with_details(
        error: impl Into<String>,
        code: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self {
            error: error.into(),
            code: code.into(),
            success: false,
            timestamp: Utc::now(),
            details: Some(details),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unreadable_literal,
    clippy::missing_panics_doc,
    clippy::field_reassign_with_default,
    clippy::float_cmp,
    clippy::uninlined_format_args,
    clippy::match_same_arms
)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;
    use serde_json::json;
    use validator::Validate;

    #[test]
    fn test_transcription_status_default() {
        assert_eq!(TranscriptionStatus::default(), TranscriptionStatus::None);
    }

    #[test]
    fn test_transcription_status_display() {
        assert_eq!(format!("{}", TranscriptionStatus::Pending), "pending");
        assert_eq!(format!("{}", TranscriptionStatus::Processing), "processing");
        assert_eq!(format!("{}", TranscriptionStatus::Completed), "completed");
        assert_eq!(format!("{}", TranscriptionStatus::Failed), "failed");
        assert_eq!(format!("{}", TranscriptionStatus::None), "none");
    }

    #[test]
    fn test_transcription_status_serialization() {
        let status = TranscriptionStatus::Completed;
        let serialized = serde_json::to_string(&status).unwrap();
        assert_eq!(serialized, "\"completed\"");

        let deserialized: TranscriptionStatus = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, TranscriptionStatus::Completed);
    }

    #[test]
    fn test_radio_call_default() {
        let call = RadioCall::default();
        assert_eq!(call.system_id, "");
        assert_eq!(call.transcription_status, TranscriptionStatus::None);
        assert!(call.id.is_none());
        assert!(call.system_label.is_none());
    }

    #[test]
    fn test_radio_call_validation_valid() {
        let mut call = RadioCall::default();
        call.system_id = "test_system".to_string();
        call.system_label = Some("Test System".to_string());

        assert!(call.validate().is_ok());
    }

    #[test]
    fn test_radio_call_validation_system_id_too_long() {
        let mut call = RadioCall::default();
        call.system_id = "a".repeat(51); // Exceeds 50 character limit

        let result = call.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.field_errors().contains_key("system_id"));
    }

    #[test]
    fn test_radio_call_validation_system_id_empty() {
        let call = RadioCall::default(); // system_id is empty string

        let result = call.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.field_errors().contains_key("system_id"));
    }

    #[test]
    fn test_radio_call_validation_label_too_long() {
        let mut call = RadioCall::default();
        call.system_id = "valid_system".to_string();
        call.system_label = Some("a".repeat(256)); // Exceeds 255 character limit

        let result = call.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.field_errors().contains_key("system_label"));
    }

    #[test]
    fn test_radio_call_serialization() {
        let mut call = RadioCall::default();
        call.system_id = "test_system".to_string();
        call.talkgroup_id = Some(12345);
        call.frequency = Some(854_000_000);
        call.transcription_status = TranscriptionStatus::Completed;
        call.transcription_text = Some("Test transcription".to_string());

        let serialized = serde_json::to_string(&call).unwrap();
        let deserialized: RadioCall = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.system_id, call.system_id);
        assert_eq!(deserialized.talkgroup_id, call.talkgroup_id);
        assert_eq!(deserialized.frequency, call.frequency);
        assert_eq!(deserialized.transcription_status, call.transcription_status);
        assert_eq!(deserialized.transcription_text, call.transcription_text);
    }

    #[test]
    fn test_file_data_creation() {
        let file_data = FileData {
            date: "20240315".to_string(),
            time: "142530".to_string(),
            unixtime: 1710509130,
            talkgroup_id: 52197,
            talkgroup_name: "TG52197".to_string(),
            radio_id: 1234567,
            duration: "00:05".to_string(),
            filename: "test.mp3".to_string(),
            filepath: "/path/to/test.mp3".to_string(),
        };

        assert_eq!(file_data.date, "20240315");
        assert_eq!(file_data.talkgroup_id, 52197);
        assert_eq!(file_data.radio_id, 1234567);
    }

    #[test]
    fn test_upload_status_variants() {
        let pending = UploadStatus::Pending;
        let processing = UploadStatus::Processing;
        let completed = UploadStatus::Completed;
        let failed = UploadStatus::Failed("Error message".to_string());

        // Test that all variants can be created and match expected patterns
        match pending {
            UploadStatus::Pending => {}
            _ => panic!("Expected Pending variant"),
        }

        match processing {
            UploadStatus::Processing => {}
            _ => panic!("Expected Processing variant"),
        }

        match completed {
            UploadStatus::Completed => {}
            _ => panic!("Expected Completed variant"),
        }

        match failed {
            UploadStatus::Failed(msg) => assert_eq!(msg, "Error message"),
            _ => panic!("Expected Failed variant"),
        }
    }

    #[test]
    fn test_api_key_creation() {
        let now = Utc::now();
        let api_key = ApiKey {
            id: "test_key_123".to_string(),
            key_hash: "hashed_key".to_string(),
            name: "Test Key".to_string(),
            created_at: now,
            expires_at: Some(now + chrono::Duration::days(30)),
            allowed_ips: vec!["192.168.1.1".to_string(), "10.0.0.0/8".to_string()],
            allowed_systems: vec!["system1".to_string(), "system2".to_string()],
            active: true,
        };

        assert_eq!(api_key.id, "test_key_123");
        assert_eq!(api_key.allowed_ips.len(), 2);
        assert_eq!(api_key.allowed_systems.len(), 2);
        assert!(api_key.active);
    }

    #[test]
    fn test_system_stats_creation() {
        let now = Utc::now();
        let stats = SystemStats {
            system_id: "test_system".to_string(),
            system_label: Some("Test System".to_string()),
            total_calls: 1000,
            calls_today: 50,
            calls_this_hour: 5,
            first_seen: Some(now - chrono::Duration::days(30)),
            last_seen: Some(now),
            top_talkgroups: Some(json!([
                {"id": 12345, "label": "Police", "count": 100},
                {"id": 67890, "label": "Fire", "count": 50}
            ])),
            last_updated: now,
        };

        assert_eq!(stats.system_id, "test_system");
        assert_eq!(stats.total_calls, 1000);
        assert_eq!(stats.calls_today, 50);
        assert_eq!(stats.calls_this_hour, 5);
        assert!(stats.top_talkgroups.is_some());
    }

    #[test]
    fn test_audio_info_creation() {
        let audio_info = AudioInfo {
            format: "mp3".to_string(),
            sample_rate: 44100,
            channels: 2,
            bit_depth: Some(16),
            duration_seconds: 30.5,
            file_size: 1024000,
        };

        assert_eq!(audio_info.format, "mp3");
        assert_eq!(audio_info.sample_rate, 44100);
        assert_eq!(audio_info.channels, 2);
        assert_eq!(audio_info.bit_depth, Some(16));
        assert_eq!(audio_info.duration_seconds, 30.5);
        assert_eq!(audio_info.file_size, 1024000);
    }

    #[test]
    fn test_processing_result_success() {
        let audio_info = AudioInfo {
            format: "mp3".to_string(),
            sample_rate: 44100,
            channels: 2,
            bit_depth: Some(16),
            duration_seconds: 30.5,
            file_size: 1024000,
        };

        let result = ProcessingResult {
            success: true,
            error: None,
            audio_info: Some(audio_info),
            processing_time_ms: 1500,
            checksum: Some("abc123".to_string()),
        };

        assert!(result.success);
        assert!(result.error.is_none());
        assert!(result.audio_info.is_some());
        assert_eq!(result.processing_time_ms, 1500);
        assert_eq!(result.checksum, Some("abc123".to_string()));
    }

    #[test]
    fn test_processing_result_failure() {
        let result = ProcessingResult {
            success: false,
            error: Some("Failed to process audio".to_string()),
            audio_info: None,
            processing_time_ms: 500,
            checksum: None,
        };

        assert!(!result.success);
        assert_eq!(result.error, Some("Failed to process audio".to_string()));
        assert!(result.audio_info.is_none());
        assert!(result.checksum.is_none());
    }

    #[test]
    fn test_pagination_info() {
        let pagination = PaginationInfo {
            page: 2,
            per_page: 10,
            total_count: 95,
            total_pages: 10,
            has_next: true,
            has_prev: true,
        };

        assert_eq!(pagination.page, 2);
        assert_eq!(pagination.per_page, 10);
        assert_eq!(pagination.total_count, 95);
        assert_eq!(pagination.total_pages, 10);
        assert!(pagination.has_next);
        assert!(pagination.has_prev);
    }

    #[test]
    fn test_api_response_success() {
        let data = vec!["item1", "item2", "item3"];
        let response = ApiResponse::success(data.clone());

        assert_eq!(response.data, data);
        assert!(response.success);
        assert!(response.message.is_none());
        assert!(response.pagination.is_none());
    }

    #[test]
    fn test_api_response_success_with_message() {
        let data = "test data";
        let message = "Operation completed successfully";
        let response = ApiResponse::success_with_message(data, message);

        assert_eq!(response.data, data);
        assert!(response.success);
        assert_eq!(response.message, Some(message.to_string()));
    }

    #[test]
    fn test_api_response_paginated() {
        let data = vec![1, 2, 3, 4, 5];
        let pagination = PaginationInfo {
            page: 1,
            per_page: 5,
            total_count: 20,
            total_pages: 4,
            has_next: true,
            has_prev: false,
        };

        let response = ApiResponse::paginated(data.clone(), pagination.clone());

        assert_eq!(response.data, data);
        assert!(response.success);
        assert!(response.message.is_none());
        assert_eq!(response.pagination, Some(pagination));
    }

    #[test]
    fn test_error_response_new() {
        let error = "Something went wrong";
        let code = "INTERNAL_ERROR";
        let response = ErrorResponse::new(error, code);

        assert_eq!(response.error, error);
        assert_eq!(response.code, code);
        assert!(!response.success);
        assert!(response.details.is_none());
    }

    #[test]
    fn test_error_response_with_details() {
        let error = "Validation failed";
        let code = "VALIDATION_ERROR";
        let details = json!({"field": "system_id", "message": "Required"});
        let response = ErrorResponse::with_details(error, code, details.clone());

        assert_eq!(response.error, error);
        assert_eq!(response.code, code);
        assert!(!response.success);
        assert_eq!(response.details, Some(details));
    }

    // Property-based tests using proptest
    proptest! {
        #[test]
        fn test_radio_call_system_id_validation(system_id in "\\PC{1,50}") {
            let mut call = RadioCall::default();
            call.system_id = system_id;
            prop_assert!(call.validate().is_ok());
        }

        #[test]
        fn test_radio_call_system_id_too_long_validation(system_id in "\\PC{51,100}") {
            let mut call = RadioCall::default();
            call.system_id = system_id;
            prop_assert!(call.validate().is_err());
        }

        #[test]
        fn test_transcription_status_roundtrip(status in prop_oneof![
            Just(TranscriptionStatus::None),
            Just(TranscriptionStatus::Pending),
            Just(TranscriptionStatus::Processing),
            Just(TranscriptionStatus::Completed),
            Just(TranscriptionStatus::Failed),
        ]) {
            let serialized = serde_json::to_string(&status).unwrap();
            let deserialized: TranscriptionStatus = serde_json::from_str(&serialized).unwrap();
            prop_assert_eq!(status, deserialized);
        }

        #[test]
        fn test_talkgroup_id_range(talkgroup_id in 1i32..=999999i32) {
            let file_data = FileData {
                date: "20240101".to_string(),
                time: "120000".to_string(),
                unixtime: 1704110400,
                talkgroup_id,
                talkgroup_name: format!("TG{}", talkgroup_id),
                radio_id: 12345,
                duration: "00:30".to_string(),
                filename: "test.mp3".to_string(),
                filepath: "/test/test.mp3".to_string(),
            };
            prop_assert_eq!(file_data.talkgroup_id, talkgroup_id);
        }
    }

    #[test]
    fn test_upload_status_serialization() {
        let statuses = vec![
            UploadStatus::Pending,
            UploadStatus::Processing,
            UploadStatus::Completed,
            UploadStatus::Failed("Test error".to_string()),
        ];

        for status in statuses {
            let serialized = serde_json::to_string(&status).unwrap();
            let deserialized: UploadStatus = serde_json::from_str(&serialized).unwrap();

            match (&status, &deserialized) {
                (UploadStatus::Pending, UploadStatus::Pending) => {}
                (UploadStatus::Processing, UploadStatus::Processing) => {}
                (UploadStatus::Completed, UploadStatus::Completed) => {}
                (UploadStatus::Failed(orig), UploadStatus::Failed(deser)) => {
                    assert_eq!(orig, deser);
                }
                _ => panic!("Serialization roundtrip failed"),
            }
        }
    }

    #[test]
    fn test_api_response_serialization() {
        let data = vec!["test1", "test2"];
        let response = ApiResponse::success_with_message(data.clone(), "Test message");

        let serialized = serde_json::to_string(&response).unwrap();
        let deserialized: ApiResponse<Vec<String>> = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.data, data);
        assert_eq!(deserialized.success, response.success);
        assert_eq!(deserialized.message, response.message);
    }

    #[test]
    fn test_complex_json_metadata() {
        let mut call = RadioCall::default();
        call.system_id = "test_system".to_string();
        call.patches = Some(json!({
            "patch_groups": [
                {"id": 1, "name": "Group 1"},
                {"id": 2, "name": "Group 2"}
            ]
        }));
        call.frequencies = Some(json!([854000000, 855000000, 856000000]));
        call.sources = Some(json!({
            "site_id": 1,
            "site_name": "Main Site",
            "antennas": ["North", "South", "East", "West"]
        }));

        let serialized = serde_json::to_string(&call).unwrap();
        let deserialized: RadioCall = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.patches, call.patches);
        assert_eq!(deserialized.frequencies, call.frequencies);
        assert_eq!(deserialized.sources, call.sources);
    }

    #[test]
    fn test_datetime_serialization() {
        let timestamp = Utc.with_ymd_and_hms(2024, 3, 15, 14, 25, 30).unwrap();
        let mut call = RadioCall::default();
        call.system_id = "test".to_string();
        call.call_timestamp = timestamp;
        call.created_at = timestamp;
        call.upload_timestamp = timestamp;

        let serialized = serde_json::to_string(&call).unwrap();
        let deserialized: RadioCall = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.call_timestamp, timestamp);
        assert_eq!(deserialized.created_at, timestamp);
        assert_eq!(deserialized.upload_timestamp, timestamp);
    }
}
