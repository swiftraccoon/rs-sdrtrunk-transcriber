//! Test fixtures and sample data

use chrono::{DateTime, Utc};
use sdrtrunk_core::types::{
    ApiResponse, AudioInfo, ErrorResponse, FileData, PaginationInfo, ProcessingResult, 
    RadioCall, SystemStats, TranscriptionStatus, UploadStatus
};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

/// Sample radio call data for testing
pub struct RadioCallFixtures;

impl RadioCallFixtures {
    /// Create a minimal valid radio call
    pub fn minimal() -> RadioCall {
        let mut call = RadioCall::default();
        call.system_id = "test_system".to_string();
        call
    }

    /// Create a complete radio call with all fields populated
    pub fn complete() -> RadioCall {
        let now = Utc::now();
        
        RadioCall {
            id: Some(Uuid::new_v4()),
            created_at: now,
            call_timestamp: now - chrono::Duration::minutes(5),
            system_id: "metro_police".to_string(),
            system_label: Some("Metro Police Department".to_string()),
            frequency: Some(854_000_000),
            talkgroup_id: Some(52197),
            talkgroup_label: Some("Police Dispatch".to_string()),
            talkgroup_group: Some("Operations".to_string()),
            talkgroup_tag: Some("Law Enforcement".to_string()),
            source_radio_id: Some(1234567),
            talker_alias: Some("Unit 23".to_string()),
            audio_filename: Some("20240315_142530_metro_police_TG52197_FROM_1234567.mp3".to_string()),
            audio_file_path: Some("/data/2024/03/15/20240315_142530_metro_police_TG52197_FROM_1234567.mp3".to_string()),
            audio_size_bytes: Some(1024000),
            duration_seconds: Some(30.5),
            upload_ip: Some("192.168.1.100".to_string()),
            upload_timestamp: now,
            upload_api_key_id: Some("test_api_key".to_string()),
            patches: Some(json!({
                "patch_groups": [
                    {"id": 1, "name": "North District"},
                    {"id": 2, "name": "South District"}
                ]
            })),
            frequencies: Some(json!([854000000, 855000000, 856000000])),
            sources: Some(json!({
                "site_id": 1,
                "site_name": "Main Tower",
                "antennas": ["North", "South", "East", "West"]
            })),
            transcription_status: TranscriptionStatus::Completed,
            transcription_text: Some("Unit 23 responding to traffic stop at Main and 5th Street".to_string()),
            transcription_confidence: Some(0.95),
            transcription_error: None,
            transcription_started_at: Some(now - chrono::Duration::minutes(3)),
            transcription_completed_at: Some(now - chrono::Duration::minutes(2)),
            speaker_count: Some(1),
            speaker_segments: Some(json!([
                {
                    "speaker": 0,
                    "start": 0.0,
                    "end": 30.5,
                    "confidence": 0.98
                }
            ])),
            transcription_segments: Some(json!([
                {
                    "start": 0.0,
                    "end": 30.5,
                    "text": "Unit 23 responding to traffic stop at Main and 5th Street",
                    "confidence": 0.95
                }
            ])),
        }
    }

    /// Create a radio call with transcription in progress
    pub fn processing() -> RadioCall {
        let mut call = Self::minimal();
        call.id = Some(Uuid::new_v4());
        call.system_label = Some("Test System".to_string());
        call.talkgroup_id = Some(12345);
        call.transcription_status = TranscriptionStatus::Processing;
        call.transcription_started_at = Some(Utc::now() - chrono::Duration::minutes(1));
        call.audio_filename = Some("test_processing.mp3".to_string());
        call
    }

    /// Create a radio call with failed transcription
    pub fn failed() -> RadioCall {
        let mut call = Self::minimal();
        call.id = Some(Uuid::new_v4());
        call.system_label = Some("Test System".to_string());
        call.talkgroup_id = Some(12345);
        call.transcription_status = TranscriptionStatus::Failed;
        call.transcription_error = Some("Audio quality too poor for transcription".to_string());
        call.transcription_started_at = Some(Utc::now() - chrono::Duration::minutes(2));
        call.transcription_completed_at = Some(Utc::now() - chrono::Duration::minutes(1));
        call.audio_filename = Some("test_failed.mp3".to_string());
        call
    }

    /// Create a radio call pending transcription
    pub fn pending() -> RadioCall {
        let mut call = Self::minimal();
        call.id = Some(Uuid::new_v4());
        call.system_label = Some("Test System".to_string());
        call.talkgroup_id = Some(12345);
        call.transcription_status = TranscriptionStatus::Pending;
        call.audio_filename = Some("test_pending.mp3".to_string());
        call
    }

