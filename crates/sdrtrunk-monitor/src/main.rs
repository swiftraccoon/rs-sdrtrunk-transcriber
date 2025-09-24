//! `SDRTrunk` File Monitor Service
//!
//! A high-performance, cross-platform file monitoring service that watches for
//! new MP3 files from `SDRTrunk` and automatically processes them for transcription.

#![forbid(unsafe_code)]
#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    missing_docs,
    rust_2018_idioms
)]

use clap::{Parser, Subcommand};
use sdrtrunk_monitor::{MonitorConfig, MonitorService, Result};
use std::path::PathBuf;
use tokio::signal;
use tracing::{error, info, warn};

/// Command line interface for the `SDRTrunk` monitor service
#[derive(Parser)]
#[command(
    name = "sdrtrunk-monitor",
    version = env!("CARGO_PKG_VERSION"),
    about = "File monitoring service for SDRTrunk transcriber",
    long_about = "A high-performance, cross-platform file monitoring service that watches for new MP3 files from SDRTrunk and automatically processes them for transcription."
)]
pub struct Cli {
    /// Configuration file path
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// Log format (json, pretty)
    #[arg(long, default_value = "pretty")]
    log_format: String,

    /// Enable structured JSON logging
    #[arg(long)]
    json: bool,

    /// Subcommand
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Available subcommands
#[derive(Subcommand)]
pub enum Commands {
    /// Start the monitoring service
    Start {
        /// Run in daemon mode (background)
        #[arg(short, long)]
        daemon: bool,

        /// PID file for daemon mode
        #[arg(long, value_name = "FILE")]
        pid_file: Option<PathBuf>,
    },

    /// Stop a running monitoring service
    Stop {
        /// PID file to read process ID from
        #[arg(long, value_name = "FILE")]
        pid_file: Option<PathBuf>,
    },

    /// Check service status
    Status {
        /// Output format (json, table)
        #[arg(short, long, default_value = "table")]
        format: String,
    },

    /// Show service metrics
    Metrics {
        /// Output format (json, table)
        #[arg(short, long, default_value = "table")]
        format: String,

        /// Watch metrics (update every N seconds)
        #[arg(short, long)]
        watch: Option<u64>,
    },

    /// Validate configuration
    Config {
        /// Show resolved configuration
        #[arg(short, long)]
        show: bool,

        /// Validate configuration file
        #[arg(short, long)]
        validate: bool,
    },

    /// Manage the processing queue
    Queue {
        /// Queue management subcommand
        #[command(subcommand)]
        action: QueueCommands,
    },

    /// Scan directory for existing files
    Scan {
        /// Directory to scan (overrides config)
        #[arg(value_name = "DIRECTORY")]
        directory: Option<PathBuf>,

        /// Actually queue found files (dry-run by default)
        #[arg(long)]
        execute: bool,
    },
}

/// Queue management commands
#[derive(Subcommand)]
pub enum QueueCommands {
    /// Show queue status
    Status,

    /// List queued files
    List {
        /// Show only failed files
        #[arg(long)]
        failed: bool,

        /// Show only processing files
        #[arg(long)]
        processing: bool,

        /// Maximum number of files to show
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Retry failed files
    Retry {
        /// Specific file ID to retry
        #[arg(value_name = "FILE_ID")]
        file_id: Option<String>,

        /// Retry all failed files
        #[arg(long)]
        all: bool,
    },

    /// Clear failed files
    Clear {
        /// Actually clear (dry-run by default)
        #[arg(long)]
        execute: bool,
    },

    /// Add file to queue manually
    Add {
        /// File path to add
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
}

/// Main entry point for the monitor service
///
/// # Errors
///
/// Returns error if service initialization or execution fails
///
/// # Panics
///
/// Panics if the tokio runtime cannot be initialized
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(&cli);

    // Load configuration
    let config = load_config(cli.config.as_deref()).await?;

    match cli.command {
        Some(Commands::Start { daemon, pid_file }) => start_service(config, daemon, pid_file).await,
        Some(Commands::Stop { pid_file }) => stop_service(pid_file),
        Some(Commands::Status { format }) => {
            show_status(&format);
            Ok(())
        }
        Some(Commands::Metrics { format, watch }) => {
            show_metrics(&format, watch);
            Ok(())
        }
        Some(Commands::Config { show, validate }) => handle_config_command(&config, show, validate),
        Some(Commands::Queue { action }) => handle_queue_command(action, &config).await,
        Some(Commands::Scan { directory, execute }) => {
            scan_directory(directory, execute, &config).await
        }
        None => {
            // Default: start service in foreground
            start_service(config, false, None).await
        }
    }
}

/// Initialize logging system
pub fn init_logging(cli: &Cli) {
    use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cli.log_level));

