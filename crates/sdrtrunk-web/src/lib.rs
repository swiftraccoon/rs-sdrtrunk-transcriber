//! `SDRTrunk` Web Interface
//!
//! A modern web interface for monitoring and managing `SDRTrunk` radio call transcriptions.

#![forbid(unsafe_code)]

pub mod api_client;
pub mod websocket;

pub(crate) mod app;
pub(crate) mod components;
pub(crate) mod handlers;
pub(crate) mod pages;
pub(crate) mod routes;
pub(crate) mod state;

// Re-export the main server entry point
pub mod server;

// Re-export the main functions
pub use server::build_app;
pub use state::AppState;