    /// Create multiple radio calls for pagination testing
    pub fn batch(count: usize, system_id: &str) -> Vec<RadioCall> {
        (0..count)
            .map(|i| {
                let mut call = Self::minimal();
                call.id = Some(Uuid::new_v4());
                call.system_id = system_id.to_string();
                call.talkgroup_id = Some(10000 + i as i32);
                call.audio_filename = Some(format!("test_batch_{i}.mp3"));
                call
            })
            .collect()
    }
}

/// Sample file data for testing filename parsing
pub struct FileDataFixtures;

impl FileDataFixtures {
    /// Standard SDRTrunk filename format
    pub fn standard() -> FileData {
        FileData {
            date: "20240315".to_string(),
            time: "142530".to_string(),
            unixtime: 1710509130,
            talkgroup_id: 52197,
            talkgroup_name: "TG52197".to_string(),
            radio_id: 1234567,
            duration: "00:30".to_string(),
            filename: "20240315_142530_metro_police_TG52197_FROM_1234567.mp3".to_string(),
            filepath: "/data/2024/03/15/20240315_142530_metro_police_TG52197_FROM_1234567.mp3".to_string(),
        }
    }

    /// Alternative system name format
    pub fn alternative_system() -> FileData {
        FileData {
            date: "20240315".to_string(),
            time: "083045".to_string(),
            unixtime: 1710489045,
            talkgroup_id: 98765,
            talkgroup_name: "TG98765".to_string(),
            radio_id: 7654321,
            duration: "00:15".to_string(),
            filename: "20240315_083045_fire_dept_TG98765_FROM_7654321.mp3".to_string(),
            filepath: "/data/2024/03/15/20240315_083045_fire_dept_TG98765_FROM_7654321.mp3".to_string(),
        }
    }

    /// Test cases for filename parsing edge cases
    pub fn edge_cases() -> Vec<(&'static str, Result<FileData, &'static str>)> {
        vec![
            // Valid cases
            (
                "20240101_120000_system_TG123_FROM_456.mp3",
                Ok(FileData {
                    date: "20240101".to_string(),
                    time: "120000".to_string(),
                    unixtime: 1704110400,
                    talkgroup_id: 123,
                    talkgroup_name: "TG123".to_string(),
                    radio_id: 456,
                    duration: "".to_string(),
                    filename: "20240101_120000_system_TG123_FROM_456.mp3".to_string(),
                    filepath: "20240101_120000_system_TG123_FROM_456.mp3".to_string(),
                }),
            ),
            // Invalid cases
            ("invalid_filename.mp3", Err("Invalid format")),
            ("", Err("Empty filename")),
            ("no_extension", Err("No extension")),
            ("20240101.mp3", Err("Too few parts")),
        ]
    }
}

/// Sample API responses for testing
pub struct ApiResponseFixtures;

impl ApiResponseFixtures {
    /// Successful response with data
    pub fn success_with_data<T>(data: T) -> ApiResponse<T> {
        ApiResponse::success(data)
    }

    /// Successful response with message
    pub fn success_with_message<T>(data: T, message: &str) -> ApiResponse<T> {
        ApiResponse::success_with_message(data, message)
    }

    /// Paginated response
    pub fn paginated<T>(data: T, page: u32, per_page: u32, total: u64) -> ApiResponse<T> {
        let total_pages = (total as f64 / per_page as f64).ceil() as u32;
        let pagination = PaginationInfo {
            page,
            per_page,
            total_count: total,
            total_pages,
            has_next: page < total_pages,
            has_prev: page > 1,
        };
        ApiResponse::paginated(data, pagination)
    }

    /// Error response
    pub fn error(message: &str, code: &str) -> ErrorResponse {
        ErrorResponse::new(message, code)
    }

    /// Error response with details
    pub fn error_with_details(message: &str, code: &str, details: serde_json::Value) -> ErrorResponse {
        ErrorResponse::with_details(message, code, details)
    }
}

/// Sample audio info for testing
pub struct AudioInfoFixtures;

impl AudioInfoFixtures {
    /// Standard MP3 file info
    pub fn mp3_standard() -> AudioInfo {
        AudioInfo {
            format: "mp3".to_string(),
            sample_rate: 44100,
            channels: 2,
            bit_depth: Some(16),
            duration_seconds: 30.5,
            file_size: 1024000,
        }
    }

