//! Error types for the `SDRTrunk` transcriber

use std::{error::Error as StdError, fmt};

/// Main error type for the `SDRTrunk` transcriber
#[derive(Debug)]
pub enum Error {
    /// I/O error
    Io(std::io::Error),

    /// Configuration error
    Configuration {
        /// Error message
        message: String,
    },

    /// Validation error
    Validation {
        /// Field that failed validation
        field: String,
        /// Validation error message
        message: String,
    },

    /// Database error
    Database(String),

    /// File processing error
    FileProcessing(String),

    /// Audio format error
    UnsupportedAudioFormat {
        /// The unsupported format
        format: String,
    },

    /// File size error
    FileSizeExceeded {
        /// Actual file size
        size: u64,
        /// Maximum allowed size
        max_size: u64,
    },

    /// Authentication error
    Authentication(String),

    /// Rate limit error
    RateLimitExceeded {
        /// Rate limit message
        message: String,
    },

    /// Resource exhausted error
    ResourceExhausted {
        /// Resource that was exhausted
        resource: String,
    },

    /// Timeout error
    Timeout {
        /// Timeout duration in milliseconds
        duration_ms: u64,
    },

    /// Not found error
    NotFound {
        /// Resource that was not found
        resource: String,
    },

    /// Serialization error
    Serialization(serde_json::Error),

    /// Transcription error
    Transcription(String),

    /// Other error
    Other(String),
}

/// Result type alias using our Error type
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Configuration { message } => write!(f, "Configuration error: {message}"),
            Self::Validation { field, message } => {
                write!(f, "Validation error: {field} - {message}")
            }
            Self::Database(msg) => write!(f, "Database error: {msg}"),
            Self::FileProcessing(msg) => write!(f, "File processing error: {msg}"),
            Self::UnsupportedAudioFormat { format } => {
                write!(f, "Audio format not supported: {format}")
            }
            Self::FileSizeExceeded { size, max_size } => {
                write!(f, "File size {size} exceeds maximum of {max_size}")
            }
            Self::Authentication(msg) => write!(f, "Authentication failed: {msg}"),
            Self::RateLimitExceeded { message } => write!(f, "Rate limit exceeded: {message}"),
            Self::ResourceExhausted { resource } => write!(f, "Resource exhausted: {resource}"),
            Self::Timeout { duration_ms } => {
                write!(f, "Operation timed out after {duration_ms}ms")
            }
            Self::NotFound { resource } => write!(f, "Resource not found: {resource}"),
            Self::Serialization(err) => write!(f, "Serialization error: {err}"),
            Self::Transcription(msg) => write!(f, "Transcription error: {msg}"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Serialization(err) => Some(err),
            _ => None,
        }
    }
}

