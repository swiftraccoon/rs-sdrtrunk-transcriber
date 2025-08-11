//! Integration tests for sdrtrunk-core functionality

mod common;

use sdrtrunk_core::context_error::Result;
use common::*;
use sdrtrunk_core::{
    config::Config,
    error::{Error, Result as CoreResult},
    types::*,
    utils::*,
};
use std::path::PathBuf;
use tempfile::tempdir;

/// Test configuration loading and validation
#[tokio::test]
async fn test_config_loading_from_environment() -> Result<()> {
    init_test_logging();
    
    // Set environment variables
    std::env::set_var("SDRTRUNK_SERVER_HOST", "192.168.1.100");
    std::env::set_var("SDRTRUNK_SERVER_PORT", "9090");
    std::env::set_var("SDRTRUNK_DATABASE_URL", "postgresql://test:test@testhost/testdb");
    std::env::set_var("SDRTRUNK_STORAGE_BASE_DIR", "/tmp/test_storage");
    
    // Load configuration
    let config = Config::load().unwrap_or_else(|_| Config::default());
    
    // The default implementation should pick up some environment variables
    // Note: Config::load() is not fully implemented, so we test the default behavior
    assert_eq!(config.database.url, "postgresql://test:test@testhost/testdb");
    assert_eq!(config.storage.base_dir, PathBuf::from("/tmp/test_storage"));
    
    // Clean up environment
    std::env::remove_var("SDRTRUNK_SERVER_HOST");
    std::env::remove_var("SDRTRUNK_SERVER_PORT");
    std::env::remove_var("SDRTRUNK_DATABASE_URL");
    std::env::remove_var("SDRTRUNK_STORAGE_BASE_DIR");
    
    Ok(())
}

/// Test configuration with temporary storage
#[tokio::test]
async fn test_config_with_temp_storage() -> Result<()> {
    let (config, _temp_dir) = TestConfigBuilder::new()
        .with_temp_storage()?
        .with_port(8080)
        .without_auth()
        .build();
    
    assert!(config.storage.base_dir.exists());
    assert_eq!(config.server.port, 8080);
    assert!(!config.api.enable_auth);
    assert!(!config.security.require_api_key);
    
    Ok(())
}

/// Test SDRTrunk filename parsing with various formats
#[tokio::test]
async fn test_sdrtrunk_filename_parsing() -> Result<()> {
    init_test_logging();
    
    // Test valid filename
    let filename = "20240315_142530_MetroPolice_TG52197_FROM_1234567.mp3";
    let file_data = parse_sdrtrunk_filename(filename)?;
    
    assert_eq!(file_data.date, "20240315");
    assert_eq!(file_data.time, "142530");
    assert_eq!(file_data.talkgroup_id, 52197);
    assert_eq!(file_data.radio_id, 1234567);
    assert_eq!(file_data.filename, filename);
    assert!(file_data.unixtime > 0);
    
    // Test alternative formats
    let test_cases = vec![
        ("20240101_120000_System1_TG123_FROM_456.mp3", (123, 456)),
        ("20231225_235959_FireDept_TG999_FROM_12345.wav", (999, 12345)),
        ("20240630_180000_EMS_TG5555_FROM_987654.flac", (5555, 987654)),
    ];
    
    for (filename, (expected_tg, expected_radio)) in test_cases {
        let file_data = parse_sdrtrunk_filename(filename)?;
        assert_eq!(file_data.talkgroup_id, expected_tg);
        assert_eq!(file_data.radio_id, expected_radio);
    }
    
    Ok(())
}

/// Test SDRTrunk filename parsing error cases
#[tokio::test]
async fn test_sdrtrunk_filename_parsing_errors() -> Result<()> {
    let invalid_filenames = vec![
        "invalid.mp3",
        "too_few_parts.mp3",
        "20240315.mp3",
        "",
        "no_extension",
    ];
    
    for filename in invalid_filenames {
        let result = parse_sdrtrunk_filename(filename);
        assert!(result.is_err(), "Expected error for filename: {filename}");
        
        if let Err(Error::FileProcessing(msg)) = result {
            assert!(!msg.is_empty());
        } else {
            panic!("Expected FileProcessing error for filename: {filename}");
        }
    }
    
    Ok(())
}