    /// WAV file info
    pub fn wav_standard() -> AudioInfo {
        AudioInfo {
            format: "wav".to_string(),
            sample_rate: 48000,
            channels: 1,
            bit_depth: Some(16),
            duration_seconds: 15.2,
            file_size: 1459200,
        }
    }

    /// FLAC file info
    pub fn flac_standard() -> AudioInfo {
        AudioInfo {
            format: "flac".to_string(),
            sample_rate: 44100,
            channels: 2,
            bit_depth: Some(24),
            duration_seconds: 45.8,
            file_size: 2048000,
        }
    }
}

/// Sample processing results
pub struct ProcessingResultFixtures;

impl ProcessingResultFixtures {
    /// Successful processing result
    pub fn success() -> ProcessingResult {
        ProcessingResult {
            success: true,
            error: None,
            audio_info: Some(AudioInfoFixtures::mp3_standard()),
            processing_time_ms: 1500,
            checksum: Some("abc123def456".to_string()),
        }
    }

    /// Failed processing result
    pub fn failure(error: &str) -> ProcessingResult {
        ProcessingResult {
            success: false,
            error: Some(error.to_string()),
            audio_info: None,
            processing_time_ms: 500,
            checksum: None,
        }
    }
}

/// Sample system statistics
pub struct SystemStatsFixtures;

impl SystemStatsFixtures {
    /// Active system with recent activity
    pub fn active_system() -> SystemStats {
        let now = Utc::now();
        SystemStats {
            system_id: "metro_police".to_string(),
            system_label: Some("Metro Police Department".to_string()),
            total_calls: 15420,
            calls_today: 347,
            calls_this_hour: 28,
            first_seen: Some(now - chrono::Duration::days(365)),
            last_seen: Some(now - chrono::Duration::minutes(2)),
            top_talkgroups: Some(json!([
                {"id": 52197, "label": "Dispatch", "count": 8234},
                {"id": 52198, "label": "Traffic", "count": 3456},
                {"id": 52199, "label": "Detectives", "count": 2106}
            ])),
            last_updated: now,
        }
    }

    /// Inactive system
    pub fn inactive_system() -> SystemStats {
        let now = Utc::now();
        SystemStats {
            system_id: "old_system".to_string(),
            system_label: Some("Deprecated System".to_string()),
            total_calls: 1234,
            calls_today: 0,
            calls_this_hour: 0,
            first_seen: Some(now - chrono::Duration::days(500)),
            last_seen: Some(now - chrono::Duration::days(30)),
            top_talkgroups: Some(json!([
                {"id": 1001, "label": "Main", "count": 800},
                {"id": 1002, "label": "Secondary", "count": 434}
            ])),
            last_updated: now - chrono::Duration::hours(6),
        }
    }
}

/// Sample upload statuses
pub struct UploadStatusFixtures;

impl UploadStatusFixtures {
    /// All possible upload statuses
    pub fn all_statuses() -> Vec<UploadStatus> {
        vec![
            UploadStatus::Pending,
            UploadStatus::Processing,
            UploadStatus::Completed,
            UploadStatus::Failed("Network timeout".to_string()),
        ]
    }
}

/// Sample transcription statuses
pub struct TranscriptionStatusFixtures;

impl TranscriptionStatusFixtures {
    /// All possible transcription statuses
    pub fn all_statuses() -> Vec<TranscriptionStatus> {
        vec![
            TranscriptionStatus::None,
            TranscriptionStatus::Pending,
            TranscriptionStatus::Processing,
            TranscriptionStatus::Completed,
            TranscriptionStatus::Failed,
        ]
    }

    /// Status progression for testing state transitions
    pub fn progression() -> Vec<TranscriptionStatus> {
        vec![
            TranscriptionStatus::None,
            TranscriptionStatus::Pending,
            TranscriptionStatus::Processing,
            TranscriptionStatus::Completed,
        ]
    }
}

/// Test datasets for performance and stress testing
pub struct TestDataSets;

impl TestDataSets {
    /// Small dataset for unit tests
    pub fn small() -> Vec<RadioCall> {
        vec![
            RadioCallFixtures::minimal(),
            RadioCallFixtures::complete(),
            RadioCallFixtures::processing(),
        ]
    }

