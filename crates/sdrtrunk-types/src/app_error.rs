//! Application-level error types for the `SDRTrunk` transcriber

use thiserror::Error;

/// Application-level error type for the `SDRTrunk` transcriber.
///
/// This error covers high-level concerns such as database failures,
/// authentication problems, and file-processing issues.  Lower-level
/// validation and transport errors live in [`crate::error`].
#[derive(Debug, Error)]
pub enum AppError {
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Configuration error
    #[error("Configuration error: {message}")]
    Configuration {
        /// Error message
        message: String,
    },

    /// Validation error
    #[error("Validation error: {field} - {message}")]
    Validation {
        /// Field that failed validation
        field: String,
        /// Validation error message
        message: String,
    },

    /// Database error
    #[error("Database error: {0}")]
    Database(String),

    /// File processing error
    #[error("File processing error: {0}")]
    FileProcessing(String),

    /// Audio format error
    #[error("Audio format not supported: {format}")]
    UnsupportedAudioFormat {
        /// The unsupported format
        format: String,
    },

    /// File size error
    #[error("File size {size} exceeds maximum of {max_size}")]
    FileSizeExceeded {
        /// Actual file size
        size: u64,
        /// Maximum allowed size
        max_size: u64,
    },

    /// Authentication error
    #[error("Authentication failed: {0}")]
    Authentication(String),

    /// Rate limit error
    #[error("Rate limit exceeded: {message}")]
    RateLimitExceeded {
        /// Rate limit message
        message: String,
    },

    /// Resource exhausted error
    #[error("Resource exhausted: {resource}")]
    ResourceExhausted {
        /// Resource that was exhausted
        resource: String,
    },

    /// Timeout error
    #[error("Operation timed out after {duration_ms}ms")]
    Timeout {
        /// Timeout duration in milliseconds
        duration_ms: u64,
    },

    /// Not found error
    #[error("Resource not found: {resource}")]
    NotFound {
        /// Resource that was not found
        resource: String,
    },

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Transcription error
    #[error("Transcription error: {0}")]
    Transcription(String),

    /// Other error
    #[error("{0}")]
    Other(String),
}

/// Result type alias using [`AppError`].
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::unnecessary_wraps,
    unused_results
)]
mod tests {
    use super::*;
    use std::error::Error as StdError;
    use std::io;

    #[test]
    fn test_io_error_conversion() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
        let app_error = AppError::from(io_error);

        assert!(matches!(app_error, AppError::Io(_)));
        assert!(format!("{app_error}").contains("I/O error"));
    }

    #[test]
    fn test_configuration_error() {
        let error = AppError::Configuration {
            message: "Invalid database URL".to_string(),
        };
        assert_eq!(
            format!("{error}"),
            "Configuration error: Invalid database URL"
        );
    }

    #[test]
    fn test_validation_error() {
        let error = AppError::Validation {
            field: "system_id".to_string(),
            message: "Field is required".to_string(),
        };
        assert_eq!(
            format!("{error}"),
            "Validation error: system_id - Field is required"
        );
    }

    #[test]
    fn test_database_error() {
        let error = AppError::Database("Connection failed".to_string());
        assert_eq!(format!("{error}"), "Database error: Connection failed");
    }

    #[test]
    fn test_file_processing_error() {
        let error = AppError::FileProcessing("Invalid audio format".to_string());
        assert_eq!(
            format!("{error}"),
            "File processing error: Invalid audio format"
        );
    }

    #[test]
    fn test_unsupported_audio_format_error() {
        let error = AppError::UnsupportedAudioFormat {
            format: "aac".to_string(),
        };
        assert_eq!(format!("{error}"), "Audio format not supported: aac");
    }

    #[test]
    fn test_file_size_exceeded_error() {
        let error = AppError::FileSizeExceeded {
            size: 150_000_000,
            max_size: 100_000_000,
        };
        assert_eq!(
            format!("{error}"),
            "File size 150000000 exceeds maximum of 100000000"
        );
    }

    #[test]
    fn test_authentication_error() {
        let error = AppError::Authentication("Invalid API key".to_string());
        assert_eq!(format!("{error}"), "Authentication failed: Invalid API key");
    }

    #[test]
    fn test_not_found_error() {
        let error = AppError::NotFound {
            resource: "User ID 123".to_string(),
        };
        assert_eq!(format!("{error}"), "Resource not found: User ID 123");
    }

    #[test]
    fn test_serialization_error_conversion() {
        let json_str = r#"{"invalid": json}"#;
        let json_error = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let app_error = AppError::from(json_error);

        assert!(matches!(app_error, AppError::Serialization(_)));
        assert!(format!("{app_error}").contains("Serialization error"));
    }

    #[test]
    fn test_other_error() {
        let error = AppError::Other("Unexpected error occurred".to_string());
        assert_eq!(format!("{error}"), "Unexpected error occurred");
    }

    #[test]
    fn test_error_chain() {
        let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "Access denied");
        let app_error = AppError::from(io_error);
        assert!(app_error.source().is_some());
    }

    #[test]
    fn test_result_type_alias() {
        fn returns_result() -> AppResult<String> {
            Ok("success".to_string())
        }

        fn returns_error() -> AppResult<String> {
            Err(AppError::Other("test error".to_string()))
        }

        assert!(returns_result().is_ok());
        assert!(returns_error().is_err());
    }
}