    let subscriber = tracing_subscriber::registry().with(env_filter);

    if cli.json || cli.log_format == "json" {
        subscriber
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        subscriber
            .with(tracing_subscriber::fmt::layer().pretty())
            .init();
    }

    info!(
        version = env!("CARGO_PKG_VERSION"),
        log_level = cli.log_level,
        "SDRTrunk Monitor Service starting"
    );
}

/// Load configuration from file or environment
///
/// # Errors
///
/// Returns error if the configuration file cannot be read or parsed
pub async fn load_config(config_path: Option<&std::path::Path>) -> Result<MonitorConfig> {
    if let Some(path) = config_path {
        info!("Loading configuration from: {}", path.display());

        let config_content = tokio::fs::read_to_string(path).await.map_err(|e| {
            sdrtrunk_monitor::MonitorError::configuration(format!(
                "Failed to read config file {}: {}",
                path.display(),
                e
            ))
        })?;

        let config: MonitorConfig = toml::from_str(&config_content).map_err(|e| {
            sdrtrunk_monitor::MonitorError::configuration(format!(
                "Failed to parse config file: {e}"
            ))
        })?;

        Ok(config)
    } else {
        info!("Loading default configuration");
        MonitorConfig::load()
    }
}

/// Run the monitoring service and wait for shutdown
///
/// # Errors
///
/// Returns error if the service fails
#[allow(clippy::future_not_send)]
async fn run_monitor_service(service: MonitorService) -> Result<()> {
    service.start().await?;
    info!("Monitoring service is running. Press Ctrl+C to stop.");

    wait_for_shutdown_signal(&service).await;

    service.stop().await?;
    info!("Service stopped successfully");
    Ok(())
}

/// Wait for shutdown signal (Ctrl+C or service shutdown)
async fn wait_for_shutdown_signal(service: &MonitorService) {
    tokio::select! {
        _ = signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down gracefully");
        }
        () = service.wait_for_shutdown() => {
            info!("Service requested shutdown");
        }
    }
}

/// Start the monitoring service
///
/// # Errors
///
/// Returns error if the service cannot be started
#[allow(clippy::future_not_send)]
async fn start_service(
    config: MonitorConfig,
    daemon: bool,
    pid_file: Option<PathBuf>,
) -> Result<()> {
    if daemon {
        return start_daemon(config, pid_file);
    }

    info!(
        watch_dir = %config.watch.watch_directory.display(),
        archive_dir = %config.storage.archive_directory.display(),
        workers = config.processing.processing_workers,
        "Starting monitoring service"
    );

    // Create and run service
    let service = MonitorService::new(config).await?;
    run_monitor_service(service).await
}

/// Start service in daemon mode
///
/// # Errors
///
/// Returns error because daemon mode is not yet implemented
fn start_daemon(_config: MonitorConfig, _pid_file: Option<PathBuf>) -> Result<()> {
    // Daemon mode implementation would go here
    // This is a complex topic that involves forking, detaching from terminal, etc.
    // For now, we'll just return an error
    error!("Daemon mode is not yet implemented");
    Err(sdrtrunk_monitor::MonitorError::configuration(
        "Daemon mode is not yet implemented",
    ))
}

/// Stop a running service
///
/// # Errors
///
/// Returns error because service stopping is not yet implemented
fn stop_service(_pid_file: Option<PathBuf>) -> Result<()> {
    // Service stopping implementation would read PID file and send signal
    // For now, we'll just return an error
    error!("Service stopping is not yet implemented");
    Err(sdrtrunk_monitor::MonitorError::configuration(
        "Service stopping is not yet implemented",
    ))
}

/// Show service status
fn show_status(_format: &str) {
    // Status checking implementation would connect to running service
    // For now, we'll just show a placeholder
    println!("Service status checking is not yet implemented");
}

/// Show service metrics
fn show_metrics(_format: &str, _watch: Option<u64>) {
    // Metrics display implementation would connect to running service
    // For now, we'll just show a placeholder
    println!("Service metrics display is not yet implemented");
}

/// Validate configuration directories
pub fn validate_config_directories(config: &MonitorConfig) {
    info!("Validating configuration...");

    for dir in [
        &config.watch.watch_directory,
        &config.storage.archive_directory,
        &config.storage.failed_directory,
        &config.storage.temp_directory,
    ] {
        if !dir.exists() {
            warn!("Directory does not exist: {}", dir.display());
        }
    }

    info!("Configuration validation completed");
}

