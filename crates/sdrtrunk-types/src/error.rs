//! Error types for the `SDRTrunk` types layer.

use std::time::Duration;
use thiserror::Error;

/// Validation errors for user input.
///
/// These errors occur at the boundary when constructing
/// validated types from untrusted input.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// System ID cannot be empty
    #[error("system ID cannot be empty")]
    EmptySystemId,

    /// System ID exceeds maximum length
    #[error("system ID too long: {length} chars (max {max})")]
    SystemIdTooLong {
        /// Actual length
        length: usize,
        /// Maximum allowed length
        max: usize,
    },

    /// Invalid talkgroup ID
    #[error("invalid talkgroup ID: {value} (must be positive)")]
    InvalidTalkgroupId {
        /// The invalid value
        value: i32,
    },

    /// Invalid radio ID
    #[error("invalid radio ID: {value} (must be positive)")]
    InvalidRadioId {
        /// The invalid value
        value: i32,
    },

    /// Invalid frequency
    #[error("invalid frequency: {value} Hz (must be positive)")]
    InvalidFrequency {
        /// The invalid value
        value: i64,
    },

    /// File size exceeded
    #[error("file size {size} exceeds maximum {max}")]
    FileSizeExceeded {
        /// Actual size
        size: u64,
        /// Maximum size
        max: u64,
    },

    /// Unsupported audio format
    #[error("unsupported audio format: {format}")]
    UnsupportedAudioFormat {
        /// The unsupported format
        format: String,
    },

    /// Generic validation error
    #[error("validation failed: {field} - {message}")]
    Generic {
        /// Field that failed validation
        field: String,
        /// Error message
        message: String,
    },
}

/// Transport and I/O errors.
#[derive(Debug, Error)]
pub enum TransportError {
    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    Http(String),

    /// Network connection lost
    #[error("connection lost")]
    Disconnected,

    /// Timeout waiting for response
    #[error("timeout after {0:?}")]
    Timeout(Duration),
}

/// Top-level error type for all public APIs.
///
/// This enum wraps layer-specific errors and provides automatic
/// conversion via the `From` trait, enabling seamless use of
/// the `?` operator.
#[derive(Debug, Error)]
pub enum Error {
    /// Validation error - user input problem
    #[error(transparent)]
    Validation(#[from] ValidationError),

    /// Transport error - network/I/O problem
    #[error(transparent)]
    Transport(#[from] TransportError),

    /// Operation timed out
    #[error("operation timed out after {0:?}")]
    Timeout(Duration),

    /// Resource not found
    #[error("resource not found: {resource}")]
    NotFound {
        /// Resource type
        resource: String,
    },

    /// Authentication failed
    #[error("authentication failed: {reason}")]
    AuthenticationFailed {
        /// Failure reason
        reason: String,
    },

    /// Rate limit exceeded
    #[error("rate limit exceeded: {message}")]
    RateLimitExceeded {
        /// Error message
        message: String,
    },

    /// Storage error (from sdrtrunk-storage)
    #[error("storage error: {0}")]
    Storage(String),

    /// Service error (from sdrtrunk-service)
    #[error("service error: {0}")]
    Service(String),

    /// Protocol error (from sdrtrunk-protocol)
    #[error("protocol error: {0}")]
    Protocol(String),
}

/// Convenience type alias for Results using our Error type.
pub type Result<T> = std::result::Result<T, Error>;
