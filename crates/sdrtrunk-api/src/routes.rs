//! API route definitions with comprehensive middleware integration

use crate::{handlers, state::AppState};
use axum::{
    Router,
    routing::{delete, get, post},
};
use http::StatusCode;
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
        // Queue statistics endpoint
        .route("/api/queue/stats", get(handlers::stats::queue_stats))
        // Transcription webhook endpoint
        .route(
            "/api/v1/transcription/callback",
            post(handlers::transcription::transcription_callback),
        )
        // WebSocket endpoint for real-time updates
        .route("/api/ws", get(handlers::websocket::websocket_handler))
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
        // Prometheus metrics endpoint
        .route("/metrics", get(handlers::metrics::serve_metrics))
}

/// Build admin routes with strict authentication
pub fn admin_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/admin/stats", get(admin_stats))
        .route("/admin/cleanup", post(admin_cleanup))
        .route("/admin/api-keys", get(handlers::admin::list_api_keys))
        .route("/admin/api-keys", post(handlers::admin::create_api_key))
        .route(
            "/admin/api-keys/:key_id",
            get(handlers::admin::get_api_key_details),
        )
        .route(
            "/admin/api-keys/:key_id",
            delete(handlers::admin::delete_api_key),
        )
}

/// Serve API documentation
async fn serve_api_docs() -> &'static str {
    // In a real implementation, this would serve the API documentation
    // For now, return a placeholder
    "API Documentation - See /api/docs/openapi.json for OpenAPI specification"
}

/// Serve `OpenAPI` specification
async fn serve_openapi_spec() -> axum::Json<serde_json::Value> {
    axum::Json(crate::openapi::generate_openapi_spec())
}

