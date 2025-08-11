//! Main entry point for the SDRTrunk API server

use sdrtrunk_api::build_router;
use sdrtrunk_core::{Config, init_logging, context_error::Result, context_error};
use sdrtrunk_database::Database;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging first
    init_logging()?;

    // Load configuration
    let config = Config::load().unwrap_or_else(|err| {
        info!("Failed to load config ({}), using defaults", err);
        Config::default()
    });

    info!(
        "Starting SDRTrunk API Server on {}:{}",
        config.server.host, config.server.port
    );

    // Initialize database connection
    info!("Connecting to database...");
    let database = match Database::new(&config).await {
        Ok(db) => {
            info!("Database connection established");
            db
        }
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            return Err(context_error!("Database connection failed: {}", e));
        }
    };

    // Run database migrations
    info!("Running database migrations...");
    if let Err(e) = database.migrate().await {
        error!("Database migration failed: {}", e);
        return Err(context_error!("Migration failed: {}", e));
    }
    info!("Database migrations completed");

    // Perform database health check
    if let Err(e) = database.health_check().await {
        error!("Database health check failed: {}", e);
        return Err(context_error!("Database health check failed: {}", e));
    }
    info!("Database health check passed");

    // Build the application router
    info!("Building application routes...");
    let app = build_router(config.clone(), database.pool().clone())
        .await?
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

    // Create server address
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .map_err(|e| context_error!("Invalid server address: {}", e))?;

    // Create TCP listener
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| context_error!("Failed to bind to {}: {}", addr, e))?;

    info!("Server listening on http://{}", addr);
    info!("Health check available at: http://{}/health", addr);
    info!("API documentation at: http://{}/api/docs", addr);

    // Start the server with graceful shutdown
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .map_err(|e| context_error!("Server error: {}", e))?;

    info!("Server shutdown complete");
    Ok(())
}

/// Handle graceful shutdown signals
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down gracefully...");
        },
        _ = terminate => {
            info!("Received terminate signal, shutting down gracefully...");
        },
    }
}