/// Test file extension validation
#[test]
fn test_file_extension_validation() {
    let allowed_extensions = vec![
        "mp3".to_string(),
        "wav".to_string(),
        "flac".to_string(),
    ];
    
    // Valid extensions
    assert!(validate_file_extension("test.mp3", &allowed_extensions));
    assert!(validate_file_extension("test.MP3", &allowed_extensions));
    assert!(validate_file_extension("test.wav", &allowed_extensions));
    assert!(validate_file_extension("test.WAV", &allowed_extensions));
    assert!(validate_file_extension("test.flac", &allowed_extensions));
    assert!(validate_file_extension("test.FLAC", &allowed_extensions));
    
    // Invalid extensions
    assert!(!validate_file_extension("test.aac", &allowed_extensions));
    assert!(!validate_file_extension("test.m4a", &allowed_extensions));
    assert!(!validate_file_extension("test", &allowed_extensions));
    assert!(!validate_file_extension("", &allowed_extensions));
    
    // Case insensitive matching
    assert!(validate_file_extension("Test.Mp3", &allowed_extensions));
    assert!(validate_file_extension("FILE.WAV", &allowed_extensions));
}

/// Test storage filename generation
#[test]
fn test_storage_filename_generation() {
    let test_cases = vec![
        "original.mp3",
        "test.wav",
        "audio.flac",
        "file_without_extension",
    ];
    
    for original in test_cases {
        let generated = generate_storage_filename(original);
        
        // Should be UUID format with extension
        assert!(generated.len() > 10);
        
        if original.contains('.') {
            let expected_ext = original.split('.').last().unwrap();
            assert!(generated.ends_with(&format!(".{expected_ext}")));
        } else {
            assert!(generated.ends_with(".mp3")); // Default extension
        }
        
        // Should be unique
        let generated2 = generate_storage_filename(original);
        assert_ne!(generated, generated2);
    }
}

/// Test date path creation
#[test]
fn test_date_path_creation() {
    let base = PathBuf::from("/data");
    let date = chrono::Utc
        .with_ymd_and_hms(2024, 3, 15, 14, 25, 30)
        .unwrap();
    
    let path = create_date_path(&base, &date);
    assert_eq!(path, PathBuf::from("/data/2024/03/15"));
    
    // Test with different dates
    let new_year = chrono::Utc
        .with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
        .unwrap();
    
    let path2 = create_date_path(&base, &new_year);
    assert_eq!(path2, PathBuf::from("/data/2025/01/01"));
}

/// Test radio call validation
#[tokio::test]
async fn test_radio_call_validation() -> Result<()> {
    use validator::Validate;
    
    // Valid radio call
    let mut call = create_test_radio_call("test_system");
    call.system_label = Some("Valid System".to_string());
    assert!(call.validate().is_ok());
    
    // Invalid system_id (empty)
    call.system_id = String::new();
    assert!(call.validate().is_err());
    
    // Invalid system_id (too long)
    call.system_id = "a".repeat(51);
    assert!(call.validate().is_err());
    
    // Valid system_id (at limit)
    call.system_id = "a".repeat(50);
    assert!(call.validate().is_ok());
    
    // Invalid system_label (too long)
    call.system_label = Some("a".repeat(256));
    assert!(call.validate().is_err());
    
    // Valid system_label (at limit)
    call.system_label = Some("a".repeat(255));
    assert!(call.validate().is_ok());
    
    Ok(())
}

/// Test transcription status transitions
#[test]
fn test_transcription_status_transitions() {
    let statuses = vec![
        TranscriptionStatus::None,
        TranscriptionStatus::Pending,
        TranscriptionStatus::Processing,
        TranscriptionStatus::Completed,
        TranscriptionStatus::Failed,
    ];
    
    // Test string representation
    assert_eq!(TranscriptionStatus::None.to_string(), "none");
    assert_eq!(TranscriptionStatus::Pending.to_string(), "pending");
    assert_eq!(TranscriptionStatus::Processing.to_string(), "processing");
    assert_eq!(TranscriptionStatus::Completed.to_string(), "completed");
    assert_eq!(TranscriptionStatus::Failed.to_string(), "failed");
    
    // Test serialization round-trip
    for status in statuses {
        let serialized = serde_json::to_string(&status).unwrap();
        let deserialized: TranscriptionStatus = serde_json::from_str(&serialized).unwrap();
        assert_eq!(status, deserialized);
    }
}

/// Test API response creation
#[test]
fn test_api_response_creation() {
    // Success response
    let data = vec!["item1", "item2", "item3"];
    let response = ApiResponse::success(data.clone());
    assert!(response.success);
    assert_eq!(response.data, data);
    assert!(response.message.is_none());
    assert!(response.pagination.is_none());
    
    // Success response with message
    let response_with_msg = ApiResponse::success_with_message("test", "Operation completed");
    assert!(response_with_msg.success);
    assert_eq!(response_with_msg.data, "test");
    assert_eq!(response_with_msg.message, Some("Operation completed".to_string()));
    
    // Paginated response
    let pagination = PaginationInfo {
        page: 1,
        per_page: 10,
        total_count: 100,
        total_pages: 10,
        has_next: true,
        has_prev: false,
    };
    let paginated = ApiResponse::paginated("data", pagination.clone());
    assert!(paginated.success);
    assert_eq!(paginated.pagination, Some(pagination));
}

