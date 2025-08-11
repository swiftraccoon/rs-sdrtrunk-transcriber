//! CORS middleware for cross-origin request handling

use crate::state::AppState;
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use tracing::{debug, warn};

/// CORS middleware that handles cross-origin requests based on configuration
pub async fn cors_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let method = request.method().clone();
    let headers = request.headers();
    
    // Get origin from request
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("null")
        .to_string();
    
    debug!("CORS request from origin: {}", origin);
    
    // Check if CORS is enabled
    if !state.config.api.enable_cors {
        debug!("CORS disabled in configuration");
        return Ok(next.run(request).await);
    }
    
    // Check if origin is allowed
    let allowed = is_origin_allowed(&origin, &state.config.api.cors_origins);
    
    if !allowed {
        warn!("Origin {} not allowed by CORS policy", origin);
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Handle preflight OPTIONS request
    if method == Method::OPTIONS {
        debug!("Handling CORS preflight request");
        return Ok(build_preflight_response(&origin, &state));
    }
    
    // Process the actual request
    let mut response = next.run(request).await;
    
    // Add CORS headers to the response
    add_cors_headers(&mut response, &origin, &state);
    
    Ok(response)
}

/// Check if the given origin is allowed by the CORS policy
fn is_origin_allowed(origin: &str, allowed_origins: &[String]) -> bool {
    // If wildcard is present, allow all origins
    if allowed_origins.contains(&"*".to_string()) {
        return true;
    }
    
    // Check exact match
    if allowed_origins.contains(&origin.to_string()) {
        return true;
    }
    
    // Check pattern matching (basic support for subdomain wildcards)
    for allowed in allowed_origins {
        if allowed.starts_with("*.") {
            let domain = &allowed[2..];
            if origin.ends_with(domain) {
                return true;
            }
        }
    }
    
    false
}

/// Build a preflight response for OPTIONS requests
fn build_preflight_response(origin: &str, state: &Arc<AppState>) -> Response {
    let mut response = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(axum::body::Body::empty())
        .unwrap();
    
    add_cors_headers(&mut response, origin, state);
    
    // Add preflight-specific headers
    let headers = response.headers_mut();
    
    // Allow common methods
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS, HEAD"),
    );
    
    // Allow common headers
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static(
            "Accept, Accept-Language, Content-Language, Content-Type, Authorization, X-API-Key, X-Request-ID"
        ),
    );
    
    // Set max age for preflight cache (24 hours)
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("86400"),
    );
    
    response
}

/// Add CORS headers to a response
fn add_cors_headers(response: &mut Response, origin: &str, state: &Arc<AppState>) {
    let headers = response.headers_mut();
    
    // Set allowed origin
    if state.config.api.cors_origins.contains(&"*".to_string()) {
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        );
    } else if is_origin_allowed(origin, &state.config.api.cors_origins) {
        if let Ok(origin_header) = HeaderValue::from_str(origin) {
            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin_header);
        }
    }
    
    // Allow credentials if not using wildcard origin
    if !state.config.api.cors_origins.contains(&"*".to_string()) {
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            HeaderValue::from_static("true"),
        );
    }
    
    // Expose custom headers
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("X-Response-Time, X-Request-ID, X-RateLimit-Remaining, X-RateLimit-Reset"),
    );
}

/// CORS layer builder (using permissive CORS for simplicity)
pub fn cors_layer() -> impl tower::Layer<axum::routing::Route> + Clone {
    axum::middleware::from_fn(permissive_cors_middleware)
}

/// Simple CORS layer for development (allows everything)
pub fn permissive_cors_layer() -> impl tower::Layer<axum::routing::Route> + Clone {
    axum::middleware::from_fn(permissive_cors_middleware)
}

/// Permissive CORS middleware for development
async fn permissive_cors_middleware(
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    
    // Handle preflight
    if method == Method::OPTIONS {
        return Response::builder()
            .status(StatusCode::NO_CONTENT)
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, PUT, DELETE, OPTIONS, HEAD")
            .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "*")
            .header(header::ACCESS_CONTROL_MAX_AGE, "86400")
            .body(axum::body::Body::empty())
            .unwrap();
    }
    
    // Process request and add headers
    let mut response = next.run(request).await;
    
    let headers = response.headers_mut();
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, HeaderValue::from_static("*"));
    headers.insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, HeaderValue::from_static("*"));
    
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_origin_allowed_wildcard() {
        let origins = vec!["*".to_string()];
        assert!(is_origin_allowed("https://example.com", &origins));
        assert!(is_origin_allowed("http://localhost:3000", &origins));
    }

    #[test]
    fn test_is_origin_allowed_exact_match() {
        let origins = vec![
            "https://example.com".to_string(),
            "http://localhost:3000".to_string(),
        ];
        assert!(is_origin_allowed("https://example.com", &origins));
        assert!(is_origin_allowed("http://localhost:3000", &origins));
        assert!(!is_origin_allowed("https://evil.com", &origins));
    }

    #[test]
    fn test_is_origin_allowed_wildcard_subdomain() {
        let origins = vec!["*.example.com".to_string()];
        assert!(is_origin_allowed("https://api.example.com", &origins));
        assert!(is_origin_allowed("https://www.example.com", &origins));
        assert!(!is_origin_allowed("https://example.com", &origins)); // Exact domain not matched by *.
        assert!(!is_origin_allowed("https://evil.com", &origins));
    }

    #[test]
    fn test_is_origin_allowed_empty_list() {
        let origins = vec![];
        assert!(!is_origin_allowed("https://example.com", &origins));
    }
}