//! Request logging middleware for tracing and monitoring

use axum::{
    extract::{Request, State},
    http::HeaderValue,
    middleware::Next,
    response::Response,
};
use std::{sync::Arc, time::Instant};
use tracing::{info, warn, Span, Instrument};

use crate::state::AppState;

/// Request logging middleware that tracks request timing and details
pub async fn request_logging_middleware(
    State(_state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    let start_time = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let version = request.version();
    
    // Extract request ID if present, otherwise generate one
    let request_id = request
        .headers()
        .get("X-Request-ID")
        .and_then(|h| h.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| generate_request_id());
    
    // Add request timing to extensions for other middleware/handlers to use
    request.extensions_mut().insert(start_time);
    
    // Create a span for this request
    let span = tracing::info_span!(
        "request",
        method = %method,
        uri = %uri,
        version = ?version,
        request_id = %request_id,
    );
    
    let response = async move {
        info!("Starting request processing");
        
        let response = next.run(request).await;
        let elapsed = start_time.elapsed();
        let status = response.status();
        
        // Log the completion
        if status.is_client_error() || status.is_server_error() {
            warn!(
                status = %status,
                elapsed = ?elapsed,
                "Request completed with error"
            );
        } else {
            info!(
                status = %status,
                elapsed = ?elapsed,
                "Request completed successfully"
            );
        }
        
        response
    }.instrument(span).await;
    
    response
}

/// Generate a unique request ID for tracing
fn generate_request_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("req_{:016x}", rng.r#gen::<u64>())
}

/// Request timing middleware that adds timing information to request extensions
pub async fn timing_middleware(
    mut request: Request,
    next: Next,
) -> Response {
    let start_time = Instant::now();
    request.extensions_mut().insert(start_time);
    
    let response = next.run(request).await;
    
    let elapsed = start_time.elapsed();
    let mut response = response;
    
    // Add timing headers
    let headers = response.headers_mut();
    headers.insert("X-Response-Time", HeaderValue::from(elapsed.as_millis() as u64));
    
    response
}

/// Request logging middleware (stateless version)
pub async fn request_logging_middleware_stateless(
    mut request: Request,
    next: Next,
) -> Response {
    let start_time = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let version = request.version();
    
    // Extract request ID if present, otherwise generate one
    let request_id = request
        .headers()
        .get("X-Request-ID")
        .and_then(|h| h.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| generate_request_id());
    
    // Add request timing to extensions for other middleware/handlers to use
    request.extensions_mut().insert(start_time);
    
    // Create a span for this request
    let span = tracing::info_span!(
        "request",
        method = %method,
        uri = %uri,
        version = ?version,
        request_id = %request_id,
    );
    
    let response = async move {
        info!("Starting request processing");
        
        let response = next.run(request).await;
        let elapsed = start_time.elapsed();
        let status = response.status();
        
        // Log the completion
        if status.is_client_error() || status.is_server_error() {
            warn!(
                status = %status,
                elapsed = ?elapsed,
                "Request completed with error"
            );
        } else {
            info!(
                status = %status,
                elapsed = ?elapsed,
                "Request completed successfully"
            );
        }
        
        response
    }.instrument(span).await;
    
    response
}

/// Layer builder for request logging middleware (disabled for now)
// pub fn request_logging_layer() -> impl tower::Layer<axum::routing::Route> + Clone {
//     axum::middleware::from_fn(request_logging_middleware_stateless)
// }

/// Layer builder for timing middleware (stateless) (disabled for now)
// pub fn timing_layer() -> impl tower::Layer<axum::routing::Route> + Clone {
//     axum::middleware::from_fn(timing_middleware)
// }