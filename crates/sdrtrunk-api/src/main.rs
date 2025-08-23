//! Main entry point for the `SDRTrunk` API server

use sdrtrunk_api::build_router;
use sdrtrunk_core::{Config, context_error, context_error::Result, init_logging};
use sdrtrunk_database::Database;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if it exists (for development convenience)
    if let Err(e) = dotenvy::dotenv() {
        // It's okay if .env doesn't exist, just log it at debug level
        eprintln!("Note: .env file not loaded: {e}");
    }

    // Initialize logging first
    init_logging()?;

    // Load configuration
    let config = Config::load().unwrap_or_else(|err| {
        info!("Failed to load config ({}), using defaults", err);
        Config::default()
    });

    info!("╔══════════════════════════════════════════════════════════╗");
    info!(
        "║       SDRTrunk Transcriber API Server v{}             ║",
        env!("CARGO_PKG_VERSION")
    );
    info!("╚══════════════════════════════════════════════════════════╝");
    info!(
        "🚀 Starting server on {}:{}",
        config.server.host, config.server.port
    );

    // Initialize database connection
    info!("🔌 Connecting to database...");
    let database = match Database::new(&config).await {
        Ok(db) => {
            info!("✅ Database connection established");
            db
        }
        Err(e) => {
            error!("Failed to connect to database: {}", e);
            return Err(context_error!("Database connection failed: {}", e));
        }
    };

    // Run database migrations
    info!("🔄 Running database migrations...");
    if let Err(e) = database.migrate().await {
        error!("Database migration failed: {}", e);
        return Err(context_error!("Migration failed: {}", e));
    }
    info!("✅ Database migrations completed");

    // Perform database health check
    if let Err(e) = database.health_check().await {
        error!("Database health check failed: {}", e);
        return Err(context_error!("Database health check failed: {}", e));
    }
    info!("✅ Database health check passed");

    // Build the application router
    info!("🛠️  Building application routes...");
    let app = build_router(config.clone(), database.pool().clone())?
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

    // Create server address
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .map_err(|e| context_error!("Invalid server address: {}", e))?;

    // Create TCP listener
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| context_error!("Failed to bind to {}: {}", addr, e))?;

    info!("╔══════════════════════════════════════════════════════════╗");
    info!("║                     SERVER READY                         ║");
    info!("╟──────────────────────────────────────────────────────────╢");
    info!("║ 🌐 API:     http://{:12}", addr);
    info!("║ 💚 Health:  http://{:12}/health", addr);
    info!("║ 📚 Docs:    http://{:12}/api/docs", addr);
    info!("╚══════════════════════════════════════════════════════════╝\n");

    // Start the server with graceful shutdown
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .map_err(|e| context_error!("Server error: {}", e))?;

    info!("👋 Server shutdown complete");
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
        () = ctrl_c => {
            info!("Received Ctrl+C, shutting down gracefully...");
        },
        () = terminate => {
            info!("Received terminate signal, shutting down gracefully...");
        },
    }
}
