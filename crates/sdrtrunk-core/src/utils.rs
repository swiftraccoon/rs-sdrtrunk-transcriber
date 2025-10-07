//! Utility functions for `SDRTrunk` transcriber

use crate::Result;
use std::path::Path;

/// Extract metadata from `SDRTrunk` filename
///
/// # Errors
///
/// Returns an error if the filename doesn't match the expected format.
/// Format: `YYYYMMDD_HHMMSS_<System>_<Talkgroup>_FROM_<RadioID>.mp3`
pub fn parse_sdrtrunk_filename(filename: &str) -> Result<crate::types::FileData> {
    let base = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| crate::Error::FileProcessing("Invalid filename".to_string()))?;

    // Parse the filename components
    let parts: Vec<&str> = base.split('_').collect();
    if parts.len() < 5 {
        return Err(crate::Error::FileProcessing(
            "Filename does not match SDRTrunk format".to_string(),
        ));
    }

    let date = parts[0].to_string();
    let time = parts[1].to_string();

    // Extract talkgroup and radio ID
    let talkgroup_part = parts.iter().find(|&&p| p.starts_with("TG")).unwrap_or(&"");
    let talkgroup_id = talkgroup_part
        .trim_start_matches("TG")
        .parse::<i32>()
        .unwrap_or(0);

    let radio_id = parts
        .iter()
        .position(|&p| p == "FROM")
        .and_then(|i| parts.get(i + 1))
        .and_then(|&id| id.parse::<i32>().ok())
        .unwrap_or(0);

    // Calculate Unix timestamp
    let unixtime =
        chrono::NaiveDateTime::parse_from_str(&format!("{date} {time}"), "%Y%m%d %H%M%S")
            .map(|dt| dt.and_utc().timestamp())
            .unwrap_or(0);

    Ok(crate::types::FileData {
        date,
        time,
        unixtime,
        talkgroup_id,
        talkgroup_name: (*talkgroup_part).to_string(),
        radio_id,
        duration: String::new(), // Will be extracted from audio file
        filename: filename.to_string(),
        filepath: filename.to_string(),
    })
}

/// Validate file extension
#[must_use]
pub fn validate_file_extension(filename: &str, allowed: &[String]) -> bool {
    Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            allowed
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(ext))
        })
}

/// Generate a unique filename for storage
#[must_use]
pub fn generate_storage_filename(original: &str) -> String {
    let uuid = uuid::Uuid::new_v4();
    let extension = Path::new(original)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("mp3");

    format!("{uuid}.{extension}")
}

/// Create date-based directory path
#[must_use]
pub fn create_date_path(base: &Path, date: &chrono::DateTime<chrono::Utc>) -> std::path::PathBuf {
    base.join(format!("{}", date.format("%Y/%m/%d")))
}

