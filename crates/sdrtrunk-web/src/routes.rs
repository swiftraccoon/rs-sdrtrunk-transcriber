//! Route definitions for the web interface

use crate::{
    handlers::{api, pages},
    state::AppState,
};
use axum::{Router, routing::get};
use std::sync::Arc;

/// Build the complete web application router
pub fn build_routes() -> Router<Arc<AppState>> {
    Router::new()
        // Page routes
        .route("/", get(pages::dashboard))
        .route("/calls", get(pages::calls_page))
        .route("/stats", get(pages::stats_page))
        .route("/admin", get(pages::admin_page))
        // API proxy routes
        .route("/api/calls", get(api::api_calls))
        .route("/api/stats/global", get(api::api_global_stats))
        .route("/api/calls/:id/audio", get(api::serve_audio))
        // WebSocket for real-time updates
        .route("/ws", get(api::websocket_handler))
        // Health check
        .route("/health", get(api::health_check))
}
