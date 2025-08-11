//! File monitoring service for SDRTrunk transcriber
//!
//! This crate provides a high-performance, cross-platform file monitoring service
//! that watches for new MP3 files and triggers processing workflows. The service
//! is designed to be resilient to errors and provides comprehensive logging.

#![forbid(unsafe_code)]
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    missing_docs,
    rust_2018_idioms
)]

pub mod config;
pub mod error;
pub mod monitor;
pub mod processor;
pub mod queue;
pub mod service;

// Re-export commonly used types
pub use config::{MonitorConfig, ProcessingConfig, StorageConfig as MonitorStorageConfig};
pub use error::{MonitorError, Result};
pub use monitor::FileMonitor;
pub use processor::{FileProcessor, ProcessingStatus};
pub use queue::{FileQueue, QueuedFile};
pub use service::MonitorService;

/// Initialize the monitoring service with default configuration
pub async fn init() -> Result<MonitorService> {
    let config = MonitorConfig::load()?;
    MonitorService::new(config).await
}

/// Initialize the monitoring service with custom configuration
pub async fn init_with_config(config: MonitorConfig) -> Result<MonitorService> {
    MonitorService::new(config).await
}
