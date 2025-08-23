//! Configuration management for the file monitoring service

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Main configuration for the file monitoring service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    /// File watching configuration
    pub watch: WatchConfig,

    /// File processing configuration
    pub processing: ProcessingConfig,

    /// Storage configuration
    pub storage: StorageConfig,

    /// Database configuration (uses sdrtrunk-core's database config)
    pub database: sdrtrunk_core::config::DatabaseConfig,

    /// Queue configuration
    pub queue: QueueConfig,

    /// Service configuration
    pub service: ServiceConfig,
}

/// File watching configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    /// Directory to watch for new files
    pub watch_directory: PathBuf,

    /// File patterns to match (glob patterns)
    #[serde(default = "default_file_patterns")]
    pub file_patterns: Vec<String>,

    /// File extensions to monitor
    #[serde(default = "default_file_extensions")]
    pub file_extensions: Vec<String>,

    /// Minimum file size in bytes (ignore files smaller than this)
    #[serde(default = "default_min_file_size")]
    pub min_file_size: u64,

    /// Maximum file size in bytes (ignore files larger than this)
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    /// Debounce delay for file system events (milliseconds)
    #[serde(default = "default_debounce_delay")]
    pub debounce_delay_ms: u64,

    /// Watch subdirectories recursively
    #[serde(default = "default_recursive")]
    pub recursive: bool,

    /// Follow symbolic links
    #[serde(default = "default_follow_symlinks")]
    pub follow_symlinks: bool,
}

/// File processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    /// Processing interval in seconds
    #[serde(default = "default_processing_interval")]
    pub processing_interval_seconds: u64,

    /// Number of concurrent processing workers
    #[serde(default = "default_processing_workers")]
    pub processing_workers: usize,

    /// Maximum retry attempts for failed processing
    #[serde(default = "default_max_retry_attempts")]
    pub max_retry_attempts: u32,

    /// Retry delay in seconds (exponential backoff)
    #[serde(default = "default_retry_delay")]
    pub retry_delay_seconds: u64,

    /// Processing timeout in seconds
    #[serde(default = "default_processing_timeout")]
    pub processing_timeout_seconds: u64,

    /// Move files after processing
    #[serde(default = "default_move_after_processing")]
    pub move_after_processing: bool,

    /// Delete files after processing (only if `move_after_processing` is false)
    #[serde(default = "default_delete_after_processing")]
    pub delete_after_processing: bool,

    /// Verify file integrity before processing
    #[serde(default = "default_verify_file_integrity")]
    pub verify_file_integrity: bool,
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Archive directory for processed files
    pub archive_directory: PathBuf,

    /// Failed files directory
    pub failed_directory: PathBuf,

    /// Temporary processing directory
    pub temp_directory: PathBuf,

    /// Organize archive by date (YYYY/MM/DD structure)
    #[serde(default = "default_organize_by_date")]
    pub organize_by_date: bool,

    /// Organize archive by system ID
    #[serde(default = "default_organize_by_system")]
    pub organize_by_system: bool,

    /// Compress archived files
    #[serde(default = "default_compress_archive")]
    pub compress_archive: bool,

    /// Compression level (0-9 for gzip/zstd)
    #[serde(default = "default_compression_level")]
    pub compression_level: u32,

    /// Maximum archive directory size in bytes (cleanup old files when exceeded)
    #[serde(default = "default_max_archive_size")]
    pub max_archive_size: u64,

    /// Archive retention days (delete files older than this)
    #[serde(default = "default_archive_retention_days")]
    pub archive_retention_days: u32,
}

/// Queue configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    /// Maximum queue size (number of files)
    #[serde(default = "default_max_queue_size")]
    pub max_queue_size: usize,

    /// Queue persistence file (for crash recovery)
    pub persistence_file: Option<PathBuf>,

    /// Priority queuing based on file age
    #[serde(default = "default_priority_by_age")]
    pub priority_by_age: bool,

    /// Priority queuing based on file size
    #[serde(default = "default_priority_by_size")]
    pub priority_by_size: bool,

    /// Batch processing size
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

