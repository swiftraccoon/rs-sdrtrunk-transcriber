//! Main entry point for the `SDRTrunk` API server

#![forbid(unsafe_code)]

use anyhow::{Result, anyhow};
use sdrtrunk_api::build_router;
use sdrtrunk_protocol::Config;
use sdrtrunk_storage::Database;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{error, info};

/// Load environment configuration
///
/// # Errors
///
/// Returns error if logging initialization fails
pub fn load_environment() -> Result<()> {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(
            fmt::layer()
                .with_target(false)
                .with_thread_ids(false)
                .with_thread_names(false)
                .with_file(false)
                .with_line_number(false)
                .with_level(true)
                .compact(),
        )
        .init();
    Ok(())
}

/// Load configuration from environment variables and config files.
///
/// # Errors
///
/// Returns an error if configuration cannot be loaded or parsed.
fn load_config() -> Result<Config> {
    let cfg = config::Config::builder()
        .add_source(config::File::with_name("config").required(false))
        .add_source(config::Environment::with_prefix("SDRTRUNK").separator("_"))
        .build()
        .map_err(|e| anyhow!("Configuration load error: {e}"))?;
    cfg.try_deserialize()
        .map_err(|e| anyhow!("Configuration deserialization error: {e}"))
}

/// Load and validate configuration
#[must_use]
pub fn load_and_validate_config() -> Config {
    load_config().unwrap_or_else(|err| {
        info!("Failed to load config ({}), using defaults", err);
        Config::default()
    })
}

/// Print startup banner
#[allow(clippy::cognitive_complexity)]
pub fn print_startup_banner(config: &Config) {
    info!("╔══════════════════════════════════════════════════════════╗");
    info!(
        "║       SDRTrunk Transcriber API Server v{}             ║",
        env!("CARGO_PKG_VERSION")
    );
    info!("╚══════════════════════════════════════════════════════════╝");
    info!(
        "Starting server on {}:{}",
        config.server.host, config.server.port
    );
}

/// Initialize database with migrations and health check
///
/// # Errors
///
/// Returns error if database connection, migration, or health check fails
#[allow(clippy::cognitive_complexity)]
pub async fn initialize_database(config: &Config) -> Result<Database> {
    // Initialize database connection
    info!("Connecting to database...");
    let database = Database::new(config).await.map_err(|e| {
        error!("Failed to connect to database: {}", e);
        anyhow!("Database connection failed: {e}")
    })?;

    info!("Database connection established");

    // Initialize schema (creates tables if they don't exist)
    info!("Initializing database schema...");
    database.init_schema().await.map_err(|e| {
        error!("Schema initialization failed: {}", e);
        anyhow!("Schema init failed: {e}")
    })?;

    info!("Database schema ready");

    // Perform database health check
    database.health_check().await.map_err(|e| {
        error!("Database health check failed: {}", e);
        anyhow!("Database health check failed: {e}")
    })?;

    info!("Database health check passed");
    Ok(database)
}

/// Create server address from configuration
///
/// # Errors
///
/// Returns error if address format is invalid
pub fn create_server_address(config: &Config) -> Result<SocketAddr> {
    format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .map_err(|e| anyhow!("Invalid server address: {e}"))
}

/// Print server ready banner
#[allow(clippy::cognitive_complexity)]
pub fn print_ready_banner(addr: SocketAddr) {
    info!("╔══════════════════════════════════════════════════════════╗");
    info!("║                     SERVER READY                         ║");
    info!("╟──────────────────────────────────────────────────────────╢");
    info!("║ 🌐 API:     http://{:12}", addr);
    info!("║ Health:  http://{:12}/health", addr);
    info!("║ Docs:    http://{:12}/api/docs", addr);
    info!("╚══════════════════════════════════════════════════════════╝\n");
}

#[tokio::main]
#[allow(
    clippy::cognitive_complexity,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]