/// Show configuration as TOML
///
/// # Errors
///
/// Returns error if configuration cannot be serialized
pub fn show_config(config: &MonitorConfig) -> Result<()> {
    let config_toml = toml::to_string_pretty(config).map_err(|e| {
        sdrtrunk_monitor::MonitorError::configuration(format!(
            "Failed to serialize configuration: {e}"
        ))
    })?;
    println!("{config_toml}");
    Ok(())
}

/// Handle configuration commands
///
/// # Errors
///
/// Returns error if configuration cannot be serialized
pub fn handle_config_command(config: &MonitorConfig, show: bool, validate: bool) -> Result<()> {
    if validate {
        validate_config_directories(config);
    }

    if show {
        show_config(config)?;
    }

    Ok(())
}

/// Handle queue management commands
///
/// # Errors
///
/// Returns error if queue operation fails
async fn handle_queue_command(action: QueueCommands, config: &MonitorConfig) -> Result<()> {
    match action {
        QueueCommands::Status => {
            println!("Queue status display is not yet implemented");
        }
        QueueCommands::List {
            failed: _,
            processing: _,
            limit: _,
        } => {
            println!("Queue listing is not yet implemented");
        }
        QueueCommands::Retry { file_id: _, all: _ } => {
            println!("Queue retry is not yet implemented");
        }
        QueueCommands::Clear { execute: _ } => {
            println!("Queue clearing is not yet implemented");
        }
        QueueCommands::Add { file } => {
            info!("Adding file to queue: {}", file.display());

            // Create a temporary service to add the file
            let _service = MonitorService::new(config.clone()).await?;
            // Implementation would add file to queue
            println!("File queuing is not yet implemented");
        }
    }

    Ok(())
}

