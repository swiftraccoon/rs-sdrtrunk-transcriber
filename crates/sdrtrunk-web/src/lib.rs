//! SDRTrunk Web Interface
//!
//! A modern web interface for monitoring and managing SDRTrunk radio call transcriptions.

#![forbid(unsafe_code)]
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    missing_docs,
    rust_2018_idioms
)]

pub mod api_client;
pub mod handlers;
pub mod routes;
pub mod server;
pub mod state;
pub mod websocket;

// Re-export the main functions
pub use server::build_app;
pub use state::AppState;