async fn main() -> Result<()> {
    load_environment()?;
    let config = load_and_validate_config();
    print_startup_banner(&config);
    let database = initialize_database(&config).await?;

    // Build the application router
    info!("Building application routes...");
    let app = build_router(config.clone(), database.pool().clone())
        .await?
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

    let addr = create_server_address(&config)?;

    // Create TCP listener
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow!("Failed to bind to {addr}: {e}"))?;

    print_ready_banner(addr);

    // Start the server with graceful shutdown
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .map_err(|e| anyhow!("Server error: {e}"))?;

    info!("Server shutdown complete");
    Ok(())
}

/// Handle graceful shutdown signals
///
/// # Panics
///
/// Panics if signal handlers cannot be installed.
#[allow(clippy::expect_used)]
pub async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        let _ = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            info!("Received Ctrl+C, shutting down gracefully...");
        },
        () = terminate => {
            info!("Received terminate signal, shutting down gracefully...");
        },
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::redundant_clone,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::needless_pass_by_value,
    clippy::uninlined_format_args,
    unused_qualifications,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::items_after_statements,
    clippy::float_cmp,
    clippy::redundant_closure_for_method_calls,
    clippy::fn_params_excessive_bools,
    clippy::similar_names,
    clippy::map_unwrap_or,
    clippy::unused_async,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::manual_string_new,
    clippy::no_effect_underscore_binding,
    clippy::option_if_let_else,
    clippy::single_char_pattern,
    clippy::ip_constant,
    clippy::or_fun_call,
    clippy::cast_lossless,
    clippy::needless_collect,
    clippy::single_match_else,
    clippy::needless_raw_string_hashes,
    clippy::match_same_arms
)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_load_and_validate_config() {
        let config = load_and_validate_config();
        assert!(!config.server.host.is_empty());
        assert!(config.server.port > 0);
    }

    #[test]
    fn test_print_startup_banner() {
        let config = Config::default();
        print_startup_banner(&config);
        // Should not panic
    }

    #[test]
    fn test_create_server_address_valid() {
        let mut config = Config::default();
        config.server.host = "127.0.0.1".to_string();
        config.server.port = 8080;

        let addr = create_server_address(&config).unwrap();
        assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!(addr.port(), 8080);
    }

    #[test]
    fn test_create_server_address_invalid_host() {
        let mut config = Config::default();
        config.server.host = "invalid-host-format".to_string();
        config.server.port = 8080;

        let result = create_server_address(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_server_address_ipv6() {
        let mut config = Config::default();
        config.server.host = "[::1]".to_string(); // IPv6 addresses need brackets in socket format
        config.server.port = 9000;

        let addr = create_server_address(&config).unwrap();
        assert_eq!(addr.port(), 9000);
    }

    #[test]
    fn test_print_ready_banner() {
        let addr = "127.0.0.1:8080".parse().unwrap();
        print_ready_banner(addr);
        // Should not panic
    }

    #[test]
    fn test_load_environment_success() {
        // This will try to load .env but shouldn't fail if it doesn't exist
        let result = load_environment();
        // Should succeed even if no .env file
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_variations() {
        // Test with default config
        let config = Config::default();
        assert!(create_server_address(&config).is_ok());

        // Test with custom host/port
        let mut config = Config::default();
        config.server.host = "0.0.0.0".to_string();
        config.server.port = 3000;

        let addr = create_server_address(&config).unwrap();
        assert_eq!(addr.port(), 3000);
    }

    #[test]
    fn test_server_address_edge_cases() {
        // Test port boundaries with valid IP
        let mut config = Config::default();
        config.server.host = "127.0.0.1".to_string();
        config.server.port = 1;

        let addr = create_server_address(&config).unwrap();
        assert_eq!(addr.port(), 1);

        config.server.port = 65535;
        let addr = create_server_address(&config).unwrap();
        assert_eq!(addr.port(), 65535);
    }
}
