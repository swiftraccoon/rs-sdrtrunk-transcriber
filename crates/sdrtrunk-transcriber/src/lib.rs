//! Transcription service for `SDRTrunk` audio files with speaker diarization
//!
//! This crate provides a flexible transcription framework supporting multiple backends,
//! with a primary focus on `WhisperX` integration for high-quality speech-to-text with
//! speaker diarization capabilities.

#![forbid(unsafe_code)]

pub mod error;
pub mod mock;
pub mod service;
pub mod types;
pub mod whisperx;

pub use error::{TranscriptionError, TranscriptionResult};
pub use sdrtrunk_protocol::config::TranscriptionConfig;
pub use sdrtrunk_types::TranscriptionStatus;
pub use service::TranscriptionService;
pub use types::{
    SpeakerSegment, TranscriptionOptions, TranscriptionRequest, TranscriptionResponse,
    TranscriptionSegment, WordSegment,
};

// Re-export commonly used items
pub use mock::MockTranscriptionService;
pub use whisperx::WhisperXService;
