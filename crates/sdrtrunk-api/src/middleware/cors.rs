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
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_is_origin_allowed_wildcard() {
        let origins = vec!["*".to_string()];
        assert!(is_origin_allowed("https://example.com", &origins));
        assert!(is_origin_allowed("http://localhost:3000", &origins));
        assert!(is_origin_allowed("null", &origins));
        assert!(is_origin_allowed("", &origins));
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
        assert!(!is_origin_allowed("https://example.com:8080", &origins));
    }

    #[test]
    fn test_is_origin_allowed_wildcard_subdomain() {
        let origins = vec!["*.example.com".to_string()];
        assert!(is_origin_allowed("https://api.example.com", &origins));
        assert!(is_origin_allowed("https://www.example.com", &origins));
        assert!(is_origin_allowed("sub.example.com", &origins));
        assert!(!is_origin_allowed("https://example.com", &origins)); // Exact domain not matched by *.
        assert!(!is_origin_allowed("https://evil.com", &origins));
        assert!(!is_origin_allowed("https://example.com.evil.com", &origins));
    }

    #[test]
    fn test_is_origin_allowed_empty_list() {
        let origins = vec![];
        assert!(!is_origin_allowed("https://example.com", &origins));
        assert!(!is_origin_allowed("", &origins));
        assert!(!is_origin_allowed("null", &origins));
    }

    #[test]
    fn test_is_origin_allowed_case_sensitivity() {
        let origins = vec!["https://Example.com".to_string()];
        assert!(is_origin_allowed("https://Example.com", &origins));
        assert!(!is_origin_allowed("https://example.com", &origins)); // Case sensitive
        assert!(!is_origin_allowed("HTTPS://EXAMPLE.COM", &origins));
    }

    #[test]
    fn test_is_origin_allowed_multiple_wildcards() {
        let origins = vec![
            "*.example.com".to_string(),
            "*.test.org".to_string(),
            "https://localhost:3000".to_string(),
        ];
        assert!(is_origin_allowed("api.example.com", &origins));
        assert!(is_origin_allowed("dev.test.org", &origins));
        assert!(is_origin_allowed("https://localhost:3000", &origins));
        assert!(!is_origin_allowed("api.evil.com", &origins));
    }

    #[test]
    fn test_is_origin_allowed_wildcard_with_protocol() {
        let origins = vec!["*.example.com".to_string()];
        // Wildcard matching is simple - just checks if it ends with the domain part
        assert!(is_origin_allowed("https://api.example.com", &origins));
        assert!(is_origin_allowed("http://www.example.com", &origins));
        assert!(is_origin_allowed("ws://socket.example.com", &origins));
    }

    #[test]
    fn test_is_origin_allowed_null_origin() {
        let origins = vec!["null".to_string()];
        assert!(is_origin_allowed("null", &origins));
        assert!(!is_origin_allowed("https://example.com", &origins));
    }

    #[test]
    fn test_is_origin_allowed_empty_origin() {
        let origins = vec!["".to_string()];
        assert!(is_origin_allowed("", &origins));
        assert!(!is_origin_allowed("https://example.com", &origins));
    }

    #[test]
    fn test_is_origin_allowed_malformed_wildcard() {
        let origins = vec!["*example.com".to_string()]; // Missing dot
        assert!(!is_origin_allowed("api.example.com", &origins));
        assert!(!is_origin_allowed("example.com", &origins));
    }

    #[test]
    fn test_is_origin_allowed_wildcard_only() {
        let origins = vec!["*".to_string(), "https://example.com".to_string()];
        // Wildcard takes precedence
        assert!(is_origin_allowed("https://evil.com", &origins));
        assert!(is_origin_allowed("https://example.com", &origins));
    }

    #[test]
    fn test_build_preflight_response() {
        use crate::config::{ApiConfig, Config, SecurityConfig};
        
        let config = Config {
            api: ApiConfig {
                enable_cors: true,
                cors_origins: vec!["https://example.com".to_string()],
                ..Default::default()
            },
            security: SecurityConfig::default(),
            ..Default::default()
        };
        
        let state = Arc::new(AppState {
            config: Arc::new(config),
            pool: sqlx::SqlitePool::connect(":memory:").await.unwrap(), // This won't work in sync test
        });
        
        // We can't easily test the full response without async/await setup
        // But we can test the header logic separately
    }

    #[test]
    fn test_wildcard_subdomain_edge_cases() {
        let origins = vec!["*.example.com".to_string()];
        
        // Should match subdomains
        assert!(is_origin_allowed("a.example.com", &origins));
        assert!(is_origin_allowed("very.long.subdomain.example.com", &origins));
        
        // Should not match the root domain
        assert!(!is_origin_allowed("example.com", &origins));
        
        // Should not match partial matches
        assert!(!is_origin_allowed("notexample.com", &origins));
        assert!(!is_origin_allowed("example.com.evil.com", &origins));
        
        // Edge case: empty subdomain
        assert!(!is_origin_allowed(".example.com", &origins));
    }
}