/// Service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service name for logging
    #[serde(default = "default_service_name")]
    pub name: String,

    /// Graceful shutdown timeout in seconds
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout_seconds: u64,

    /// Health check interval in seconds
    #[serde(default = "default_health_check_interval")]
    pub health_check_interval_seconds: u64,

    /// Enable metrics collection
    #[serde(default = "default_enable_metrics")]
    pub enable_metrics: bool,

    /// Metrics export interval in seconds
    #[serde(default = "default_metrics_interval")]
    pub metrics_interval_seconds: u64,

    /// Enable automatic restart on critical errors
    #[serde(default = "default_auto_restart")]
    pub auto_restart: bool,

    /// Maximum restart attempts
    #[serde(default = "default_max_restart_attempts")]
    pub max_restart_attempts: u32,
}

// Default value functions
fn default_file_patterns() -> Vec<String> {
    vec!["*.mp3".to_string()]
}

fn default_file_extensions() -> Vec<String> {
    vec!["mp3".to_string()]
}

const fn default_min_file_size() -> u64 {
    1024 // 1KB minimum
}

const fn default_max_file_size() -> u64 {
    100_000_000 // 100MB maximum
}

const fn default_debounce_delay() -> u64 {
    1000 // 1 second
}

const fn default_recursive() -> bool {
    true
}

const fn default_follow_symlinks() -> bool {
    false
}

const fn default_processing_interval() -> u64 {
    5 // 5 seconds
}

fn default_processing_workers() -> usize {
    num_cpus::get().max(2)
}

const fn default_max_retry_attempts() -> u32 {
    3
}

const fn default_retry_delay() -> u64 {
    10 // 10 seconds base delay
}

const fn default_processing_timeout() -> u64 {
    300 // 5 minutes
}

const fn default_move_after_processing() -> bool {
    true
}

const fn default_delete_after_processing() -> bool {
    false
}

const fn default_verify_file_integrity() -> bool {
    true
}

const fn default_organize_by_date() -> bool {
    true
}

const fn default_organize_by_system() -> bool {
    true
}

const fn default_compress_archive() -> bool {
    false
}

const fn default_compression_level() -> u32 {
    6
}

const fn default_max_archive_size() -> u64 {
    10_000_000_000 // 10GB
}

const fn default_archive_retention_days() -> u32 {
    90 // 3 months
}

const fn default_max_queue_size() -> usize {
    10000
}

const fn default_priority_by_age() -> bool {
    true
}

const fn default_priority_by_size() -> bool {
    false
}

const fn default_batch_size() -> usize {
    10
}

fn default_service_name() -> String {
    "sdrtrunk-monitor".to_string()
}

const fn default_shutdown_timeout() -> u64 {
    30 // 30 seconds
}

const fn default_health_check_interval() -> u64 {
    60 // 1 minute
}

const fn default_enable_metrics() -> bool {
    true
}

const fn default_metrics_interval() -> u64 {
    300 // 5 minutes
}

const fn default_auto_restart() -> bool {
    true
}

const fn default_max_restart_attempts() -> u32 {
    5
}

impl WatchConfig {
    /// Get debounce delay as Duration
    #[must_use]
    pub const fn debounce_delay(&self) -> Duration {
        Duration::from_millis(self.debounce_delay_ms)
    }
}

impl ProcessingConfig {
    /// Get processing interval as Duration
    #[must_use]
    pub const fn processing_interval(&self) -> Duration {
        Duration::from_secs(self.processing_interval_seconds)
    }

    /// Get retry delay as Duration
    #[must_use]
    pub const fn retry_delay(&self) -> Duration {
        Duration::from_secs(self.retry_delay_seconds)
    }

    /// Get processing timeout as Duration
    #[must_use]
    pub const fn processing_timeout(&self) -> Duration {
        Duration::from_secs(self.processing_timeout_seconds)
    }
}

