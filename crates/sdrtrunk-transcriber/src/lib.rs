//! Transcription service for `SDRTrunk` audio files with speaker diarization
//!
//! This crate provides a flexible transcription framework supporting multiple backends,
//! with a primary focus on `WhisperX` integration for high-quality speech-to-text with
//! speaker diarization capabilities.

#![forbid(unsafe_code)]
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    missing_docs
)]
#![allow(
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::multiple_crate_versions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::struct_excessive_bools,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::significant_drop_tightening,
    clippy::unnecessary_literal_bound,
    clippy::type_complexity,
    clippy::needless_borrows_for_generic_args,
    clippy::items_after_statements,
    clippy::float_cmp,
    clippy::too_many_lines,
    clippy::redundant_closure,
    clippy::unused_async,
    clippy::unused_self,
    clippy::return_self_not_must_use,
    clippy::suboptimal_flops,
    clippy::redundant_clone,
    clippy::match_same_arms,
    clippy::useless_format,
    clippy::uninlined_format_args,
    clippy::collapsible_if,
    clippy::single_match,
    clippy::single_match_else
)]

pub mod error;
pub mod mock;
pub mod service;
pub mod types;
pub mod whisperx;
pub mod worker;

pub use error::{TranscriptionError, TranscriptionResult};
pub use sdrtrunk_core::{TranscriptionConfig, TranscriptionStatus};
pub use service::TranscriptionService;
pub use types::{
    SpeakerSegment, TranscriptionOptions, TranscriptionRequest, TranscriptionResponse,
    TranscriptionSegment, WordSegment,
};
pub use worker::TranscriptionWorkerPool;

// Re-export commonly used items
pub use mock::MockTranscriptionService;
pub use whisperx::WhisperXService;