    /// Medium dataset for integration tests
    pub fn medium() -> Vec<RadioCall> {
        let mut calls = Vec::new();
        
        // Add different system types
        let systems = ["police", "fire", "ems", "public_works"];
        for (i, system) in systems.iter().enumerate() {
            calls.extend(RadioCallFixtures::batch(25, system));
        }
        
        // Add some special cases
        calls.push(RadioCallFixtures::complete());
        calls.push(RadioCallFixtures::processing());
        calls.push(RadioCallFixtures::failed());
        calls.push(RadioCallFixtures::pending());
        
        calls
    }

    /// Large dataset for performance tests
    pub fn large() -> Vec<RadioCall> {
        let mut calls = Vec::new();
        
        // Generate larger batches
        let systems = [
            "metro_police", "county_sheriff", "state_patrol", "fire_dept",
            "ems_services", "public_works", "transit_authority", "airport_ops"
        ];
        
        for system in &systems {
            calls.extend(RadioCallFixtures::batch(1000, system));
        }
        
        calls
    }

    /// Dataset with edge cases for testing robustness
    pub fn edge_cases() -> Vec<RadioCall> {
        vec![
            // Empty optional fields
            RadioCallFixtures::minimal(),
            
            // Very long text fields
            {
                let mut call = RadioCallFixtures::minimal();
                call.system_id = "a".repeat(50); // Max length
                call.transcription_text = Some("Lorem ipsum ".repeat(100));
                call
            },
            
            // Extreme numeric values
            {
                let mut call = RadioCallFixtures::minimal();
                call.frequency = Some(i64::MAX);
                call.talkgroup_id = Some(i32::MAX);
                call.source_radio_id = Some(i32::MIN);
                call
            },
            
            // Special characters in text fields
            {
                let mut call = RadioCallFixtures::minimal();
                call.system_id = "test_emoji_🚨🚔".to_string();
                call.transcription_text = Some("Unicode test: café naïve résumé 日本語".to_string());
                call
            },
        ]
    }
}

/// Configuration fixtures for testing
pub struct ConfigFixtures;

impl ConfigFixtures {
    /// Development configuration
    pub fn development() -> sdrtrunk_core::Config {
        let mut config = sdrtrunk_core::Config::default();
        config.server.host = "127.0.0.1".to_string();
        config.server.port = 3000;
        config.database.url = "postgresql://test:test@localhost/sdrtrunk_test".to_string();
        config.api.enable_auth = false;
        config.security.require_api_key = false;
        config.logging.level = "debug".to_string();
        config
    }

    /// Production-like configuration
    pub fn production() -> sdrtrunk_core::Config {
        let mut config = sdrtrunk_core::Config::default();
        config.server.host = "0.0.0.0".to_string();
        config.server.port = 8080;
        config.database.url = "postgresql://prod:secret@db.example.com/sdrtrunk".to_string();
        config.api.enable_auth = true;
        config.security.require_api_key = true;
        config.security.enable_ip_restrictions = true;
        config.logging.level = "info".to_string();
        config
    }

