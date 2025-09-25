//! Error types for the transcription service

use std::fmt;
use std::io;
use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for transcription operations
pub type TranscriptionResult<T> = Result<T, TranscriptionError>;

/// Errors that can occur during transcription operations
#[derive(Error, Debug)]
pub enum TranscriptionError {
    /// File not found or inaccessible
    #[error("File not found or inaccessible: {path}")]
    FileNotFound {
        /// Path to the file
        path: PathBuf,
    },

    /// Invalid audio format
    #[error("Invalid audio format: {format}. Supported formats: {supported:?}")]
    InvalidAudioFormat {
        /// Detected format
        format: String,
        /// List of supported formats
        supported: Vec<String>,
    },

    /// Service unavailable
    #[error("Transcription service unavailable: {service}")]
    ServiceUnavailable {
        /// Service name
        service: String,
    },

    /// Service communication error
    #[error("Failed to communicate with transcription service: {message}")]
    ServiceCommunication {
        /// Error message
        message: String,
    },

    /// Processing timeout
    #[error("Transcription processing timeout after {seconds} seconds")]
    ProcessingTimeout {
        /// Timeout duration
        seconds: u64,
    },

    /// Model loading error
    #[error("Failed to load transcription model: {model}")]
    ModelLoadError {
        /// Model name
        model: String,
    },

    /// Transcription processing error
    #[error("Transcription processing failed: {reason}")]
    ProcessingFailed {
        /// Failure reason
        reason: String,
    },

    /// Configuration error
    #[error("Invalid configuration: {message}")]
    ConfigurationError {
        /// Error message
        message: String,
    },

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP request error
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// Database error
    #[error("Database error: {0}")]
    Database(String),

    /// Queue full error
    #[error("Transcription queue is full (max: {max_size})")]
    QueueFull {
        /// Maximum queue size
        max_size: usize,
    },

    /// Worker pool error
    #[error("Worker pool error: {message}")]
    WorkerPool {
        /// Error message
        message: String,
    },

    /// Python service error
    #[error("Python service error: {message}")]
    PythonService {
        /// Error message
        message: String,
    },

    /// Subprocess error
    #[error("Subprocess error: {message}")]
    Subprocess {
        /// Error message
        message: String,
    },

    /// Validation error
    #[error("Validation error: {message}")]
    Validation {
        /// Error message
        message: String,
    },

    /// Unknown error
    #[error("Unknown error occurred")]
    Unknown,
}

impl TranscriptionError {
    /// Create a file not found error
    pub fn file_not_found(path: impl Into<PathBuf>) -> Self {
        Self::FileNotFound { path: path.into() }
    }

    /// Create an invalid audio format error
    pub fn invalid_format(format: impl Into<String>, supported: Vec<String>) -> Self {
        Self::InvalidAudioFormat {
            format: format.into(),
            supported,
        }
    }

    /// Create a service unavailable error
    pub fn service_unavailable(service: impl Into<String>) -> Self {
        Self::ServiceUnavailable {
            service: service.into(),
        }
    }

    /// Create a service communication error
    pub fn service_communication(message: impl Into<String>) -> Self {
        Self::ServiceCommunication {
            message: message.into(),
        }
    }

    /// Create a processing timeout error
    pub const fn timeout(seconds: u64) -> Self {
        Self::ProcessingTimeout { seconds }
    }

    /// Create a model load error
    pub fn model_load_error(model: impl Into<String>) -> Self {
        Self::ModelLoadError {
            model: model.into(),
        }
    }

    /// Create a processing failed error
    pub fn processing_failed(reason: impl Into<String>) -> Self {
        Self::ProcessingFailed {
            reason: reason.into(),
        }
    }

    /// Create a configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::ConfigurationError {
            message: message.into(),
        }
    }

    /// Create a database error
    pub fn database(message: impl fmt::Display) -> Self {
        Self::Database(message.to_string())
    }

    /// Create a queue full error
    pub const fn queue_full(max_size: usize) -> Self {
        Self::QueueFull { max_size }
    }

    /// Create a worker pool error
    pub fn worker_pool(message: impl Into<String>) -> Self {
        Self::WorkerPool {
            message: message.into(),
        }
    }

    /// Create a Python service error
    pub fn python_service(message: impl Into<String>) -> Self {
        Self::PythonService {
            message: message.into(),
        }
    }

    /// Create a subprocess error
    pub fn subprocess(message: impl Into<String>) -> Self {
        Self::Subprocess {
            message: message.into(),
        }
    }

    /// Create a validation error
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    /// Check if error is retryable
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::ServiceUnavailable { .. }
                | Self::ServiceCommunication { .. }
                | Self::ProcessingTimeout { .. }
                | Self::Http(_)
                | Self::QueueFull { .. }
        )
    }

    /// Get error severity level for logging
    pub const fn severity(&self) -> ErrorSeverity {
        match self {
            Self::FileNotFound { .. } | Self::InvalidAudioFormat { .. } => ErrorSeverity::Warning,
            Self::ServiceUnavailable { .. }
            | Self::ModelLoadError { .. }
            | Self::WorkerPool { .. }
            | Self::PythonService { .. } => ErrorSeverity::Error,
            Self::ConfigurationError { .. } | Self::Unknown => ErrorSeverity::Critical,
            _ => ErrorSeverity::Info,
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Informational - not really an error
    Info,
    /// Warning - should be investigated
    Warning,
    /// Error - operation failed but system continues
    Error,
    /// Critical - system health impacted
    Critical,
}

// Conversions to core error types
impl From<TranscriptionError> for sdrtrunk_core::context_error::ContextError {
    fn from(err: TranscriptionError) -> Self {
        Self::with_context(err, "Transcription service error")
    }
}

impl From<TranscriptionError> for sdrtrunk_core::Error {
    fn from(err: TranscriptionError) -> Self {
        Self::Transcription(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = TranscriptionError::file_not_found("/test/path.mp3");
        assert!(matches!(err, TranscriptionError::FileNotFound { .. }));

        let err =
            TranscriptionError::invalid_format("aac", vec!["mp3".to_string(), "wav".to_string()]);
        assert!(matches!(err, TranscriptionError::InvalidAudioFormat { .. }));

        let err = TranscriptionError::timeout(30);
        assert!(matches!(err, TranscriptionError::ProcessingTimeout { .. }));
    }

    #[test]
    fn test_error_retryable() {
        let retryable = TranscriptionError::service_unavailable("whisperx");
        assert!(retryable.is_retryable());

        let not_retryable = TranscriptionError::file_not_found("/test.mp3");
        assert!(!not_retryable.is_retryable());

        let retryable = TranscriptionError::queue_full(100);
        assert!(retryable.is_retryable());
    }

    #[test]
    fn test_error_severity() {
        let warning = TranscriptionError::file_not_found("/test.mp3");
        assert_eq!(warning.severity(), ErrorSeverity::Warning);

        let error = TranscriptionError::service_unavailable("whisperx");
        assert_eq!(error.severity(), ErrorSeverity::Error);

        let critical = TranscriptionError::configuration("Invalid model path");
        assert_eq!(critical.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_error_display() {
        let err = TranscriptionError::processing_failed("Model crashed");
        let display = format!("{err}");
        assert!(display.contains("Model crashed"));

        let err = TranscriptionError::timeout(60);
        let display = format!("{err}");
        assert!(display.contains("60 seconds"));
    }
}
