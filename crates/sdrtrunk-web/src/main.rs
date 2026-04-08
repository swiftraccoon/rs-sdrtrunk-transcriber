//! Web server for `SDRTrunk` transcriber interface
#![forbid(unsafe_code)]
#![allow(clippy::type_complexity)]

use sdrtrunk_protocol::Config;
use sdrtrunk_web::build_app;
use std::net::{IpAddr, SocketAddr};
use tracing::{info, warn};

/// Load configuration from environment variables and config files.
///
/// # Errors
///
/// Returns an error if configuration cannot be loaded or parsed.
fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let cfg = config::Config::builder()
        .add_source(config::File::with_name("config").required(false))
        .add_source(config::Environment::with_prefix("SDRTRUNK").separator("_"))
        .build()?;

    Ok(cfg.try_deserialize()?)
}

#[tokio::main]
#[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Get configuration
    let config = load_config().unwrap_or_else(|e| {
        warn!("Failed to load config: {}, using defaults", e);
        Config::default()
    });

    // Build the application with configuration
    let app = build_app(config.clone());

    // Use configuration for web server address
    let host: IpAddr = config
        .webserver
        .host
        .parse()
        .map_err(|e| format!("Invalid web server host '{}': {}", config.webserver.host, e))?;
    let addr = SocketAddr::new(host, config.webserver.port);

    info!("Starting SDRTrunk web server on {}", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}