    /// Testing configuration with temporary directories
    pub fn testing() -> sdrtrunk_core::Config {
        let mut config = sdrtrunk_core::Config::default();
        config.server.port = 0; // Random available port
        config.database.url = "postgresql://postgres:postgres@localhost/test".to_string();
        config.api.enable_auth = false;
        config.security.require_api_key = false;
        config.logging.level = "trace".to_string();
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radio_call_fixtures() {
        let minimal = RadioCallFixtures::minimal();
        assert_eq!(minimal.system_id, "test_system");
        assert!(minimal.id.is_none());

        let complete = RadioCallFixtures::complete();
        assert!(complete.id.is_some());
        assert_eq!(complete.system_id, "metro_police");
        assert_eq!(complete.transcription_status, TranscriptionStatus::Completed);

        let processing = RadioCallFixtures::processing();
        assert_eq!(processing.transcription_status, TranscriptionStatus::Processing);

        let failed = RadioCallFixtures::failed();
        assert_eq!(failed.transcription_status, TranscriptionStatus::Failed);
        assert!(failed.transcription_error.is_some());

        let batch = RadioCallFixtures::batch(5, "test_system");
        assert_eq!(batch.len(), 5);
        assert!(batch.iter().all(|c| c.system_id == "test_system"));
    }

    #[test]
    fn test_file_data_fixtures() {
        let standard = FileDataFixtures::standard();
        assert_eq!(standard.talkgroup_id, 52197);
        assert_eq!(standard.radio_id, 1234567);

        let alternative = FileDataFixtures::alternative_system();
        assert_eq!(alternative.talkgroup_id, 98765);
        assert_eq!(alternative.radio_id, 7654321);

        let edge_cases = FileDataFixtures::edge_cases();
        assert!(!edge_cases.is_empty());
        
        let valid_cases: Vec<_> = edge_cases.iter().filter(|(_, result)| result.is_ok()).collect();
        let invalid_cases: Vec<_> = edge_cases.iter().filter(|(_, result)| result.is_err()).collect();
        
        assert!(!valid_cases.is_empty());
        assert!(!invalid_cases.is_empty());
    }

    #[test]
    fn test_api_response_fixtures() {
        let success = ApiResponseFixtures::success_with_data("test");
        assert!(success.success);
        assert_eq!(success.data, "test");

        let with_message = ApiResponseFixtures::success_with_message("test", "Success!");
        assert!(with_message.success);
        assert_eq!(with_message.message, Some("Success!".to_string()));

        let paginated = ApiResponseFixtures::paginated(vec![1, 2, 3], 1, 10, 100);
        assert!(paginated.success);
        assert!(paginated.pagination.is_some());
        let pagination = paginated.pagination.unwrap();
        assert_eq!(pagination.page, 1);
        assert_eq!(pagination.total_count, 100);
        assert_eq!(pagination.total_pages, 10);

        let error = ApiResponseFixtures::error("Not found", "NOT_FOUND");
        assert_eq!(error.error, "Not found");
        assert_eq!(error.code, "NOT_FOUND");
        assert!(!error.success);
    }

    #[test]
    fn test_audio_info_fixtures() {
        let mp3 = AudioInfoFixtures::mp3_standard();
        assert_eq!(mp3.format, "mp3");
        assert_eq!(mp3.sample_rate, 44100);

        let wav = AudioInfoFixtures::wav_standard();
        assert_eq!(wav.format, "wav");
        assert_eq!(wav.channels, 1);

        let flac = AudioInfoFixtures::flac_standard();
        assert_eq!(flac.format, "flac");
        assert_eq!(flac.bit_depth, Some(24));
    }

    #[test]
    fn test_processing_result_fixtures() {
        let success = ProcessingResultFixtures::success();
        assert!(success.success);
        assert!(success.error.is_none());
        assert!(success.audio_info.is_some());

        let failure = ProcessingResultFixtures::failure("Test error");
        assert!(!failure.success);
        assert_eq!(failure.error, Some("Test error".to_string()));
        assert!(failure.audio_info.is_none());
    }

    #[test]
    fn test_system_stats_fixtures() {
        let active = SystemStatsFixtures::active_system();
        assert_eq!(active.system_id, "metro_police");
        assert!(active.calls_today > 0);
        assert!(active.top_talkgroups.is_some());

        let inactive = SystemStatsFixtures::inactive_system();
        assert_eq!(inactive.system_id, "old_system");
        assert_eq!(inactive.calls_today, 0);
        assert_eq!(inactive.calls_this_hour, 0);
    }

    #[test]
    fn test_upload_status_fixtures() {
        let statuses = UploadStatusFixtures::all_statuses();
        assert_eq!(statuses.len(), 4);
        
        match &statuses[3] {
            UploadStatus::Failed(msg) => assert_eq!(msg, "Network timeout"),
            _ => panic!("Expected Failed status"),
        }
    }

    #[test]
    fn test_transcription_status_fixtures() {
        let all = TranscriptionStatusFixtures::all_statuses();
        assert_eq!(all.len(), 5);

        let progression = TranscriptionStatusFixtures::progression();
        assert_eq!(progression.len(), 4);
        assert_eq!(progression[0], TranscriptionStatus::None);
        assert_eq!(progression[3], TranscriptionStatus::Completed);
    }

    #[test]
    fn test_datasets() {
        let small = TestDataSets::small();
        assert_eq!(small.len(), 3);

        let medium = TestDataSets::medium();
        assert!(medium.len() > 100);

        let large = TestDataSets::large();
        assert!(large.len() > 1000);

        let edge_cases = TestDataSets::edge_cases();
        assert!(edge_cases.len() >= 4);
    }

    #[test]
    fn test_config_fixtures() {
        let dev = ConfigFixtures::development();
        assert_eq!(dev.server.host, "127.0.0.1");
        assert!(!dev.api.enable_auth);

        let prod = ConfigFixtures::production();
        assert_eq!(prod.server.host, "0.0.0.0");
        assert!(prod.api.enable_auth);

        let test = ConfigFixtures::testing();
        assert_eq!(test.server.port, 0);
        assert_eq!(test.logging.level, "trace");
    }
}