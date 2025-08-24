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

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode, HeaderMap, HeaderValue, Uri},
        middleware::Next,
        response::Response,
    };
    use std::sync::Arc;

    // Mock AppState for testing
    fn create_mock_app_state() -> Arc<AppState> {
        // This is a simplified mock - in real tests you'd need a proper AppState
        // For now, we'll focus on testing the stateless middleware
        unimplemented!("Mock AppState not needed for stateless tests")
    }

    #[test]
    fn test_generate_request_id() {
        let id1 = generate_request_id();
        let id2 = generate_request_id();
        
        // Should start with "req_"
        assert!(id1.starts_with("req_"));
        assert!(id2.starts_with("req_"));
        
        // Should be unique
        assert_ne!(id1, id2);
        
        // Should be proper length ("req_" + 16 hex chars)
        assert_eq!(id1.len(), 20);
        assert_eq!(id2.len(), 20);
        
        // Should contain only valid hex characters after prefix
        let hex_part = &id1[4..];
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_generate_request_id_uniqueness() {
        let mut ids = std::collections::HashSet::new();
        
        // Generate 1000 IDs and ensure they're all unique
        for _ in 0..1000 {
            let id = generate_request_id();
            assert!(ids.insert(id), "Generated duplicate request ID");
        }
    }

    #[tokio::test]
    async fn test_timing_middleware() {
        // Create a test request
        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        // Mock Next that returns a simple response
        let next = Next::new(|_req: Request| async {
            // Simulate some processing time
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap()
        });

        let response = timing_middleware(request, next).await;
        
        // Check that response has timing header
        assert!(response.headers().contains_key("X-Response-Time"));
        
        // The response time should be at least 10ms (our sleep time)
        let response_time_header = response.headers().get("X-Response-Time").unwrap();
        let response_time: u64 = response_time_header.to_str().unwrap().parse().unwrap();
        assert!(response_time >= 10, "Response time {} should be at least 10ms", response_time);
    }

    #[tokio::test]
    async fn test_timing_middleware_adds_extension() {
        let mut request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        // Verify extension is not present initially
        assert!(request.extensions().get::<Instant>().is_none());

        let next = Next::new(|req: Request| async {
            // Check that the extension was added
            let start_time = req.extensions().get::<Instant>().copied();
            assert!(start_time.is_some(), "Start time should be added to request extensions");
            
            Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap()
        });

        let _response = timing_middleware(request, next).await;
    }

    #[tokio::test]
    async fn test_request_logging_middleware_stateless_success() {
        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/test")
            .header("X-Request-ID", "custom-req-123")
            .body(Body::empty())
            .unwrap();

        let next = Next::new(|_req: Request| async {
            Response::builder()
                .status(StatusCode::CREATED)
                .body(Body::empty())
                .unwrap()
        });

        let response = request_logging_middleware_stateless(request, next).await;
        
        // Should return the response from next
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_request_logging_middleware_stateless_error() {
        let request = Request::builder()
            .method(Method::GET)
            .uri("/api/error")
            .body(Body::empty())
            .unwrap();

        let next = Next::new(|_req: Request| async {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        });

        let response = request_logging_middleware_stateless(request, next).await;
        
        // Should return the error response from next
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_request_logging_middleware_stateless_with_custom_request_id() {
        let custom_id = "my-custom-request-id-456";
        let request = Request::builder()
            .method(Method::PATCH)
            .uri("/api/update")
            .header("X-Request-ID", custom_id)
            .body(Body::empty())
            .unwrap();

        let next = Next::new(|req: Request| async {
            // Check that the extension was added
            assert!(req.extensions().get::<Instant>().is_some());
            
            Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap()
        });

        let _response = request_logging_middleware_stateless(request, next).await;
        // The logging middleware should have used the custom request ID
        // (we can't easily verify the log output in unit tests)
    }

    #[tokio::test]
    async fn test_request_logging_middleware_stateless_without_request_id() {
        let request = Request::builder()
            .method(Method::DELETE)
            .uri("/api/delete/123")
            .body(Body::empty())
            .unwrap();

        let next = Next::new(|req: Request| async {
            // Check that the extension was added
            assert!(req.extensions().get::<Instant>().is_some());
            
            Response::builder()
                .status(StatusCode::NO_CONTENT)
                .body(Body::empty())
                .unwrap()
        });

        let response = request_logging_middleware_stateless(request, next).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_request_logging_middleware_stateless_invalid_request_id_header() {
        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .header("X-Request-ID", HeaderValue::from_bytes(&[0xFF, 0xFE]).unwrap())
            .body(Body::empty())
            .unwrap();

        let next = Next::new(|_req: Request| async {
            Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap()
        });

        // Should not panic with invalid UTF-8 in request ID header
        let response = request_logging_middleware_stateless(request, next).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_timing_middleware_header_format() {
        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let next = Next::new(|_req: Request| async {
            // Sleep for a predictable amount of time
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            
            let mut response = Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap();
            
            // Add some existing headers to test that timing doesn't interfere
            response.headers_mut().insert("Content-Type", HeaderValue::from_static("text/plain"));
            
            response
        });

        let response = timing_middleware(request, next).await;
        
        // Check that original headers are preserved
        assert_eq!(response.headers().get("Content-Type").unwrap(), "text/plain");
        
        // Check timing header
        let timing_header = response.headers().get("X-Response-Time").unwrap();
        let timing_value = timing_header.to_str().unwrap();
        
        // Should be a valid number
        let timing_ms: u64 = timing_value.parse().expect("Should be valid number");
        
        // Should be at least 50ms
        assert!(timing_ms >= 50, "Timing {} should be at least 50ms", timing_ms);
        
        // Should be reasonable (less than 1 second for this simple test)
        assert!(timing_ms < 1000, "Timing {} should be less than 1 second", timing_ms);
    }

    #[tokio::test]
    async fn test_timing_middleware_zero_processing_time() {
        let request = Request::builder()
            .method(Method::GET)
            .uri("/instant")
            .body(Body::empty())
            .unwrap();

        let next = Next::new(|_req: Request| async {
            // Return immediately without any processing
            Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap()
        });

        let response = timing_middleware(request, next).await;
        
        let timing_header = response.headers().get("X-Response-Time").unwrap();
        let timing_value = timing_header.to_str().unwrap();
        let timing_ms: u64 = timing_value.parse().unwrap();
        
        // Even instant processing should have some measurable time (usually 0-1ms)
        // Just verify it's a valid number and not negative
        assert!(timing_ms < 100, "Even instant processing should complete quickly, got {}ms", timing_ms);
    }

    #[tokio::test]
    async fn test_multiple_middleware_interaction() {
        let request = Request::builder()
            .method(Method::POST)
            .uri("/api/test")
            .body(Body::empty())
            .unwrap();

        // Simulate request_logging_middleware_stateless followed by timing_middleware
        let next_timing = Next::new(|req: Request| async {
            // This simulates what would happen in a real handler
            // The request should have timing info in extensions
            assert!(req.extensions().get::<Instant>().is_some());
            
            Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap()
        });

        let next_logging = Next::new(|req: Request| async {
            timing_middleware(req, next_timing).await
        });

        let response = request_logging_middleware_stateless(request, next_logging).await;
        
        // Response should have timing header from timing middleware
        assert!(response.headers().contains_key("X-Response-Time"));
        assert_eq!(response.status(), StatusCode::OK);
    }
}