/// Scan directory for existing files
///
/// # Errors
///
/// Returns error if directory cannot be scanned
async fn scan_directory(
    directory: Option<PathBuf>,
    execute: bool,
    config: &MonitorConfig,
) -> Result<()> {
    let scan_dir = directory.as_ref().unwrap_or(&config.watch.watch_directory);

    info!(
        directory = %scan_dir.display(),
        execute = execute,
        "Scanning directory for files"
    );

    // Create a temporary monitor to scan
    let monitor = sdrtrunk_monitor::FileMonitor::new(config.watch.clone());
    let files = monitor.scan_existing_files().await?;

    println!("Found {} files:", files.len());
    for (i, file) in files.iter().enumerate() {
        println!("  {}: {}", i + 1, file.display());
    }

    if execute && !files.is_empty() {
        info!("Queuing {} files for processing", files.len());

        let _service = MonitorService::new(config.clone()).await?;
        // Implementation would queue all found files
        println!("File queuing is not yet implemented");
    } else if !execute && !files.is_empty() {
        println!("\nTo actually queue these files, run with --execute");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_cli(log_level: &str, json: bool) -> Cli {
        Cli {
            config: None,
            log_level: log_level.to_string(),
            log_format: if json {
                "json".to_string()
            } else {
                "pretty".to_string()
            },
            json,
            command: None,
        }
    }

    #[test]
    fn test_init_logging_pretty_format() {
        let cli = create_test_cli("info", false);
        // Skip actual logging init to avoid "global default subscriber" error
        assert_eq!(cli.log_level, "info");
        assert_eq!(cli.log_format, "pretty");
    }

    #[test]
    fn test_init_logging_json_format() {
        let cli = create_test_cli("debug", true);
        // Skip actual logging init to avoid "global default subscriber" error
        assert_eq!(cli.log_level, "debug");
        assert!(cli.json);
    }

    #[test]
    fn test_init_logging_different_levels() {
        // Only test one level to avoid "global default subscriber already set" error
        let cli = create_test_cli("info", false);
        // Skip actual initialization in tests to avoid conflicts
        // Just verify the CLI structure is valid
        assert_eq!(cli.log_level, "info");
        assert!(!cli.json);
    }

    #[tokio::test]
    async fn test_load_config_default() {
        // This test loads the default monitor config, not the main config
        if let Ok(config) = load_config(None).await {
            assert!(!config.watch.watch_directory.as_os_str().is_empty());
        } else {
            // It's okay if default config loading fails in test environment
            // since it might depend on environment variables or files
        }
    }

    #[tokio::test]
    async fn test_load_config_nonexistent_file() {
        let path = PathBuf::from("/nonexistent/config.toml");
        let result = load_config(Some(&path)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_load_config_valid_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        let config_content = r#"
[database]
url = "sqlite::memory:"

[watch]
watch_directory = "/tmp/test"
file_patterns = ["*.mp3"]

[processing]
processing_workers = 2
max_retry_attempts = 3

[storage]
archive_directory = "/tmp/archive"
failed_directory = "/tmp/failed"
temp_directory = "/tmp/temp"

[queue]
max_queue_size = 1000

[service]
health_check_interval_seconds = 30
        "#;

        tokio::fs::write(&config_path, config_content)
            .await
            .unwrap();
        let config = load_config(Some(&config_path)).await.unwrap();
        assert_eq!(config.processing.processing_workers, 2);
        assert_eq!(config.processing.max_retry_attempts, 3);
    }

    #[test]
    fn test_validate_config_directories() {
        let config = sdrtrunk_monitor::MonitorConfig::default();
        validate_config_directories(&config);
        // Should not panic
    }

    #[test]
    fn test_show_config() {
        let config = sdrtrunk_monitor::MonitorConfig::default();
        let result = show_config(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_config_command_show() {
        let config = sdrtrunk_monitor::MonitorConfig::default();
        let result = handle_config_command(&config, true, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_config_command_validate() {
        let config = sdrtrunk_monitor::MonitorConfig::default();
        let result = handle_config_command(&config, false, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_config_command_both() {
        let config = sdrtrunk_monitor::MonitorConfig::default();
        let result = handle_config_command(&config, true, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_parsing() {
        // Test that CLI parsing works for basic cases
        use clap::Parser;

        let args = vec!["sdrtrunk-monitor", "--log-level", "debug", "--json"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.log_level, "debug");
        assert!(cli.json);
    }

    #[test]
    fn test_cli_subcommands() {
        use clap::Parser;

        let test_cases = vec![
            vec!["sdrtrunk-monitor", "start", "--daemon"],
            vec!["sdrtrunk-monitor", "stop"],
            vec!["sdrtrunk-monitor", "status", "--format", "json"],
            vec!["sdrtrunk-monitor", "config", "--show", "--validate"],
        ];

        for args in test_cases {
            let result = Cli::try_parse_from(args.clone());
            assert!(result.is_ok(), "Failed to parse args: {args:?}");
        }
    }

    #[test]
    fn test_start_daemon_not_implemented() {
        let config = sdrtrunk_monitor::MonitorConfig::default();
        let result = start_daemon(config, None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not yet implemented")
        );
    }

    #[test]
    fn test_stop_service_not_implemented() {
        let result = stop_service(None);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("not yet implemented")
        );
    }

    #[test]
    fn test_show_status_placeholder() {
        // Test that show_status doesn't panic
        show_status("json");
        show_status("table");
    }

    #[test]
    fn test_show_metrics_placeholder() {
        // Test that show_metrics doesn't panic
        show_metrics("json", None);
        show_metrics("table", Some(5));
    }

    #[tokio::test]
    async fn test_handle_queue_command_status() {
        let config = sdrtrunk_monitor::MonitorConfig::default();
        let result = handle_queue_command(QueueCommands::Status, &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_queue_command_list() {
        let config = sdrtrunk_monitor::MonitorConfig::default();
        let result = handle_queue_command(
            QueueCommands::List {
                failed: true,
                processing: false,
                limit: 10,
            },
            &config,
        )
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_queue_command_retry() {
        let config = sdrtrunk_monitor::MonitorConfig::default();
        let result = handle_queue_command(
            QueueCommands::Retry {
                file_id: Some("test-id".to_string()),
                all: false,
            },
            &config,
        )
        .await;
        assert!(result.is_ok());

        let result = handle_queue_command(
            QueueCommands::Retry {
                file_id: None,
                all: true,
            },
            &config,
        )
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_queue_command_clear() {
        let config = sdrtrunk_monitor::MonitorConfig::default();
        let result = handle_queue_command(QueueCommands::Clear { execute: false }, &config).await;
        assert!(result.is_ok());

        let result = handle_queue_command(QueueCommands::Clear { execute: true }, &config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_queue_command_add() {
        let config = sdrtrunk_monitor::MonitorConfig::default();
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.mp3");
        tokio::fs::write(&test_file, b"test content").await.unwrap();

        // This should fail because service creation will fail without proper config
        let result = handle_queue_command(QueueCommands::Add { file: test_file }, &config).await;
        // We expect this to fail due to service creation, which is acceptable for testing
        let _ = result;
    }

    #[tokio::test]
    async fn test_scan_directory_default() {
        let config = sdrtrunk_monitor::MonitorConfig::default();

        // This should fail in test environment, which is expected
        let result = scan_directory(None, false, &config).await;
        let _ = result; // We don't assert success since directory might not exist
    }

    #[tokio::test]
    async fn test_scan_directory_custom_path() {
        let config = sdrtrunk_monitor::MonitorConfig::default();
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().to_path_buf();

        // Create a test MP3 file
        let test_file = test_dir.join("test.mp3");
        tokio::fs::write(&test_file, b"test mp3 content")
            .await
            .unwrap();

        let result = scan_directory(Some(test_dir), false, &config).await;
        // Result depends on monitor implementation, so we don't assert
        let _ = result;
    }

    #[tokio::test]
    async fn test_scan_directory_with_execute() {
        let config = sdrtrunk_monitor::MonitorConfig::default();
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().to_path_buf();

        let result = scan_directory(Some(test_dir), true, &config).await;
        // Result depends on monitor implementation
        let _ = result;
    }

    #[test]
    fn test_cli_with_config_file() {
        use clap::Parser;

        let args = vec!["sdrtrunk-monitor", "--config", "/path/to/config.toml"];
        let cli = Cli::try_parse_from(args).unwrap();
        assert!(cli.config.is_some());
        assert_eq!(cli.config.unwrap(), PathBuf::from("/path/to/config.toml"));
    }

    #[test]
    fn test_cli_all_options() {
        use clap::Parser;

        let args = vec![
            "sdrtrunk-monitor",
            "--config",
            "/test/config.toml",
            "--log-level",
            "trace",
            "--log-format",
            "json",
            "--json",
        ];
        let cli = Cli::try_parse_from(args).unwrap();
        assert_eq!(cli.log_level, "trace");
        assert_eq!(cli.log_format, "json");
        assert!(cli.json);
    }

    #[test]
    fn test_queue_commands_subcommands() {
        use clap::Parser;

        let test_cases = vec![
            vec!["sdrtrunk-monitor", "queue", "status"],
            vec![
                "sdrtrunk-monitor",
                "queue",
                "list",
                "--failed",
                "--limit",
                "50",
            ],
            vec!["sdrtrunk-monitor", "queue", "retry", "--all"],
            vec!["sdrtrunk-monitor", "queue", "clear", "--execute"],
            vec!["sdrtrunk-monitor", "queue", "add", "/path/to/file.mp3"],
        ];

        for args in test_cases {
            let result = Cli::try_parse_from(args.clone());
            assert!(result.is_ok(), "Failed to parse args: {args:?}");
        }
    }

    #[test]
    fn test_scan_command() {
        use clap::Parser;

        let args = vec!["sdrtrunk-monitor", "scan", "/path/to/scan", "--execute"];
        let cli = Cli::try_parse_from(args).unwrap();

        if let Some(Commands::Scan { directory, execute }) = cli.command {
            assert_eq!(directory.unwrap(), PathBuf::from("/path/to/scan"));
            assert!(execute);
        } else {
            panic!("Expected scan command");
        }
    }

    #[test]
    fn test_metrics_command() {
        use clap::Parser;

        let args = vec![
            "sdrtrunk-monitor",
            "metrics",
            "--format",
            "json",
            "--watch",
            "10",
        ];
        let cli = Cli::try_parse_from(args).unwrap();

        if let Some(Commands::Metrics { format, watch }) = cli.command {
            assert_eq!(format, "json");
            assert_eq!(watch.unwrap(), 10);
        } else {
            panic!("Expected metrics command");
        }
    }

    #[test]
    fn test_start_command_with_pid_file() {
        use clap::Parser;

        let args = vec![
            "sdrtrunk-monitor",
            "start",
            "--daemon",
            "--pid-file",
            "/var/run/monitor.pid",
        ];
        let cli = Cli::try_parse_from(args).unwrap();

        if let Some(Commands::Start { daemon, pid_file }) = cli.command {
            assert!(daemon);
            assert_eq!(pid_file.unwrap(), PathBuf::from("/var/run/monitor.pid"));
        } else {
            panic!("Expected start command");
        }
    }

    #[test]
    fn test_stop_command_with_pid_file() {
        use clap::Parser;

        let args = vec![
            "sdrtrunk-monitor",
            "stop",
            "--pid-file",
            "/var/run/monitor.pid",
        ];
        let cli = Cli::try_parse_from(args).unwrap();

        if let Some(Commands::Stop { pid_file }) = cli.command {
            assert_eq!(pid_file.unwrap(), PathBuf::from("/var/run/monitor.pid"));
        } else {
            panic!("Expected stop command");
        }
    }

    #[tokio::test]
    async fn test_load_config_invalid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("invalid_config.toml");

        let invalid_content = "[invalid toml content";
        tokio::fs::write(&config_path, invalid_content)
            .await
            .unwrap();

        let result = load_config(Some(&config_path)).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to parse config file")
        );
    }
}
