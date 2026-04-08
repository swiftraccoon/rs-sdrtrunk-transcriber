#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

//! Business logic, configuration, and validation for the `SDRTrunk` transcriber.
//!
//! This crate sits between [`sdrtrunk_types`] and the I/O layers. It contains:
//!
//! - **Configuration types**: [`Config`], `ServerConfig`, `DatabaseConfig`,
//!   `StorageConfig`, `TranscriptionConfig`, etc.
//! - **Protocol errors**: [`ProtocolError`] for serialization and format issues
//! - **Type re-exports**: [`types`] module re-exports the validated types layer
//!
//! # Design
//!
//! No async, no I/O, no database — pure data structures and validation.
//! Configuration loading (file/env) happens in the binary crates, not here.

pub mod config;
pub mod error;

pub use config::Config;
pub use error::ProtocolError;

/// Re-export validated types from the types layer.
pub use sdrtrunk_types as types;
