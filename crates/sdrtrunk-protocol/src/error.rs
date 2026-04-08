//! Protocol layer error types.

use thiserror::Error;

/// Protocol-level serialization and format errors.
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// JSON serialization failed
    #[error("serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Invalid request format
    #[error("invalid request format: {reason}")]
    InvalidFormat {
        /// Reason for invalidity
        reason: String,
    },

    /// Missing required field
    #[error("missing required field: {field}")]
    MissingField {
        /// Field name
        field: String,
    },

    /// Field parsing failed
    #[error("failed to parse field {field}: {detail}")]
    FieldParse {
        /// Field name
        field: String,
        /// Parse error detail
        detail: String,
    },
}

/// Result type alias for protocol operations.
pub type Result<T> = std::result::Result<T, ProtocolError>;
