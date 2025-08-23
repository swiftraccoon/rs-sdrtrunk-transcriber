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
struct Cli {
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
enum Commands {
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
enum QueueCommands {
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
    // Load .env file if it exists (for development convenience)
    if let Err(e) = dotenvy::dotenv() {
        // It's okay if .env doesn't exist
        eprintln!("Note: .env file not loaded: {e}");
    }

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
fn init_logging(cli: &Cli) {
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
async fn load_config(config_path: Option<&std::path::Path>) -> Result<MonitorConfig> {
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
fn validate_config_directories(config: &MonitorConfig) {
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
fn show_config(config: &MonitorConfig) -> Result<()> {
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
fn handle_config_command(config: &MonitorConfig, show: bool, validate: bool) -> Result<()> {
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
