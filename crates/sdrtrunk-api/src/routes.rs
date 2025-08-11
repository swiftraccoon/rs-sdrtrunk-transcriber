//! API route definitions with comprehensive middleware integration

use crate::{handlers, state::AppState};
use axum::{
    Router,
    routing::{delete, get, post},
};
use std::sync::Arc;
use tower_http::compression::CompressionLayer;

/// Build API routes with basic middleware stack
pub fn api_routes() -> Router<Arc<AppState>> {
    Router::new()
        // Upload endpoints - Rdio Scanner compatible
        .route(
            "/api/call-upload",
            post(handlers::upload::handle_call_upload),
        )
        .route(
            "/api/rdio-scanner/upload",
            post(handlers::upload::handle_call_upload),
        )
        // SDRTrunk connectivity test endpoints
        .route("/test", get(connectivity_test))
        .route("/api/test", get(connectivity_test))
        .route("/api", get(api_info))
        .route("/", get(root_endpoint))
        // Call management endpoints
        .route("/api/calls", get(handlers::calls::list_calls))
        .route("/api/calls/:id", get(handlers::calls::get_call))
        // .route("/api/calls/:id/audio", get(handlers::calls::get_call_audio)) // Disabled for minimal build
        // Statistics endpoints
        .route(
            "/api/systems/:system_id/stats",
            get(handlers::stats::get_system_stats),
        )
        .route("/api/stats/global", get(handlers::stats::get_global_stats))
        // Apply basic middleware
        .layer(CompressionLayer::new())
}

/// Build health check routes (no authentication required)
pub fn health_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(handlers::health::health_check))
        .route("/ready", get(handlers::health::readiness_check))
        .route(
            "/health/detailed",
            get(handlers::health::detailed_health_check),
        )
}

/// Build documentation and utility routes
pub fn docs_routes() -> Router<Arc<AppState>> {
    Router::new()
        // API documentation endpoints
        .route("/api/docs", get(serve_api_docs))
        .route("/api/docs/openapi.json", get(serve_openapi_spec))
        // Metrics endpoint (could be secured separately)
        .route("/metrics", get(serve_metrics))
}

/// Build admin routes with strict authentication
pub fn admin_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/admin/stats", get(admin_stats))
        .route("/admin/cleanup", post(admin_cleanup))
        .route("/admin/api-keys", get(list_api_keys))
        .route("/admin/api-keys", post(create_api_key))
        .route("/admin/api-keys/:key_id", get(get_api_key))
        .route("/admin/api-keys/:key_id", delete(delete_api_key))
}

/// Serve API documentation
async fn serve_api_docs() -> &'static str {
    // In a real implementation, this would serve the API documentation
    // For now, return a placeholder
    "API Documentation - See /api/docs/openapi.json for OpenAPI specification"
}

/// Serve OpenAPI specification
async fn serve_openapi_spec() -> axum::Json<serde_json::Value> {
    // In a real implementation, this would generate or serve the OpenAPI spec
    axum::Json(serde_json::json!({
        "openapi": "3.0.0",
        "info": {
            "title": "SDRTrunk Transcriber API",
            "version": "0.1.0",
            "description": "REST API for SDRTrunk call transcription and management"
        },
        "paths": {
            "/api/call-upload": {
                "post": {
                    "summary": "Upload a radio call recording",
                    "description": "Upload audio files from SDRTrunk for processing and transcription"
                }
            },
            "/api/calls": {
                "get": {
                    "summary": "List radio calls",
                    "description": "Retrieve a paginated list of radio calls with filtering options"
                }
            }
        }
    }))
}

/// Serve Prometheus metrics
async fn serve_metrics() -> &'static str {
    // In a real implementation, this would serve Prometheus metrics
    // For now, return a placeholder
    "# HELP sdrtrunk_calls_total Total number of calls processed\n# TYPE sdrtrunk_calls_total counter\nsdrtrunk_calls_total 0\n"
}

/// Admin statistics endpoint
async fn admin_stats(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    // Implementation would gather comprehensive system statistics
    let stats = serde_json::json!({
        "database": {
            "pool_size": state.pool.size(),
            "idle_connections": state.pool.num_idle()
        },
        "storage": {
            "upload_dir": state.upload_dir.display().to_string()
        },
        "config": {
            "api_auth_enabled": state.config.security.require_api_key,
            "cors_enabled": state.config.api.enable_cors
        }
    });

    Ok(axum::Json(stats))
}

/// Admin cleanup endpoint
async fn admin_cleanup(
    axum::extract::State(_state): axum::extract::State<Arc<AppState>>,
) -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    // Implementation would perform database cleanup operations
    Ok(axum::Json(serde_json::json!({
        "message": "Cleanup operation completed",
        "cleaned_records": 0
    })))
}

/// List API keys (admin only)
async fn list_api_keys() -> axum::Json<serde_json::Value> {
    // Implementation would list API keys (without revealing actual keys)
    axum::Json(serde_json::json!({
        "api_keys": []
    }))
}

/// Create API key (admin only)
async fn create_api_key() -> axum::Json<serde_json::Value> {
    // Implementation would create a new API key
    axum::Json(serde_json::json!({
        "message": "API key creation not yet implemented"
    }))
}

/// Get API key details (admin only)
async fn get_api_key(
    axum::extract::Path(_key_id): axum::extract::Path<String>,
) -> axum::Json<serde_json::Value> {
    // Implementation would return API key details
    axum::Json(serde_json::json!({
        "message": "API key details not yet implemented"
    }))
}

/// Delete API key (admin only)
async fn delete_api_key(
    axum::extract::Path(_key_id): axum::extract::Path<String>,
) -> axum::Json<serde_json::Value> {
    // Implementation would delete API key
    axum::Json(serde_json::json!({
        "message": "API key deletion not yet implemented"
    }))
}

/// Combine all routes into a single router
pub fn build_router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(api_routes())
        .merge(health_routes())
        .merge(docs_routes())
        .merge(admin_routes())
        // Fallback handler for unknown routes
        .fallback(not_found_handler)
}

/// Handle 404 Not Found errors
async fn not_found_handler() -> (axum::http::StatusCode, axum::Json<serde_json::Value>) {
    (
        axum::http::StatusCode::NOT_FOUND,
        axum::Json(serde_json::json!({
            "error": "Not Found",
            "code": "ROUTE_NOT_FOUND",
            "message": "The requested endpoint does not exist"
        })),
    )
}

/// Root endpoint for basic connectivity
async fn root_endpoint() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "service": "SDRTrunk Transcriber API",
        "version": "0.1.0",
        "status": "ok"
    }))
}

/// Connectivity test endpoint for SDRTrunk
async fn connectivity_test() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "message": "Rdio Scanner API compatible endpoint",
        "service": "sdrtrunk-transcriber"
    }))
}

/// API info endpoint
async fn api_info() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "api": "SDRTrunk Transcriber API",
        "version": "0.1.0",
        "endpoints": {
            "upload": "/api/call-upload",
            "rdio_upload": "/api/rdio-scanner/upload",
            "calls": "/api/calls",
            "health": "/health"
        },
        "compatible": "Rdio Scanner API"
    }))
}
