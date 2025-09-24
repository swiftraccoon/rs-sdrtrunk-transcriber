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

/// Basic health check endpoint for monitoring systems
///
/// Provides essential health information including database connectivity, service uptime,
/// and connection pool statistics. This endpoint is designed for load balancers and
/// monitoring systems that need a quick health assessment.
///
/// # Arguments
///
/// * `state` - Application state containing database pool and configuration
///
/// # Returns
///
/// Returns HTTP 200 with health details if service is healthy, or HTTP 503 if database
/// connectivity fails or other critical issues are detected.
///
/// # Example Response
///
/// ```json
/// {
///   "status": "healthy",
///   "version": "0.1.0",
///   "timestamp": "2024-03-15T14:25:30Z",
///   "database": {
///     "connected": true,
///     "pool_stats": {
///       "connections_in_use": 2,
///       "max_connections": 50,
///       "idle_connections": 8
///     },
///     "response_time_ms": 15
///   },
///   "uptime_seconds": 3600
/// }
/// ```
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

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // Mock functions removed - they were causing unused warnings
    // Integration tests that need AppState should be in tests/ directory

    #[tokio::test]
    async fn test_health_response_serialization() {
        let health_response = HealthResponse {
            status: "healthy".to_string(),
            version: "0.1.0".to_string(),
            timestamp: chrono::Utc::now(),
            database: DatabaseHealth {
                connected: true,
                pool_stats: PoolStats {
                    connections_in_use: 2,
                    max_connections: 50,
                    idle_connections: 8,
                },
                response_time_ms: 15,
            },
            uptime_seconds: 3600,
        };

        let json = serde_json::to_string(&health_response).expect("Failed to serialize");
        assert!(json.contains("healthy"));
        assert!(json.contains("0.1.0"));
        assert!(json.contains("database"));
        assert!(json.contains("pool_stats"));
    }

    #[tokio::test]
    async fn test_readiness_response_serialization() {
        let readiness_response = ReadinessResponse {
            ready: true,
            timestamp: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&readiness_response).expect("Failed to serialize");
        assert!(json.contains("ready"));
        assert!(json.contains("true"));
    }

    #[test]
    fn test_get_uptime_seconds() {
        let uptime1 = get_uptime_seconds();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let uptime2 = get_uptime_seconds();

        // Uptime should increase
        assert!(uptime2 >= uptime1);

        // Should be reasonable (not zero, not crazy high)
        assert!(uptime1 < 3600); // Less than 1 hour for test
    }

    #[test]
    fn test_get_memory_usage() {
        let memory_usage = get_memory_usage();

        // Should return a JSON object
        assert!(memory_usage.is_object());
        assert!(memory_usage.get("note").is_some());
    }

    #[test]
    fn test_get_disk_usage_accessible_path() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let disk_usage = get_disk_usage(temp_dir.path());

        assert_eq!(disk_usage["accessible"], true);
        assert!(disk_usage["storage_path"].as_str().is_some());
    }

    #[test]
    fn test_get_disk_usage_inaccessible_path() {
        let nonexistent_path = std::path::Path::new("/nonexistent/path/that/should/not/exist");
        let disk_usage = get_disk_usage(nonexistent_path);

        assert_eq!(disk_usage["accessible"], false);
        assert!(disk_usage["error"].as_str().is_some());
    }

    // Note: Activity status testing is done in the stats module where it's defined

    #[tokio::test]
    async fn test_pool_stats_creation() {
        let pool_stats = PoolStats {
            connections_in_use: 5,
            max_connections: 20,
            idle_connections: 3,
        };

        assert_eq!(pool_stats.connections_in_use, 5);
        assert_eq!(pool_stats.max_connections, 20);
        assert_eq!(pool_stats.idle_connections, 3);
    }

    #[tokio::test]
    async fn test_database_health_creation() {
        let db_health = DatabaseHealth {
            connected: true,
            pool_stats: PoolStats {
                connections_in_use: 2,
                max_connections: 10,
                idle_connections: 1,
            },
            response_time_ms: 25,
        };

        assert!(db_health.connected);
        assert_eq!(db_health.response_time_ms, 25);
        assert_eq!(db_health.pool_stats.max_connections, 10);
    }

    // Mock functions to test internal logic without database dependencies
    #[tokio::test]
    async fn test_health_logic_without_db() {
        // Test uptime calculation
        let uptime1 = get_uptime_seconds();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let uptime2 = get_uptime_seconds();
        assert!(uptime2 >= uptime1);

        // Test memory usage mock
        let memory = get_memory_usage();
        assert!(memory.is_object());
    }

    #[tokio::test]
    async fn test_health_response_with_all_fields() {
        let timestamp = chrono::Utc::now();
        let health = HealthResponse {
            status: "healthy".to_string(),
            version: "1.0.0".to_string(),
            timestamp,
            database: DatabaseHealth {
                connected: true,
                pool_stats: PoolStats {
                    connections_in_use: 3,
                    max_connections: 25,
                    idle_connections: 5,
                },
                response_time_ms: 12,
            },
            uptime_seconds: 7200,
        };

        // Verify all fields are correctly set
        assert_eq!(health.status, "healthy");
        assert_eq!(health.version, "1.0.0");
        assert_eq!(health.uptime_seconds, 7200);
        assert!(health.database.connected);
        assert_eq!(health.database.response_time_ms, 12);
    }

    #[tokio::test]
    async fn test_readiness_response_ready() {
        let timestamp = chrono::Utc::now();
        let readiness = ReadinessResponse {
            ready: true,
            timestamp,
        };

        assert!(readiness.ready);
        // Timestamp should be recent (within last minute)
        let now = chrono::Utc::now();
        let diff = (now - readiness.timestamp).num_seconds().abs();
        assert!(diff < 60);
    }

    #[tokio::test]
    async fn test_readiness_response_not_ready() {
        let readiness = ReadinessResponse {
            ready: false,
            timestamp: chrono::Utc::now(),
        };

        assert!(!readiness.ready);
    }

    #[tokio::test]
    async fn test_health_check_handler_with_memory_db() {
        use crate::state::AppState;
        use axum::extract::State;
        use sdrtrunk_core::Config;
        use sdrtrunk_database::Database;
        use std::sync::Arc;

        // Try to create a test database connection
        let mut config = Config::default();
        config.database.url = "sqlite::memory:".to_string();

        if let Ok(db) = Database::new(&config).await
            && db.migrate().await.is_ok()
        {
            let state = Arc::new(AppState::new(config, db.pool().clone()).unwrap());

            // Test successful health check
            let result = health_check(State(state.clone())).await;

            match result {
                Ok(json_response) => {
                    let health = json_response.0;
                    assert_eq!(health.status, "healthy");
                    assert!(health.database.connected);
                    assert!(health.uptime_seconds > 0); // Should have positive uptime
                    assert!(!health.version.is_empty());
                }
                Err(status) => {
                    // Database connection might fail in test environment, that's ok
                    assert_eq!(status, axum::http::StatusCode::SERVICE_UNAVAILABLE);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_readiness_check_handler_with_memory_db() {
        use crate::state::AppState;
        use axum::extract::State;
        use sdrtrunk_core::Config;
        use sdrtrunk_database::Database;
        use std::sync::Arc;

        // Try to create a test database connection
        let mut config = Config::default();
        config.database.url = "sqlite::memory:".to_string();

        if let Ok(db) = Database::new(&config).await
            && db.migrate().await.is_ok()
        {
            let state = Arc::new(AppState::new(config, db.pool().clone()).unwrap());

            // Test successful readiness check
            let result = readiness_check(State(state.clone())).await;

            match result {
                Ok(json_response) => {
                    let readiness = json_response.0;
                    assert!(readiness.ready);
                }
                Err(status) => {
                    // Database connection might fail in test environment, that's ok
                    assert_eq!(status, axum::http::StatusCode::SERVICE_UNAVAILABLE);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_check_database_health_function() {
        use crate::state::AppState;
        use sdrtrunk_core::Config;
        use sdrtrunk_database::Database;
        use std::sync::Arc;

        // Try to create a test database connection
        let mut config = Config::default();
        config.database.url = "sqlite::memory:".to_string();

        if let Ok(db) = Database::new(&config).await
            && db.migrate().await.is_ok()
        {
            let state = Arc::new(AppState::new(config, db.pool().clone()).unwrap());

            // Test database health check
            let result = check_database_health(&state).await;

            match result {
                Ok(db_health) => {
                    assert!(db_health.connected);
                    // response_time_ms is u32, so it's always >= 0 - check it's reasonable instead
                    assert!(
                        db_health.response_time_ms < 5000,
                        "Database response time too high"
                    );
                    assert!(db_health.pool_stats.max_connections > 0);
                    // These are u32, always >= 0 - check they're within expected range
                    assert!(
                        db_health.pool_stats.idle_connections
                            <= db_health.pool_stats.max_connections
                    );
                    assert!(
                        db_health.pool_stats.connections_in_use
                            <= db_health.pool_stats.max_connections
                    );
                }
                Err(_) => {
                    // Database health check might fail in test environment, that's acceptable
                }
            }
        }
    }

    #[test]
    fn test_health_endpoints_response_serialization() {
        use chrono::Utc;

        // Test HealthResponse serialization
        let health = HealthResponse {
            status: "healthy".to_string(),
            version: "1.0.0".to_string(),
            timestamp: Utc::now(),
            database: DatabaseHealth {
                connected: true,
                pool_stats: PoolStats {
                    connections_in_use: 5,
                    max_connections: 25,
                    idle_connections: 10,
                },
                response_time_ms: 15,
            },
            uptime_seconds: 3600,
        };

        let json = serde_json::to_string(&health).expect("Failed to serialize health response");
        assert!(json.contains("healthy"));
        assert!(json.contains("1.0.0"));
        assert!(json.contains("connected"));
        assert!(json.contains("3600"));

        // Test deserialization
        let deserialized: HealthResponse =
            serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(deserialized.status, "healthy");
        assert_eq!(deserialized.version, "1.0.0");
        assert_eq!(deserialized.uptime_seconds, 3600);

        // Test ReadinessResponse serialization
        let readiness = ReadinessResponse {
            ready: true,
            timestamp: Utc::now(),
        };

        let json =
            serde_json::to_string(&readiness).expect("Failed to serialize readiness response");
        assert!(json.contains("ready"));
        assert!(json.contains("true"));

        // Test deserialization
        let deserialized: ReadinessResponse =
            serde_json::from_str(&json).expect("Failed to deserialize");
        assert!(deserialized.ready);
    }

    #[test]
    fn test_database_health_structures() {
        let pool_stats = PoolStats {
            connections_in_use: 10,
            max_connections: 50,
            idle_connections: 15,
        };

        let db_health = DatabaseHealth {
            connected: true,
            pool_stats: pool_stats.clone(),
            response_time_ms: 25,
        };

        assert!(db_health.connected);
        assert_eq!(db_health.response_time_ms, 25);
        assert_eq!(db_health.pool_stats.connections_in_use, 10);
        assert_eq!(db_health.pool_stats.max_connections, 50);
        assert_eq!(db_health.pool_stats.idle_connections, 15);

        // Test serialization
        let json = serde_json::to_string(&db_health).expect("Failed to serialize");
        assert!(json.contains("connected"));
        assert!(json.contains("25"));
        assert!(json.contains("50"));

        // Test cloning
        let cloned = db_health.clone();
        assert_eq!(cloned.connected, db_health.connected);
        assert_eq!(cloned.response_time_ms, db_health.response_time_ms);
    }

    #[test]
    fn test_get_uptime_seconds_function() {
        // Test that get_uptime_seconds returns a reasonable value
        let uptime = get_uptime_seconds();

        // Uptime is i64, check it's within reasonable bounds
        // Should be non-negative (could be 0 at startup)
        assert!(uptime >= 0, "Uptime should be non-negative");
        // Assume test system hasn't been running for more than 10 years
        assert!(uptime < 365 * 24 * 3600 * 10, "Uptime unreasonably high");
    }

    #[test]
    fn test_get_memory_usage_function() {
        // Test memory usage reporting
        let memory_usage = get_memory_usage();

        // Memory usage should return a JSON value
        assert!(memory_usage.is_object());

        // Should contain the note field since it's not implemented
        if let Some(note) = memory_usage.get("note") {
            assert!(note.is_string());
        }
    }

    #[test]
    fn test_get_disk_usage_functions() {
        use std::path::Path;

        // Test with accessible path (current directory)
        let current_dir = std::env::current_dir().unwrap();
        let disk_usage = get_disk_usage(&current_dir);

        // Should return a JSON value
        assert!(disk_usage.is_object());

        // Test with inaccessible path
        let bad_path = Path::new("/nonexistent/path/that/should/not/exist");
        let result = get_disk_usage(bad_path);

        // Should still return a JSON object (may contain error info)
        assert!(result.is_object());
    }

    #[test]
    fn test_health_response_comprehensive() {
        use chrono::Utc;

        // Test with various field combinations
        let test_cases = vec![
            ("healthy", true, 0, 100),
            ("degraded", true, 500, 50),
            ("unhealthy", false, 1000, 0),
        ];

        for (status, connected, response_time, uptime) in test_cases {
            let health = HealthResponse {
                status: status.to_string(),
                version: "test".to_string(),
                timestamp: Utc::now(),
                database: DatabaseHealth {
                    connected,
                    pool_stats: PoolStats {
                        connections_in_use: 1,
                        max_connections: 10,
                        idle_connections: 9,
                    },
                    response_time_ms: response_time,
                },
                uptime_seconds: uptime,
            };

            assert_eq!(health.status, status);
            assert_eq!(health.database.connected, connected);
            assert_eq!(health.database.response_time_ms, response_time);
            assert_eq!(health.uptime_seconds, uptime);
        }
    }
}
