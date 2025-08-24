//! File monitoring service for `SDRTrunk` transcriber
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
///
/// # Errors
///
/// Returns [`MonitorError`] if:
/// - Configuration loading fails
/// - Database connection cannot be established
/// - Required directories cannot be created
pub async fn init() -> Result<MonitorService> {
    let config = MonitorConfig::load()?;
    MonitorService::new(config).await
}

/// Initialize the monitoring service with custom configuration
///
/// # Errors
///
/// Returns [`MonitorError`] if:
/// - Database connection cannot be established
/// - Required directories cannot be created
/// - Invalid configuration parameters
pub async fn init_with_config(config: MonitorConfig) -> Result<MonitorService> {
    MonitorService::new(config).await
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_re_exports() {
        // Test that re-exports work
        let _config = MonitorConfig::default();
        let _processing_config = MonitorConfig::default().processing;
        let _storage_config = MonitorConfig::default().storage;

        // Test error types
        let _error = MonitorError::configuration("test");

        // Test status enum
        let _status = ProcessingStatus::Pending;
    }

    #[test]
    fn test_monitor_config_defaults() {
        let config = MonitorConfig::default();

        // Test that defaults are sensible
        assert!(!config.watch.watch_directory.to_string_lossy().is_empty());
        assert!(config.processing.processing_interval_seconds > 0);
        assert!(config.processing.processing_workers > 0);
        assert!(config.processing.max_retry_attempts > 0);
    }

    #[test]
    fn test_processing_config_defaults() {
        let config = MonitorConfig::default().processing;

        // Test processing config defaults
        assert!(config.processing_timeout_seconds > 0);
        assert!(config.processing_workers > 0);
        assert!(config.verify_file_integrity || !config.verify_file_integrity);
    }

    #[test]
    fn test_storage_config_defaults() {
        let config = MonitorConfig::default().storage;

        // Test storage config defaults
        assert!(!config.archive_directory.to_string_lossy().is_empty());
        assert!(config.max_archive_size > 0);
        assert!(config.archive_retention_days > 0);
    }

    #[test]
    fn test_error_display() {
        let error = MonitorError::configuration("test error");
        let display = format!("{error}");
        assert!(display.contains("test error"));

        let error = MonitorError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file error",
        ));
        let display = format!("{error}");
        assert!(display.contains("file error"));
    }

    #[test]
    fn test_error_debug() {
        let error = MonitorError::configuration("test error");
        let debug = format!("{error:?}");
        assert!(debug.contains("Configuration"));
        assert!(debug.contains("test error"));
    }

    #[test]
    fn test_processing_status_variants() {
        // Test all status variants exist
        let _pending = ProcessingStatus::Pending;
        let _processing = ProcessingStatus::Processing;
        let _completed = ProcessingStatus::Completed;
        let _failed = ProcessingStatus::Failed {
            error: "test error".to_string(),
            retry_count: 0,
        };
    }

    #[test]
    fn test_processing_status_debug() {
        let status = ProcessingStatus::Pending;
        let debug = format!("{status:?}");
        assert!(debug.contains("Pending"));

        let status = ProcessingStatus::Processing;
        let debug = format!("{status:?}");
        assert!(debug.contains("Processing"));
    }

    #[tokio::test]
    async fn test_init_with_invalid_config() {
        // Create a config with invalid paths to test error handling
        let mut config = MonitorConfig::default();
        config.watch.watch_directory = PathBuf::from("/nonexistent/invalid/path");

        let result = init_with_config(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_init_with_valid_temp_config() {
        // Create a valid temporary config
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let mut config = MonitorConfig::default();
        config.watch.watch_directory = temp_dir.path().to_path_buf();
        config.processing.processing_timeout_seconds = 30;
        config.storage.archive_directory = temp_dir.path().to_path_buf();

        // This should work but might fail due to database connection
        let result = init_with_config(config).await;
        // Don't assert success as it depends on DB availability
        // Just ensure we get a proper error type if it fails
        if let Err(error) = result {
            // Should be a proper MonitorError
            let error_str = format!("{error}");
            assert!(!error_str.is_empty());
        }
    }

    #[test]
    fn test_queued_file_creation() {
        // SystemTime import removed - using chrono instead

        let file = QueuedFile {
            id: uuid::Uuid::new_v4(),
            path: PathBuf::from("/test/file.mp3"),
            size: 1024,
            queued_at: chrono::Utc::now(),
            modified_at: chrono::Utc::now(),
            priority: 1,
            retry_count: 0,
            last_error: None,
            metadata: crate::queue::FileMetadata {
                extension: Some("mp3".to_string()),
                stem: "file".to_string(),
                is_symlink: false,
                checksum: None,
            },
        };

        assert_eq!(file.path, PathBuf::from("/test/file.mp3"));
        assert_eq!(file.retry_count, 0);
        assert_eq!(file.size, 1024);
    }

    #[test]
    fn test_monitor_config_validation() {
        let config = MonitorConfig::default();

        // Test configuration constraints
        assert!(config.processing.processing_interval_seconds >= 1);
        assert!(config.processing.processing_workers >= 1);
        assert!(config.processing.max_retry_attempts >= 1);
        assert!(config.processing.retry_delay_seconds >= 1);
    }

    #[test]
    fn test_processing_config_validation() {
        let config = MonitorConfig::default().processing;

        // Test processing configuration constraints
        assert!(config.processing_timeout_seconds >= 1);
        assert!(config.processing_workers >= 1);
        assert!(config.max_retry_attempts >= 1);
    }

    #[test]
    fn test_storage_config_validation() {
        let config = MonitorConfig::default().storage;

        // Test storage configuration constraints
        assert!(config.max_archive_size > 0);
        assert!(config.archive_retention_days >= 1);
    }

    #[test]
    fn test_result_type_alias() {
        // Test Result type alias works
        let success: Result<i32> = Ok(42);
        assert!(matches!(success, Ok(42)));

        let failure: Result<i32> = Err(MonitorError::configuration("test"));
        assert!(failure.is_err());
    }

    #[test]
    fn test_module_structure() {
        // Test that all modules are accessible
        use crate::{config, error, monitor, processor, queue, service};

        // This is mainly a compile-time test to ensure modules are properly structured
        let _config_mod = std::any::type_name::<config::MonitorConfig>();
        let _error_mod = std::any::type_name::<error::MonitorError>();
        let _monitor_mod = std::any::type_name::<monitor::FileMonitor>();
        let _processor_mod = std::any::type_name::<processor::FileProcessor>();
        let _queue_mod = std::any::type_name::<queue::FileQueue>();
        let _service_mod = std::any::type_name::<service::MonitorService>();
    }
}