impl ServiceConfig {
    /// Get shutdown timeout as Duration
    #[must_use]
    pub const fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.shutdown_timeout_seconds)
    }

    /// Get health check interval as Duration
    #[must_use]
    pub const fn health_check_interval(&self) -> Duration {
        Duration::from_secs(self.health_check_interval_seconds)
    }

    /// Get metrics interval as Duration
    #[must_use]
    pub const fn metrics_interval(&self) -> Duration {
        Duration::from_secs(self.metrics_interval_seconds)
    }
}

impl MonitorConfig {
    /// Load configuration from environment and files
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError::Configuration`] if:
    /// - Configuration files contain invalid TOML/JSON syntax
    /// - Required configuration values are missing
    /// - Configuration values are out of valid ranges
    /// - Environment variables have invalid values
    pub fn load() -> crate::Result<Self> {
        let config = config::Config::builder()
            .add_source(config::File::with_name("monitor").required(false))
            .add_source(config::File::with_name("config").required(false))
            .add_source(config::Environment::with_prefix("SDRTRUNK_MONITOR").separator("_"))
            .build()
            .map_err(|e| crate::MonitorError::configuration(e.to_string()))?;

        config
            .try_deserialize()
            .map_err(|e| crate::MonitorError::configuration(e.to_string()))
    }
}

impl Default for MonitorConfig {
    fn default() -> Self {
        let home_dir = directories::UserDirs::new()
            .map_or_else(|| PathBuf::from("."), |dirs| dirs.home_dir().to_path_buf());

        let data_dir = home_dir.join(".sdrtrunk-monitor");

        Self {
            watch: WatchConfig {
                watch_directory: data_dir.join("watch"),
                file_patterns: default_file_patterns(),
                file_extensions: default_file_extensions(),
                min_file_size: default_min_file_size(),
                max_file_size: default_max_file_size(),
                debounce_delay_ms: default_debounce_delay(),
                recursive: default_recursive(),
                follow_symlinks: default_follow_symlinks(),
            },
            processing: ProcessingConfig {
                processing_interval_seconds: default_processing_interval(),
                processing_workers: default_processing_workers(),
                max_retry_attempts: default_max_retry_attempts(),
                retry_delay_seconds: default_retry_delay(),
                processing_timeout_seconds: default_processing_timeout(),
                move_after_processing: default_move_after_processing(),
                delete_after_processing: default_delete_after_processing(),
                verify_file_integrity: default_verify_file_integrity(),
            },
            storage: StorageConfig {
                archive_directory: data_dir.join("archive"),
                failed_directory: data_dir.join("failed"),
                temp_directory: data_dir.join("temp"),
                organize_by_date: default_organize_by_date(),
                organize_by_system: default_organize_by_system(),
                compress_archive: default_compress_archive(),
                compression_level: default_compression_level(),
                max_archive_size: default_max_archive_size(),
                archive_retention_days: default_archive_retention_days(),
            },
            database: sdrtrunk_core::config::DatabaseConfig {
                url: "postgresql://localhost/sdrtrunk".to_string(),
                max_connections: 50,
                min_connections: 5,
                connect_timeout: 30,
                idle_timeout: 600,
            },
            queue: QueueConfig {
                max_queue_size: default_max_queue_size(),
                persistence_file: Some(data_dir.join("queue.json")),
                priority_by_age: default_priority_by_age(),
                priority_by_size: default_priority_by_size(),
                batch_size: default_batch_size(),
            },
            service: ServiceConfig {
                name: default_service_name(),
                shutdown_timeout_seconds: default_shutdown_timeout(),
                health_check_interval_seconds: default_health_check_interval(),
                enable_metrics: default_enable_metrics(),
                metrics_interval_seconds: default_metrics_interval(),
                auto_restart: default_auto_restart(),
                max_restart_attempts: default_max_restart_attempts(),
            },
        }
    }
}
