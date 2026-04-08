#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

//! Pure validated types for the `SDRTrunk` transcriber.
//!
//! This crate defines the core domain types for the `SDRTrunk` radio
//! transcription system. All types use the newtype pattern with
//! validation at construction, preventing invalid data from propagating.
//!
//! # Design Principles
//!
//! - **Zero I/O dependencies**: no tokio, sqlx, or async runtime
//! - **Validation at construction**: `SystemId::new("")` returns `Err`
//! - **Type safety**: `TalkgroupId` and `RadioId` are distinct types
//! - **Serde transparent**: newtypes serialize as their inner value
//! - **Optional sqlx**: enable the `sqlx` feature for database integration
//!
//! # Examples
//!
//! ```
//! use sdrtrunk_types::{SystemId, TalkgroupId, Frequency};
//!
//! let system = SystemId::new("police-dispatch").unwrap();
//! let talkgroup = TalkgroupId::new(52197).unwrap();
//! let freq = Frequency::new(851_012_500).unwrap();
//!
//! assert_eq!(system.as_str(), "police-dispatch");
//! assert_eq!(talkgroup.as_i32(), 52197);
//! assert_eq!(freq.as_hz(), 851_012_500);
//! ```
//!
//! # Modules
//!
//! - [`system`]: System identifiers (validated string, max 50 chars)
//! - [`talkgroup`]: Talkgroup IDs and labels
//! - [`radio`]: Radio identifiers
//! - [`frequency`]: Radio frequencies in Hz
//! - [`call`]: Call identifiers and the `RadioCall` domain type
//! - [`status`]: Transcription status enumeration
//! - [`error`]: Validation and transport error types
//! - [`app_error`]: Application-level error type

pub mod app_error;
pub mod call;
pub mod error;
pub mod frequency;
pub mod radio;
pub mod status;
pub mod system;
pub mod talkgroup;

pub use app_error::{AppError, AppResult};
pub use call::{CallId, RadioCall};
pub use error::{Error, Result, TransportError, ValidationError};
pub use frequency::Frequency;
pub use radio::RadioId;
pub use status::TranscriptionStatus;
pub use system::SystemId;
pub use talkgroup::{TalkgroupId, TalkgroupLabel};
