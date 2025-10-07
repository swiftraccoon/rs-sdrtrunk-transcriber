//! Web server for SDRTrunk transcriber interface
#![forbid(unsafe_code)]

use sdrtrunk_web::build_app;
use std::net::{IpAddr, SocketAddr};
use tracing::{info, warn};

#[tokio::main]
#[allow(clippy::type_complexity)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Get configuration
    let config = sdrtrunk_core::Config::load().unwrap_or_else(|e| {
        warn!("Failed to load config: {}, using defaults", e);
        sdrtrunk_core::Config::default()
    });

    // Build the application with configuration
    let app = build_app(config.clone());

    // Use configuration for web server address
    let host: IpAddr = config.webserver.host.parse()
        .map_err(|e| format!("Invalid web server host '{}': {}", config.webserver.host, e))?;
    let addr = SocketAddr::new(host, config.webserver.port);

    info!("Starting SDRTrunk web server on {}", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}