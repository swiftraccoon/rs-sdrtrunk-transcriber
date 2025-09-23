//! Transcription service for SDRTrunk audio files with speaker diarization
//!
//! This crate provides a flexible transcription framework supporting multiple backends,
//! with a primary focus on WhisperX integration for high-quality speech-to-text with
//! speaker diarization capabilities.

#![forbid(unsafe_code)]
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    missing_docs
)]
#![allow(clippy::module_name_repetitions, clippy::must_use_candidate)]

pub mod error;
pub mod mock;
pub mod service;
pub mod types;
pub mod whisperx;
pub mod worker;

pub use error::{TranscriptionError, TranscriptionResult};
pub use service::TranscriptionService;
pub use types::{
    SpeakerSegment, TranscriptionConfig, TranscriptionOptions, TranscriptionRequest,
    TranscriptionResponse, TranscriptionSegment, TranscriptionStatus, WordSegment,
};
pub use worker::TranscriptionWorkerPool;

// Re-export commonly used items
pub use mock::MockTranscriptionService;
pub use whisperx::WhisperXService;