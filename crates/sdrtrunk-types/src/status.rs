//! Transcription status types.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Transcription status enumeration.
///
/// # Examples
///
/// ```
/// use sdrtrunk_types::TranscriptionStatus;
///
/// let status = TranscriptionStatus::default();
/// assert_eq!(status, TranscriptionStatus::Pending);
/// assert_eq!(format!("{status}"), "pending");
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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
    /// Transcription cancelled
    Cancelled,
    /// No transcription requested
    None,
}

impl Default for TranscriptionStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl fmt::Display for TranscriptionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Processing => write!(f, "processing"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::None => write!(f, "none"),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::missing_panics_doc)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        assert_eq!(TranscriptionStatus::default(), TranscriptionStatus::Pending);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", TranscriptionStatus::Pending), "pending");
        assert_eq!(format!("{}", TranscriptionStatus::Processing), "processing");
        assert_eq!(format!("{}", TranscriptionStatus::Completed), "completed");
        assert_eq!(format!("{}", TranscriptionStatus::Failed), "failed");
        assert_eq!(format!("{}", TranscriptionStatus::Cancelled), "cancelled");
        assert_eq!(format!("{}", TranscriptionStatus::None), "none");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let status = TranscriptionStatus::Completed;
        let serialized = serde_json::to_string(&status).unwrap();
        assert_eq!(serialized, "\"completed\"");

        let deserialized: TranscriptionStatus = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, TranscriptionStatus::Completed);
    }
}