// From implementations for automatic conversions
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err)
    }
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
#[allow(
    clippy::unreadable_literal,
    clippy::missing_panics_doc,
    clippy::uninlined_format_args,
    clippy::missing_errors_doc,
    clippy::unnecessary_wraps,
    clippy::match_same_arms,
    clippy::manual_string_new
)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json;
    use std::error::Error as StdError;
    use std::io;

    #[test]
    fn test_io_error_conversion() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let app_error = Error::from(io_error);

        match app_error {
            Error::Io(_) => {}
            _ => panic!("Expected Io error variant"),
        }

        assert!(format!("{}", app_error).contains("I/O error"));
    }

    #[test]
    fn test_configuration_error() {
        let error = Error::Configuration {
            message: "Invalid database URL".to_string(),
        };

        assert_eq!(
            format!("{}", error),
            "Configuration error: Invalid database URL"
        );
    }

    #[test]
    fn test_validation_error() {
        let error = Error::Validation {
            field: "system_id".to_string(),
            message: "Field is required".to_string(),
        };

        assert_eq!(
            format!("{}", error),
            "Validation error: system_id - Field is required"
        );
    }

    #[test]
    fn test_database_error() {
        let error = Error::Database("Connection failed".to_string());
        assert_eq!(format!("{}", error), "Database error: Connection failed");
    }

    #[test]
    fn test_file_processing_error() {
        let error = Error::FileProcessing("Invalid audio format".to_string());
        assert_eq!(
            format!("{}", error),
            "File processing error: Invalid audio format"
        );
    }

    #[test]
    fn test_unsupported_audio_format_error() {
        let error = Error::UnsupportedAudioFormat {
            format: "aac".to_string(),
        };

        assert_eq!(format!("{}", error), "Audio format not supported: aac");
    }

    #[test]
    fn test_file_size_exceeded_error() {
        let error = Error::FileSizeExceeded {
            size: 150_000_000,
            max_size: 100_000_000,
        };

        assert_eq!(
            format!("{}", error),
            "File size 150000000 exceeds maximum of 100000000"
        );
    }

    #[test]
    fn test_authentication_error() {
        let error = Error::Authentication("Invalid API key".to_string());
        assert_eq!(
            format!("{}", error),
            "Authentication failed: Invalid API key"
        );
    }

    #[test]
    fn test_rate_limit_exceeded_error() {
        let error = Error::RateLimitExceeded {
            message: "Too many requests".to_string(),
        };

        assert_eq!(
            format!("{}", error),
            "Rate limit exceeded: Too many requests"
        );
    }

    #[test]
    fn test_resource_exhausted_error() {
        let error = Error::ResourceExhausted {
            resource: "Database connections".to_string(),
        };

        assert_eq!(
            format!("{}", error),
            "Resource exhausted: Database connections"
        );
    }

    #[test]
    fn test_timeout_error() {
        let error = Error::Timeout { duration_ms: 30000 };

        assert_eq!(format!("{}", error), "Operation timed out after 30000ms");
    }

    #[test]
    fn test_not_found_error() {
        let error = Error::NotFound {
            resource: "User ID 123".to_string(),
        };

        assert_eq!(format!("{}", error), "Resource not found: User ID 123");
    }

    #[test]
    fn test_serialization_error_conversion() {
        let json_str = r#"{"invalid": json}"#;
        let json_error = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let app_error = Error::from(json_error);

        match app_error {
            Error::Serialization(_) => {}
            _ => panic!("Expected Serialization error variant"),
        }

        assert!(format!("{}", app_error).contains("Serialization error"));
    }

    #[test]
    fn test_other_error() {
        let error = Error::Other("Unexpected error occurred".to_string());
        assert_eq!(format!("{}", error), "Unexpected error occurred");
    }

    #[test]
    fn test_error_debug_formatting() {
        let error = Error::Configuration {
            message: "Missing required field".to_string(),
        };

        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("Configuration"));
        assert!(debug_str.contains("Missing required field"));
    }

    #[test]
    fn test_error_chain() {
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
        let app_error = Error::from(io_error);

        // Test that the error chain is preserved
        assert!(app_error.source().is_some());
    }

    #[test]
    fn test_result_type_alias() {
        fn returns_result() -> Result<String> {
            Ok("success".to_string())
        }

        fn returns_error() -> Result<String> {
            Err(Error::Other("test error".to_string()))
        }

        assert!(returns_result().is_ok());
        assert!(returns_error().is_err());
    }

    #[test]
    fn test_error_patterns() {
        let errors = vec![
            Error::Configuration {
                message: "test".to_string(),
            },
            Error::Database("test".to_string()),
            Error::Authentication("test".to_string()),
            Error::Other("test".to_string()),
        ];

        for error in errors {
            match error {
                Error::Configuration { .. } => {}
                Error::Database(_) => {}
                Error::Authentication(_) => {}
                Error::Other(_) => {}
                _ => panic!("Unexpected error variant"),
            }
        }
    }

    #[test]
    fn test_error_equality_on_message() {
        let error1 = Error::Database("Connection failed".to_string());
        let error2 = Error::Database("Connection failed".to_string());

        // Note: We can't directly compare Error variants for equality since they don't implement PartialEq
        // But we can compare their string representations
        assert_eq!(format!("{}", error1), format!("{}", error2));
    }

    #[test]
    fn test_complex_error_scenarios() {
        // Test file size error with realistic values
        let error = Error::FileSizeExceeded {
            size: 150 * 1024 * 1024,     // 150 MB
            max_size: 100 * 1024 * 1024, // 100 MB
        };

        let error_msg = format!("{}", error);
        assert!(error_msg.contains("157286400"));
        assert!(error_msg.contains("104857600"));
    }

    #[test]
    fn test_validation_error_with_special_characters() {
        let error = Error::Validation {
            field: "user.email".to_string(),
            message: "Must contain @ symbol".to_string(),
        };

        let error_msg = format!("{}", error);
        assert!(error_msg.contains("user.email"));
        assert!(error_msg.contains("Must contain @ symbol"));
    }

    #[test]
    fn test_timeout_error_with_zero_duration() {
        let error = Error::Timeout { duration_ms: 0 };

        assert_eq!(format!("{}", error), "Operation timed out after 0ms");
    }

    #[test]
    fn test_resource_exhausted_with_empty_resource() {
        let error = Error::ResourceExhausted {
            resource: "".to_string(),
        };

        assert_eq!(format!("{}", error), "Resource exhausted: ");
    }

    #[test]
    fn test_error_source_for_non_io_errors() {
        let error = Error::Configuration {
            message: "test".to_string(),
        };
        assert!(error.source().is_none());

        let error = Error::Database("test".to_string());
        assert!(error.source().is_none());

        let error = Error::Validation {
            field: "test".to_string(),
            message: "test".to_string(),
        };
        assert!(error.source().is_none());
    }

    #[test]
    fn test_serialization_error_source() {
        let json_str = r#"{"invalid": json}"#;
        let json_error = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let app_error = Error::from(json_error);

        assert!(app_error.source().is_some());
    }

    #[test]
    fn test_all_error_display_variants() {
        let test_cases = vec![
            (
                Error::Io(io::Error::other("test")),
                "I/O error:",
            ),
            (
                Error::Configuration {
                    message: "config error".to_string(),
                },
                "Configuration error: config error",
            ),
            (
                Error::Validation {
                    field: "field1".to_string(),
                    message: "invalid".to_string(),
                },
                "Validation error: field1 - invalid",
            ),
            (
                Error::Database("db error".to_string()),
                "Database error: db error",
            ),
            (
                Error::FileProcessing("file error".to_string()),
                "File processing error: file error",
            ),
            (
                Error::UnsupportedAudioFormat {
                    format: "wav".to_string(),
                },
                "Audio format not supported: wav",
            ),
            (
                Error::FileSizeExceeded {
                    size: 200,
                    max_size: 100,
                },
                "File size 200 exceeds maximum of 100",
            ),
            (
                Error::Authentication("auth error".to_string()),
                "Authentication failed: auth error",
            ),
            (
                Error::RateLimitExceeded {
                    message: "rate limit".to_string(),
                },
                "Rate limit exceeded: rate limit",
            ),
            (
                Error::ResourceExhausted {
                    resource: "memory".to_string(),
                },
                "Resource exhausted: memory",
            ),
            (
                Error::Timeout { duration_ms: 5000 },
                "Operation timed out after 5000ms",
            ),
            (
                Error::NotFound {
                    resource: "file.txt".to_string(),
                },
                "Resource not found: file.txt",
            ),
            (Error::Other("other error".to_string()), "other error"),
        ];

        for (error, expected_contains) in test_cases {
            let display_str = format!("{}", error);
            assert!(
                display_str.contains(expected_contains),
                "Error display '{}' should contain '{}'",
                display_str,
                expected_contains
            );
        }
    }

    #[test]
    fn test_error_from_implementations() {
        // Test automatic conversion from std::io::Error
        let io_err = io::Error::new(io::ErrorKind::NotFound, "not found");
        let _: Error = io_err.into();

        // Test automatic conversion from serde_json::Error
        let json_err = serde_json::from_str::<i32>("invalid").unwrap_err();
        let _: Error = json_err.into();
    }

    #[test]
    fn test_result_alias_usage() {
        fn test_function() -> Result<i32> {
            Ok(42)
        }

        fn test_error_function() -> Result<i32> {
            Err(Error::Other("test error".to_string()))
        }

        assert_eq!(test_function().unwrap(), 42);
        assert!(test_error_function().is_err());
    }
}