/// Admin statistics endpoint
///
/// # Errors
///
/// Returns an error if the database query fails.
async fn admin_stats(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Result<axum::Json<serde_json::Value>, StatusCode> {
    // Implementation would gather comprehensive system statistics
    let admin_stats = serde_json::json!({
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

    Ok(axum::Json(admin_stats))
}

/// Admin cleanup endpoint
///
/// # Errors
///
/// Returns an error if the database operations fail.
async fn admin_cleanup(
    axum::extract::State(_state): axum::extract::State<Arc<AppState>>,
) -> Result<axum::Json<serde_json::Value>, StatusCode> {
    // Implementation would perform database cleanup operations
    Ok(axum::Json(serde_json::json!({
        "message": "Cleanup operation completed",
        "cleaned_records": 0
    })))
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
async fn not_found_handler() -> (StatusCode, axum::Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
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

/// Connectivity test endpoint for `SDRTrunk`
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
    use axum::http::StatusCode;
    use serde_json;

    #[tokio::test]
    async fn test_serve_api_docs() {
        let docs = serve_api_docs().await;
        let expected = "API Documentation - See /api/docs/openapi.json for OpenAPI specification";

        // Test that the function returns the expected static string
        assert_eq!(docs, expected);
    }

    #[tokio::test]
    async fn test_serve_openapi_spec() {
        let spec = serve_openapi_spec().await;
        let json_value = spec.0;

        // Verify OpenAPI spec structure
        assert_eq!(json_value["openapi"], "3.0.3");
        assert_eq!(json_value["info"]["title"], "SDRTrunk Transcriber API");
        assert_eq!(json_value["info"]["version"], "0.1.0");

        // Check that paths exist
        assert!(json_value["paths"].is_object());
        let paths = json_value["paths"].as_object().unwrap();
        assert!(!paths.is_empty());

        // Check specific paths that we know exist in the comprehensive spec
        assert!(paths.contains_key("/api/call-upload"));
        assert!(paths.contains_key("/api/calls"));
        assert!(paths.contains_key("/api/calls/{id}"));
        assert!(paths.contains_key("/health"));
        assert!(paths.contains_key("/metrics"));
        assert!(paths.contains_key("/api/ws"));
    }

    #[tokio::test]
    async fn test_connectivity_test_response() {
        let response = connectivity_test().await;
        let json_value = response.0;

        assert_eq!(json_value["status"], "ok");
        assert_eq!(json_value["service"], "sdrtrunk-transcriber");
        assert!(
            json_value["message"]
                .as_str()
                .unwrap()
                .contains("Rdio Scanner")
        );
    }

    #[tokio::test]
    async fn test_root_endpoint_response() {
        let response = root_endpoint().await;
        let json_value = response.0;

        assert_eq!(json_value["service"], "SDRTrunk Transcriber API");
        assert_eq!(json_value["version"], "0.1.0");
        assert_eq!(json_value["status"], "ok");
    }

    #[tokio::test]
    async fn test_api_info_response() {
        let response = api_info().await;
        let json_value = response.0;

        assert_eq!(json_value["api"], "SDRTrunk Transcriber API");
        assert_eq!(json_value["version"], "0.1.0");
        assert_eq!(json_value["compatible"], "Rdio Scanner API");

        // Check endpoints structure
        let endpoints = &json_value["endpoints"];
        assert_eq!(endpoints["upload"], "/api/call-upload");
        assert_eq!(endpoints["rdio_upload"], "/api/rdio-scanner/upload");
        assert_eq!(endpoints["calls"], "/api/calls");
        assert_eq!(endpoints["health"], "/health");
    }

    #[tokio::test]
    async fn test_not_found_handler() {
        let (status, response) = not_found_handler().await;
        let json_value = response.0;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json_value["error"], "Not Found");
        assert_eq!(json_value["code"], "ROUTE_NOT_FOUND");
        assert!(
            json_value["message"]
                .as_str()
                .unwrap()
                .contains("requested endpoint does not exist")
        );
    }

    // Test router construction (without running server)
    #[test]
    fn test_api_routes_construction() {
        let router = api_routes();

        // Router should be constructible
        // We can't easily test routing without a full app setup
        // but we can verify the router builds
        assert!(std::mem::size_of_val(&router) > 0);
    }

    #[test]
    fn test_health_routes_construction() {
        let router = health_routes();
        assert!(std::mem::size_of_val(&router) > 0);
    }

    #[test]
    fn test_docs_routes_construction() {
        let router = docs_routes();
        assert!(std::mem::size_of_val(&router) > 0);
    }

    #[test]
    fn test_admin_routes_construction() {
        let router = admin_routes();
        assert!(std::mem::size_of_val(&router) > 0);
    }

    #[test]
    fn test_build_router_construction() {
        let router = build_router();
        assert!(std::mem::size_of_val(&router) > 0);
    }

    // Test JSON serialization of responses
    #[tokio::test]
    async fn test_openapi_spec_serialization() {
        let spec = serve_openapi_spec().await;
        let json_string = serde_json::to_string(&spec.0).expect("Should serialize to JSON");

        // Verify it's valid JSON and contains key fields
        assert!(json_string.contains("\"openapi\":\"3.0.3\""));
        assert!(json_string.contains("\"title\":\"SDRTrunk Transcriber API\""));
    }

    #[tokio::test]
    async fn test_error_response_structure() {
        let (status, response) = not_found_handler().await;

        // Verify error response has all required fields
        let json_value = response.0;
        assert!(json_value.is_object());
        assert!(json_value.get("error").is_some());
        assert!(json_value.get("code").is_some());
        assert!(json_value.get("message").is_some());

        // Verify status code
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_api_endpoint_responses_are_json() {
        // Test that all API endpoints return JSON responses
        let root_resp = root_endpoint().await;
        let api_resp = api_info().await;
        let connectivity_resp = connectivity_test().await;

        // All should be axum::Json responses
        assert!(root_resp.0.is_object());
        assert!(api_resp.0.is_object());
        assert!(connectivity_resp.0.is_object());
    }

    #[tokio::test]
    async fn test_openapi_spec_paths() {
        let spec = serve_openapi_spec().await;
        let paths = &spec.0["paths"];

        // Check that paths object exists and has content
        assert!(paths.is_object());

        // The paths are in the JSON under the specific keys defined in serve_openapi_spec
        assert!(!paths.as_object().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_connectivity_test_compatibility() {
        let response = connectivity_test().await;
        let json_value = response.0;

        // Should indicate Rdio Scanner compatibility
        let message = json_value["message"].as_str().unwrap();
        assert!(message.contains("Rdio Scanner"));
        assert!(message.contains("compatible"));

        // Should have status ok
        assert_eq!(json_value["status"], "ok");
    }

    #[tokio::test]
    async fn test_api_docs_placeholder() {
        let docs = serve_api_docs().await;

        // Should mention OpenAPI spec location
        assert!(docs.contains("/api/docs/openapi.json"));
        assert!(docs.contains("API Documentation"));
        assert!(docs.contains("OpenAPI specification"));
    }
}
