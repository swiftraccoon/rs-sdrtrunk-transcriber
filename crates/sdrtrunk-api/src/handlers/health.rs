//! Health check endpoints for monitoring and diagnostics

use crate::state::AppState;
use axum::{extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Service status
    pub status: String,
    /// Service version
    pub version: String,
    /// Timestamp of the check
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Database connectivity status
    pub database: DatabaseHealth,
    /// System uptime in seconds
    pub uptime_seconds: u64,
}

/// Database health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHealth {
    /// Database connection status
    pub connected: bool,
    /// Connection pool statistics
    pub pool_stats: PoolStats,
    /// Response time in milliseconds
    pub response_time_ms: u64,
}

/// Connection pool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    /// Number of connections currently in use
    pub connections_in_use: u32,
    /// Maximum number of connections allowed
    pub max_connections: u32,
    /// Number of idle connections
    pub idle_connections: u32,
}

/// Readiness check response (simpler than health)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessResponse {
    /// Service readiness status
    pub ready: bool,
    /// Timestamp of the check
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Basic health check endpoint
///
/// Returns 200 OK if the service is running and database is accessible
pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> Result<Json<HealthResponse>, StatusCode> {
    let start_time = std::time::Instant::now();

    // Check database connectivity
    let database_health = match check_database_health(&state).await {
        Ok(health) => health,
        Err(e) => {
            error!("Database health check failed: {}", e);
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };

    let response_time = start_time.elapsed().as_millis() as u64;

    let health_response = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now(),
        database: database_health,
        uptime_seconds: get_uptime_seconds(),
    };

    info!("Health check completed in {}ms", response_time);
    Ok(Json(health_response))
}

/// Readiness check endpoint for Kubernetes-style health checks
///
/// Returns 200 OK if the service is ready to accept traffic
pub async fn readiness_check(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ReadinessResponse>, StatusCode> {
    // Simple database ping to verify readiness
    match sqlx::query("SELECT 1").fetch_one(&state.pool).await {
        Ok(_) => {
            let response = ReadinessResponse {
                ready: true,
                timestamp: chrono::Utc::now(),
            };
            Ok(Json(response))
        }
        Err(e) => {
            error!("Readiness check failed - database not accessible: {}", e);
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

/// Check database health and gather metrics
async fn check_database_health(state: &Arc<AppState>) -> Result<DatabaseHealth, sqlx::Error> {
    let start_time = std::time::Instant::now();

    // Perform a simple query to test connectivity
    sqlx::query("SELECT 1 as health_check")
        .fetch_one(&state.pool)
        .await?;

    let response_time_ms = start_time.elapsed().as_millis() as u64;

    // Get pool statistics
    let pool_stats = PoolStats {
        connections_in_use: state.pool.num_idle() as u32, // Note: SQLx doesn't expose in-use count directly
        max_connections: state.pool.options().get_max_connections(),
        idle_connections: state.pool.num_idle() as u32,
    };

    Ok(DatabaseHealth {
        connected: true,
        pool_stats,
        response_time_ms,
    })
}

/// Get system uptime in seconds (simplified - returns process uptime)
fn get_uptime_seconds() -> u64 {
    static START_TIME: std::sync::LazyLock<std::time::Instant> =
        std::sync::LazyLock::new(std::time::Instant::now);
    START_TIME.elapsed().as_secs()
}

/// Detailed health check endpoint with extended diagnostics
pub async fn detailed_health_check(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut health_data = serde_json::Map::new();

    // Basic health info
    health_data.insert(
        "service".to_string(),
        serde_json::json!({
            "name": "sdrtrunk-api",
            "version": env!("CARGO_PKG_VERSION"),
            "status": "healthy",
            "timestamp": chrono::Utc::now()
        }),
    );

    // Database health
    match check_database_health(&state).await {
        Ok(db_health) => {
            health_data.insert(
                "database".to_string(),
                serde_json::to_value(&db_health).unwrap(),
            );
        }
        Err(e) => {
            health_data.insert(
                "database".to_string(),
                serde_json::json!({
                    "connected": false,
                    "error": e.to_string()
                }),
            );
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    }

    // System metrics
    health_data.insert(
        "system".to_string(),
        serde_json::json!({
            "uptime_seconds": get_uptime_seconds(),
            "memory_usage": get_memory_usage(),
            "disk_usage": get_disk_usage(&state.config.storage.base_dir)
        }),
    );

    // Configuration status
    health_data.insert(
        "configuration".to_string(),
        serde_json::json!({
            "database_url_configured": !state.config.database.url.is_empty(),
            "storage_path_exists": state.config.storage.base_dir.exists(),
            "api_auth_enabled": state.config.api.enable_auth,
            "cors_enabled": state.config.api.enable_cors
        }),
    );

    Ok(Json(serde_json::Value::Object(health_data)))
}

/// Get memory usage information (simplified)
fn get_memory_usage() -> serde_json::Value {
    // This is a simplified implementation - in production you might want to use
    // system crates like `sysinfo` for more detailed memory information
    serde_json::json!({
        "note": "Memory usage tracking not implemented - consider adding sysinfo crate for production"
    })
}

/// Get disk usage for storage directory
fn get_disk_usage(path: &std::path::Path) -> serde_json::Value {
    match std::fs::metadata(path) {
        Ok(_) => serde_json::json!({
            "storage_path": path.display().to_string(),
            "accessible": true,
            "note": "Disk usage calculation not implemented - consider adding system monitoring"
        }),
        Err(_) => serde_json::json!({
            "storage_path": path.display().to_string(),
            "accessible": false,
            "error": "Storage path not accessible"
        }),
    }
}