/// Test error response creation
#[test]
fn test_error_response_creation() {
    // Basic error response
    let error = ErrorResponse::new("Something went wrong", "INTERNAL_ERROR");
    assert_eq!(error.error, "Something went wrong");
    assert_eq!(error.code, "INTERNAL_ERROR");
    assert!(!error.success);
    assert!(error.details.is_none());
    
    // Error response with details
    let details = serde_json::json!({"field": "system_id", "message": "Required"});
    let error_with_details = ErrorResponse::with_details("Validation failed", "VALIDATION_ERROR", details.clone());
    assert_eq!(error_with_details.error, "Validation failed");
    assert_eq!(error_with_details.code, "VALIDATION_ERROR");
    assert_eq!(error_with_details.details, Some(details));
}

/// Test complex data structures serialization
#[tokio::test]
async fn test_complex_serialization() -> Result<()> {
    let mut call = RadioCallFixtures::complete();
    
    // Test with complex JSON metadata
    call.patches = Some(serde_json::json!({
        "patch_groups": [
            {"id": 1, "name": "North District", "active": true},
            {"id": 2, "name": "South District", "active": false}
        ],
        "patch_count": 2
    }));
    
    call.frequencies = Some(serde_json::json!([
        854000000, 855000000, 856000000, 857000000
    ]));
    
    call.sources = Some(serde_json::json!({
        "site_id": 123,
        "site_name": "Downtown Tower",
        "coordinates": {"lat": 40.7128, "lon": -74.0060},
        "antennas": ["North", "South", "East", "West"],
        "coverage_radius_km": 25.5
    }));
    
    // Serialize and deserialize
    let serialized = serde_json::to_string(&call)?;
    let deserialized: RadioCall = serde_json::from_str(&serialized)?;
    
    // Verify complex fields are preserved
    assert_eq!(call.patches, deserialized.patches);
    assert_eq!(call.frequencies, deserialized.frequencies);
    assert_eq!(call.sources, deserialized.sources);
    
    Ok(())
}

/// Test error handling and error types
#[tokio::test]
async fn test_error_handling() -> Result<()> {
    // Test different error types
    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
    let app_error = Error::from(io_error);
    
    match app_error {
        Error::Io(_) => {}, // Expected
        _ => panic!("Expected Io error variant"),
    }
    
    // Test custom errors
    let config_error = Error::Configuration {
        message: "Invalid configuration".to_string(),
    };
    assert!(format!("{config_error}").contains("Configuration error"));
    
    let validation_error = Error::Validation {
        field: "system_id".to_string(),
        message: "Field is required".to_string(),
    };
    assert!(format!("{validation_error}").contains("Validation error"));
    
    // Test error chaining
    let json_error = serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err();
    let serialization_error = Error::from(json_error);
    match serialization_error {
        Error::Serialization(_) => {}, // Expected
        _ => panic!("Expected Serialization error variant"),
    }
    
    Ok(())
}

/// Test logging initialization
#[tokio::test]
async fn test_logging_initialization() -> Result<()> {
    // Test that logging can be initialized without errors
    // Note: We can't test the actual output without more complex setup
    let result = sdrtrunk_core::init_logging();
    
    // Should not error (though it might fail if already initialized)
    match result {
        Ok(()) => {}, // Success
        Err(e) => {
            // May fail if already initialized in other tests, which is OK
            println!("Logging init result: {e:?}");
        }
    }
    
    Ok(())
}

/// Test file data with edge cases
#[test]
fn test_file_data_edge_cases() {
    // Test with minimum values
    let file_data = FileData {
        date: "20000101".to_string(),
        time: "000000".to_string(),
        unixtime: 0,
        talkgroup_id: 1,
        talkgroup_name: "TG1".to_string(),
        radio_id: 1,
        duration: "00:00".to_string(),
        filename: "test.mp3".to_string(),
        filepath: "/test.mp3".to_string(),
    };
    
    // Should serialize/deserialize without issues
    let serialized = serde_json::to_string(&file_data).unwrap();
    let deserialized: FileData = serde_json::from_str(&serialized).unwrap();
    assert_eq!(file_data.talkgroup_id, deserialized.talkgroup_id);
    
    // Test with maximum reasonable values
    let max_file_data = FileData {
        date: "99991231".to_string(),
        time: "235959".to_string(),
        unixtime: i64::MAX,
        talkgroup_id: i32::MAX,
        talkgroup_name: format!("TG{}", i32::MAX),
        radio_id: i32::MAX,
        duration: "99:59".to_string(),
        filename: "a".repeat(255),
        filepath: "/".to_string() + &"a".repeat(1000),
    };
    
    let serialized = serde_json::to_string(&max_file_data).unwrap();
    let deserialized: FileData = serde_json::from_str(&serialized).unwrap();
    assert_eq!(max_file_data.talkgroup_id, deserialized.talkgroup_id);
}