/// Validate system ID format
#[must_use]
pub fn validate_system_id(system_id: &str) -> bool {
    !system_id.is_empty()
        && system_id.len() <= 50
        && system_id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Sanitize filename for safe storage
#[must_use]
pub fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|c| {
            match c {
                // Keep alphanumeric, dots, underscores, and hyphens
                c if c.is_alphanumeric() || c == '.' || c == '_' || c == '-' => c,
                // Replace everything else with underscore
                _ => '_',
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

/// Calculate file checksum (MD5 for simplicity)
#[must_use]
pub fn calculate_file_checksum(data: &[u8]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Format duration in seconds to human readable format
#[must_use]
pub fn format_duration(seconds: f64) -> String {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let total_seconds = seconds.round() as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let secs = total_seconds % 60;

    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes:02}:{secs:02}")
    }
}

/// Parse duration string back to seconds
///
/// # Errors
///
/// Returns an error if the duration string is not in MM:SS or HH:MM:SS format.
pub fn parse_duration(duration_str: &str) -> Result<f64> {
    let parts: Vec<&str> = duration_str.split(':').collect();

    match parts.len() {
        2 => {
            // MM:SS format
            let minutes: f64 = parts[0]
                .parse()
                .map_err(|_| crate::Error::Other("Invalid duration format".to_string()))?;
            let seconds: f64 = parts[1]
                .parse()
                .map_err(|_| crate::Error::Other("Invalid duration format".to_string()))?;
            Ok(minutes.mul_add(60.0, seconds))
        }
        3 => {
            // HH:MM:SS format
            let hours: f64 = parts[0]
                .parse()
                .map_err(|_| crate::Error::Other("Invalid duration format".to_string()))?;
            let minutes: f64 = parts[1]
                .parse()
                .map_err(|_| crate::Error::Other("Invalid duration format".to_string()))?;
            let seconds: f64 = parts[2]
                .parse()
                .map_err(|_| crate::Error::Other("Invalid duration format".to_string()))?;
            Ok(hours.mul_add(3600.0, minutes.mul_add(60.0, seconds)))
        }
        _ => Err(crate::Error::Other("Invalid duration format".to_string())),
    }
}

/// Convert frequency to human readable format
#[must_use]
pub fn format_frequency(frequency_hz: i64) -> String {
    #[allow(clippy::cast_precision_loss)]
    if frequency_hz >= 1_000_000_000 {
        format!("{:.3} GHz", frequency_hz as f64 / 1_000_000_000.0)
    } else if frequency_hz >= 1_000_000 {
        format!("{:.3} MHz", frequency_hz as f64 / 1_000_000.0)
    } else if frequency_hz >= 1_000 {
        format!("{:.3} kHz", frequency_hz as f64 / 1_000.0)
    } else {
        format!("{frequency_hz} Hz")
    }
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
#[allow(
    clippy::missing_panics_doc,
    clippy::unreadable_literal,
    clippy::float_cmp,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::uninlined_format_args
)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use proptest::prelude::*;

    #[test]
    fn test_parse_sdrtrunk_filename() {
        let filename = "20240315_142530_System1_TG52197_FROM_1234567.mp3";
        let result = parse_sdrtrunk_filename(filename).unwrap();

        assert_eq!(result.date, "20240315");
        assert_eq!(result.time, "142530");
        assert_eq!(result.talkgroup_id, 52197);
        assert_eq!(result.radio_id, 1234567);
        assert_eq!(result.talkgroup_name, "TG52197");
    }

    #[test]
    fn test_parse_sdrtrunk_filename_variations() {
        let test_cases = vec![
            ("20240101_120000_Metro_TG123_FROM_456.mp3", 123, 456),
            ("20231225_235959_FireDept_TG999_FROM_12345.wav", 999, 12345),
            (
                "20240630_180000_EMS_Services_TG5555_FROM_987654.flac",
                5555,
                987654,
            ),
        ];

        for (filename, expected_tg, expected_radio) in test_cases {
            let result = parse_sdrtrunk_filename(filename).unwrap();
            assert_eq!(result.talkgroup_id, expected_tg);
            assert_eq!(result.radio_id, expected_radio);
            assert!(result.unixtime > 0);
        }
    }

    #[test]
    fn test_parse_sdrtrunk_filename_errors() {
        let invalid_cases = vec![
            "invalid.mp3",
            "too_few_parts.mp3",
            "20240315.mp3",
            "",
            "no_extension",
            "20240315_142530.mp3", // Missing required parts
        ];

        for filename in invalid_cases {
            let result = parse_sdrtrunk_filename(filename);
            assert!(result.is_err(), "Expected error for filename: {filename}");
        }
    }

    #[test]
    fn test_validate_file_extension() {
        let allowed_extensions = vec!["mp3".to_string(), "wav".to_string(), "flac".to_string()];

        // Valid extensions
        assert!(validate_file_extension("test.mp3", &allowed_extensions));
        assert!(validate_file_extension("test.MP3", &allowed_extensions));
        assert!(validate_file_extension("test.wav", &allowed_extensions));
        assert!(validate_file_extension("test.WAV", &allowed_extensions));
        assert!(validate_file_extension("test.flac", &allowed_extensions));
        assert!(validate_file_extension("test.FLAC", &allowed_extensions));
        assert!(validate_file_extension(
            "path/to/test.mp3",
            &allowed_extensions
        ));

        // Invalid extensions
        assert!(!validate_file_extension("test.aac", &allowed_extensions));
        assert!(!validate_file_extension("test.m4a", &allowed_extensions));
        assert!(!validate_file_extension("test", &allowed_extensions));
        assert!(!validate_file_extension("", &allowed_extensions));
        assert!(!validate_file_extension("test.txt", &allowed_extensions));
    }

    #[test]
    fn test_generate_storage_filename() {
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
                let expected_ext = original.split('.').next_back().unwrap();
                assert!(generated.ends_with(&format!(".{expected_ext}")));
            } else {
                assert!(generated.ends_with(".mp3")); // Default extension
            }

            // Should be unique
            let generated2 = generate_storage_filename(original);
            assert_ne!(generated, generated2);
        }
    }

    #[test]
    fn test_create_date_path() {
        let base = std::path::PathBuf::from("/data");
        let date = chrono::Utc
            .with_ymd_and_hms(2024, 3, 15, 14, 25, 30)
            .unwrap();

        let path = create_date_path(&base, &date);
        assert_eq!(path, std::path::PathBuf::from("/data/2024/03/15"));

        // Test with different dates
        let new_year = chrono::Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();

        let path2 = create_date_path(&base, &new_year);
        assert_eq!(path2, std::path::PathBuf::from("/data/2025/01/01"));
    }

    #[test]
    fn test_validate_system_id() {
        // Valid system IDs
        assert!(validate_system_id("police"));
        assert!(validate_system_id("fire_dept"));
        assert!(validate_system_id("metro-police"));
        assert!(validate_system_id("system123"));
        assert!(validate_system_id("a".repeat(50).as_str())); // At limit

        // Invalid system IDs
        assert!(!validate_system_id("")); // Empty
        assert!(!validate_system_id("a".repeat(51).as_str())); // Too long
        assert!(!validate_system_id("system with spaces"));
        assert!(!validate_system_id("system@special"));
        assert!(!validate_system_id("system/path"));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(
            sanitize_filename("valid_filename.mp3"),
            "valid_filename.mp3"
        );
        assert_eq!(
            sanitize_filename("file with spaces.mp3"),
            "file_with_spaces.mp3"
        );
        assert_eq!(sanitize_filename("file@#$%^&*().mp3"), "file_________.mp3");
        assert_eq!(
            sanitize_filename("file/path\\name.mp3"),
            "file_path_name.mp3"
        );
        assert_eq!(sanitize_filename("___file___.mp3"), "file___.mp3");
        assert_eq!(sanitize_filename("file-name_123.mp3"), "file-name_123.mp3");
    }

    #[test]
    fn test_calculate_file_checksum() {
        let data1 = b"test data";
        let data2 = b"different data";
        let data3 = b"test data"; // Same as data1

        let checksum1 = calculate_file_checksum(data1);
        let checksum2 = calculate_file_checksum(data2);
        let checksum3 = calculate_file_checksum(data3);

        assert_ne!(checksum1, checksum2);
        assert_eq!(checksum1, checksum3);
        assert!(checksum1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30.0), "00:30");
        assert_eq!(format_duration(90.0), "01:30");
        assert_eq!(format_duration(3661.0), "01:01:01");
        assert_eq!(format_duration(0.0), "00:00");
        assert_eq!(format_duration(59.9), "01:00"); // Rounds up
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("00:30").unwrap(), 30.0);
        assert_eq!(parse_duration("01:30").unwrap(), 90.0);
        assert_eq!(parse_duration("01:01:01").unwrap(), 3661.0);
        assert_eq!(parse_duration("00:00").unwrap(), 0.0);

        // Error cases
        assert!(parse_duration("invalid").is_err());
        assert!(parse_duration("1:2:3:4").is_err());
        assert!(parse_duration("").is_err());
        assert!(parse_duration("01:xx").is_err());
    }

    #[test]
    fn test_format_frequency() {
        assert_eq!(format_frequency(154000000), "154.000 MHz");
        assert_eq!(format_frequency(854000000), "854.000 MHz");
        assert_eq!(format_frequency(1500000), "1.500 MHz");
        assert_eq!(format_frequency(1000), "1.000 kHz");
        assert_eq!(format_frequency(500), "500 Hz");
        assert_eq!(format_frequency(2400000000), "2.400 GHz");
    }

    #[test]
    fn test_duration_roundtrip() {
        let test_durations = vec![0.0, 30.0, 90.0, 3661.0, 3599.5];

        for duration in test_durations {
            let formatted = format_duration(duration);
            let parsed = parse_duration(&formatted).unwrap();
            // Allow small rounding errors
            assert!((duration - parsed).abs() < 1.0);
        }
    }

    // Property-based tests
    proptest! {
        #[test]
        fn test_storage_filename_uniqueness(original in "[a-zA-Z0-9_.-]+") {
            let filename1 = generate_storage_filename(&original);
            let filename2 = generate_storage_filename(&original);
            prop_assert_ne!(filename1, filename2);
        }

        #[test]
        fn test_sanitize_filename_properties(input in ".*") {
            let sanitized = sanitize_filename(&input);
            // Should not contain problematic characters
            prop_assert!(!sanitized.contains(' '));
            prop_assert!(!sanitized.contains('/'));
            prop_assert!(!sanitized.contains('\\'));
            prop_assert!(!sanitized.contains('@'));
        }

        #[test]
        fn test_system_id_validation_properties(
            system_id in "[a-zA-Z][a-zA-Z0-9_-]{0,49}"
        ) {
            prop_assert!(validate_system_id(&system_id));
        }

        #[test]
        fn test_frequency_formatting_properties(frequency in 1i64..10_000_000_000i64) {
            let formatted = format_frequency(frequency);
            prop_assert!(!formatted.is_empty());
            prop_assert!(formatted.contains("Hz") || formatted.contains("kHz") ||
                        formatted.contains("MHz") || formatted.contains("GHz"));
        }

        #[test]
        fn test_duration_formatting_properties(seconds in 0.0f64..86400.0f64) {
            let formatted = format_duration(seconds);
            prop_assert!(formatted.contains(':'));
            prop_assert!(formatted.len() >= 5); // At least MM:SS
        }
    }

    #[test]
    fn test_edge_cases() {
        // Test with very small numbers
        assert_eq!(format_frequency(1), "1 Hz");
        assert_eq!(format_duration(0.1), "00:00");

        // Test with very large numbers
        assert_eq!(format_frequency(999_999_999_999), "1000.000 GHz");
        assert_eq!(format_duration(359999.0), "99:59:59");

        // Test empty and edge case strings
        assert!(!validate_system_id(""));
        assert_eq!(sanitize_filename(""), "");
        assert_eq!(sanitize_filename("..."), "..."); // Dots are kept

        // Test unicode handling (Unicode alphanumeric chars are kept)
        assert_eq!(sanitize_filename("café.mp3"), "café.mp3");
        assert_eq!(sanitize_filename("файл.mp3"), "файл.mp3");
    }

    #[test]
    fn test_filename_parsing_edge_cases() {
        // Test with minimal valid filename
        let minimal = "20240101_000000_S_TG1_FROM_1.mp3";
        let result = parse_sdrtrunk_filename(minimal).unwrap();
        assert_eq!(result.talkgroup_id, 1);
        assert_eq!(result.radio_id, 1);

        // Test with no TG prefix
        let no_tg = "20240101_000000_System_123_FROM_456.mp3";
        let result = parse_sdrtrunk_filename(no_tg).unwrap();
        assert_eq!(result.talkgroup_id, 0); // Should default to 0

        // Test with no radio ID (but still has FROM)
        let no_radio = "20240101_000000_System_TG123_FROM_.mp3";
        let result = parse_sdrtrunk_filename(no_radio).unwrap();
        assert_eq!(result.radio_id, 0); // Should default to 0
    }

    // Property-based tests
    mod proptests {
        use super::*;

        proptest::proptest! {
            /// Property: Valid SDRTrunk filenames should always parse successfully
            #[test]
            fn valid_sdrtrunk_filename_always_parses(
                year in 2000u32..2100,
                month in 1u32..13,
                day in 1u32..29,  // Safe for all months
                hour in 0u32..24,
                minute in 0u32..60,
                second in 0u32..60,
                talkgroup in 1u32..100000,
                radio_id in 1000000u32..10000000,
                system in "[a-zA-Z_]{1,20}",
            ) {
                let filename = format!(
                    "{:04}{:02}{:02}_{:02}{:02}{:02}_{}_TG{}_FROM_{}.mp3",
                    year, month, day, hour, minute, second, system, talkgroup, radio_id
                );

                let result = parse_sdrtrunk_filename(&filename);
                prop_assert!(result.is_ok(), "Valid filename should parse: {}", filename);

                let file_data = result.unwrap();
                prop_assert_eq!(file_data.talkgroup_id, i32::try_from(talkgroup).unwrap());
                prop_assert_eq!(file_data.radio_id, i32::try_from(radio_id).unwrap());
                prop_assert_eq!(file_data.filename, filename);
            }

            /// Property: System IDs with valid characters should validate
            #[test]
            fn valid_system_id_characters_always_validate(
                id in "[a-zA-Z0-9_-]{1,50}"
            ) {
                prop_assert!(validate_system_id(&id), "Valid system ID should pass: {}", id);
            }

            /// Property: System IDs over 50 chars should fail
            #[test]
            fn system_id_over_length_always_fails(
                extra_chars in 1usize..100,
            ) {
                let id = "a".repeat(51 + extra_chars);
                prop_assert!(!validate_system_id(&id), "Overlong system ID should fail: length={}", id.len());
            }

            /// Property: File extension validation is case-insensitive
            #[test]
            fn file_extension_validation_is_case_insensitive(
                ext in "[mM][pP]3",
            ) {
                let filename = format!("test.{}", ext);
                let allowed = vec!["mp3".to_string()];
                prop_assert!(validate_file_extension(&filename, &allowed));
            }

            /// Property: Sanitized filenames contain only safe characters
            #[test]
            fn sanitized_filenames_only_safe_chars(
                filename in "\\PC{1,100}",  // Any Unicode string
            ) {
                let sanitized = sanitize_filename(&filename);
                prop_assert!(
                    sanitized.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-'),
                    "Sanitized filename should only contain safe chars: {}",
                    sanitized
                );
            }

            /// Property: Sanitization is idempotent
            #[test]
            fn sanitization_is_idempotent(
                filename in "\\PC{1,50}",
            ) {
                let once = sanitize_filename(&filename);
                let twice = sanitize_filename(&once);
                prop_assert_eq!(once, twice, "Sanitization should be idempotent");
            }

            /// Property: Sanitized filenames never empty if input not empty
            #[test]
            fn sanitized_filenames_preserve_non_emptiness(
                filename in "\\PC{1,50}",
            ) {
                let sanitized = sanitize_filename(&filename);
                // If original has any safe chars, sanitized should not be empty
                if filename.chars().any(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-') {
                    prop_assert!(!sanitized.is_empty(), "Sanitized should not be empty when input has safe chars");
                }
            }
        }
    }
}