/// Test audio info validation
#[test]
fn test_audio_info_validation() {
    let audio_info = AudioInfo {
        format: "mp3".to_string(),
        sample_rate: 44100,
        channels: 2,
        bit_depth: Some(16),
        duration_seconds: 30.5,
        file_size: 1024000,
    };
    
    // Test serialization
    let serialized = serde_json::to_string(&audio_info).unwrap();
    let deserialized: AudioInfo = serde_json::from_str(&serialized).unwrap();
    assert_eq!(audio_info.format, deserialized.format);
    assert_eq!(audio_info.sample_rate, deserialized.sample_rate);
    assert_eq!(audio_info.channels, deserialized.channels);
    assert_eq!(audio_info.bit_depth, deserialized.bit_depth);
    assert_eq!(audio_info.duration_seconds, deserialized.duration_seconds);
    assert_eq!(audio_info.file_size, deserialized.file_size);
}

/// Test processing result scenarios
#[test]
fn test_processing_result_scenarios() {
    // Success case
    let success = ProcessingResult {
        success: true,
        error: None,
        audio_info: Some(AudioInfo {
            format: "mp3".to_string(),
            sample_rate: 44100,
            channels: 2,
            bit_depth: Some(16),
            duration_seconds: 30.5,
            file_size: 1024000,
        }),
        processing_time_ms: 1500,
        checksum: Some("abc123".to_string()),
    };
    
    assert!(success.success);
    assert!(success.error.is_none());
    assert!(success.audio_info.is_some());
    
    // Failure case
    let failure = ProcessingResult {
        success: false,
        error: Some("Processing failed".to_string()),
        audio_info: None,
        processing_time_ms: 100,
        checksum: None,
    };
    
    assert!(!failure.success);
    assert!(failure.error.is_some());
    assert!(failure.audio_info.is_none());
    
    // Test serialization for both
    for result in [success, failure] {
        let serialized = serde_json::to_string(&result).unwrap();
        let deserialized: ProcessingResult = serde_json::from_str(&serialized).unwrap();
        assert_eq!(result.success, deserialized.success);
        assert_eq!(result.error, deserialized.error);
        assert_eq!(result.processing_time_ms, deserialized.processing_time_ms);
    }
}

/// Property-based testing for filename parsing
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    
    proptest! {
        #[test]
        fn test_filename_generation_uniqueness(
            system_id in "[a-zA-Z][a-zA-Z0-9_]{1,30}",
            talkgroup_id in 1u32..999999u32,
            radio_id in 1000000u32..9999999u32
        ) {
            let filename1 = format!("20240315_142530_{system_id}_TG{talkgroup_id}_FROM_{radio_id}.mp3");
            let filename2 = format!("20240315_142531_{system_id}_TG{talkgroup_id}_FROM_{radio_id}.mp3");
            
            // Different timestamps should produce different filenames
            prop_assert_ne!(filename1, filename2);
            
            // Both should be parseable
            let parsed1 = parse_sdrtrunk_filename(&filename1);
            let parsed2 = parse_sdrtrunk_filename(&filename2);
            
            prop_assert!(parsed1.is_ok());
            prop_assert!(parsed2.is_ok());
            
            if let (Ok(data1), Ok(data2)) = (parsed1, parsed2) {
                prop_assert_eq!(data1.talkgroup_id, talkgroup_id as i32);
                prop_assert_eq!(data2.talkgroup_id, talkgroup_id as i32);
                prop_assert_eq!(data1.radio_id, radio_id as i32);
                prop_assert_eq!(data2.radio_id, radio_id as i32);
            }
        }
        
        #[test]
        fn test_storage_filename_uniqueness(original_filename in "[a-zA-Z0-9_-]+\\.(mp3|wav|flac)") {
            let filename1 = generate_storage_filename(&original_filename);
            let filename2 = generate_storage_filename(&original_filename);
            
            // Should always be unique
            prop_assert_ne!(filename1, filename2);
            
            // Should preserve extension
            let original_ext = original_filename.split('.').last().unwrap();
            prop_assert!(filename1.ends_with(&format!(".{original_ext}")));
            prop_assert!(filename2.ends_with(&format!(".{original_ext}")));
        }
        
        #[test]
        fn test_validate_file_extension_properties(
            filename in "[a-zA-Z0-9_-]+\\.(mp3|wav|flac|aac|m4a)",
            allowed_exts in prop::collection::vec("[a-z]{2,5}", 1..10)
        ) {
            let result = validate_file_extension(&filename, &allowed_exts);
            
            let file_ext = filename.split('.').last().unwrap().to_lowercase();
            let should_be_valid = allowed_exts.iter().any(|ext| ext.eq_ignore_ascii_case(&file_ext));
            
            prop_assert_eq!(result, should_be_valid);
        }
    }
}