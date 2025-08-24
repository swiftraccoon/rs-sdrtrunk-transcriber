//! Authentication middleware for API key validation

use crate::{middleware::MiddlewareError, state::AppState};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use sdrtrunk_database::queries;
use std::sync::Arc;
use tracing::{debug, error, warn};

/// API key header name
const API_KEY_HEADER: &str = "X-API-Key";
/// Alternative header name (commonly used)
const AUTH_HEADER: &str = "Authorization";

/// Authentication middleware that validates API keys
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, (StatusCode, axum::Json<MiddlewareError>)> {
    // Skip authentication for health check endpoints
    let path = request.uri().path();
    if path.starts_with("/health") || path.starts_with("/ready") {
        return Ok(next.run(request).await);
    }
    
    // Skip authentication if not required
    if !state.config.security.require_api_key {
        debug!("API key authentication disabled, skipping validation");
        return Ok(next.run(request).await);
    }
    
    // Extract API key from headers
    let api_key = extract_api_key(&headers)?;
    
    // Validate API key
    let api_key_info = validate_api_key(&state, &api_key).await?;
    
    // Check IP restrictions if configured
    if let Some(allowed_ips) = &api_key_info.allowed_ips {
        let client_ip = get_client_ip(&headers, &request);
        if !allowed_ips.contains(&client_ip) {
            warn!("API key {} used from unauthorized IP: {}", api_key_info.id, client_ip);
            return Err(MiddlewareError::new(
                "API key not authorized for this IP address",
                "IP_NOT_AUTHORIZED"
            ).into());
        }
    }
    
    // Add API key info to request extensions
    request.extensions_mut().insert(api_key_info);
    
    debug!("Authentication successful for API key: {}", api_key);
    Ok(next.run(request).await)
}

/// Extract API key from request headers
fn extract_api_key(headers: &HeaderMap) -> Result<String, (StatusCode, axum::Json<MiddlewareError>)> {
    // Try X-API-Key header first
    if let Some(api_key) = headers.get(API_KEY_HEADER) {
        return api_key
            .to_str()
            .map(String::from)
            .map_err(|_| {
                MiddlewareError::new(
                    "Invalid API key format in X-API-Key header",
                    "INVALID_API_KEY_FORMAT"
                ).into()
            });
    }
    
    // Try Authorization header with Bearer scheme
    if let Some(auth_header) = headers.get(AUTH_HEADER) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(bearer_token) = auth_str.strip_prefix("Bearer ") {
                return Ok(bearer_token.to_string());
            }
        }
        return Err(MiddlewareError::new(
            "Invalid Authorization header format. Use 'Bearer <token>'",
            "INVALID_AUTH_HEADER"
        ).into());
    }
    
    Err(MiddlewareError::new(
        "API key required. Provide via X-API-Key header or Authorization: Bearer <token>",
        "MISSING_API_KEY"
    ).into())
}

/// Validate API key against database
async fn validate_api_key(
    state: &Arc<AppState>,
    api_key: &str,
) -> Result<sdrtrunk_database::models::ApiKeyDb, (StatusCode, axum::Json<MiddlewareError>)> {
    // Hash the API key for database lookup
    let key_hash = format!("{:x}", md5::compute(api_key));
    
    match queries::validate_api_key(&state.pool, &key_hash).await {
        Ok(Some(api_key_info)) => {
            // Check if key is active
            if !api_key_info.active {
                warn!("Inactive API key used: {}", api_key_info.id);
                return Err(MiddlewareError::new(
                    "API key is inactive",
                    "INACTIVE_API_KEY"
                ).into());
            }
            
            // Check if key has expired
            if let Some(expires_at) = api_key_info.expires_at {
                if expires_at < chrono::Utc::now() {
                    warn!("Expired API key used: {}", api_key_info.id);
                    return Err(MiddlewareError::new(
                        "API key has expired",
                        "EXPIRED_API_KEY"
                    ).into());
                }
            }
            
            debug!("Valid API key authenticated: {}", api_key_info.id);
            Ok(api_key_info)
        }
        Ok(None) => {
            warn!("Invalid API key attempted: {}", &api_key[..8.min(api_key.len())]);
            Err(MiddlewareError::new(
                "Invalid API key",
                "INVALID_API_KEY"
            ).into())
        }
        Err(e) => {
            error!("Database error during API key validation: {}", e);
            Err(MiddlewareError::new(
                "Failed to validate API key",
                "VALIDATION_ERROR"
            ).into())
        }
    }
}

/// Extract client IP from request headers and connection info
fn get_client_ip(headers: &HeaderMap, request: &Request) -> String {
    // Try X-Forwarded-For header (for proxies)
    if let Some(forwarded) = headers.get("X-Forwarded-For") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // Take the first IP in case of multiple proxies
            if let Some(first_ip) = forwarded_str.split(',').next() {
                return first_ip.trim().to_string();
            }
        }
    }
    
    // Try X-Real-IP header (nginx)
    if let Some(real_ip) = headers.get("X-Real-IP") {
        if let Ok(ip_str) = real_ip.to_str() {
            return ip_str.to_string();
        }
    }
    
    // Fall back to connection remote address
    request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|connect_info| connect_info.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}


/// Optional authentication middleware (allows unauthenticated requests)
pub async fn optional_auth_middleware(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Response {
    // Try to authenticate if API key is provided
    if let Ok(api_key) = extract_api_key(&headers) {
        if let Ok(api_key_info) = validate_api_key(&state, &api_key).await {
            request.extensions_mut().insert(api_key_info);
        }
    }
    
    next.run(request).await
}

/// No-op authentication middleware (allows all requests)
pub async fn no_auth_middleware(
    request: Request,
    next: Next,
) -> Response {
    // Simply pass through - no authentication
    next.run(request).await
}

/// Simplified optional authentication middleware (stateless)
pub async fn simplified_optional_auth_middleware(
    request: Request,
    next: Next,
) -> Response {
    // For now, just pass through - no authentication
    next.run(request).await
}

/// Authentication layer builder (simplified - no auth for now)
pub fn auth_layer() -> impl tower::Layer<axum::routing::Route> + Clone {
    axum::middleware::from_fn(no_auth_middleware)
}

/// Optional authentication layer builder (simplified)
pub fn optional_auth_layer() -> impl tower::Layer<axum::routing::Route> + Clone {
    axum::middleware::from_fn(simplified_optional_auth_middleware)
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_extract_api_key_from_x_api_key_header() {
        let mut headers = HeaderMap::new();
        headers.insert(API_KEY_HEADER, HeaderValue::from_static("test-key-123"));
        
        let result = extract_api_key(&headers).unwrap();
        assert_eq!(result, "test-key-123");
    }

    #[test]
    fn test_extract_api_key_from_authorization_header() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTH_HEADER, HeaderValue::from_static("Bearer test-key-456"));
        
        let result = extract_api_key(&headers).unwrap();
        assert_eq!(result, "test-key-456");
    }

    #[test]
    fn test_extract_api_key_missing() {
        let headers = HeaderMap::new();
        
        let result = extract_api_key(&headers);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_api_key_invalid_x_api_key_format() {
        let mut headers = HeaderMap::new();
        headers.insert(API_KEY_HEADER, HeaderValue::from_bytes(&[0xFF, 0xFE]).unwrap());
        
        let result = extract_api_key(&headers);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_api_key_invalid_authorization_format() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTH_HEADER, HeaderValue::from_static("Basic dGVzdA=="));
        
        let result = extract_api_key(&headers);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_api_key_malformed_authorization() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTH_HEADER, HeaderValue::from_static("Bearer"));
        
        let result = extract_api_key(&headers);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_api_key_prefers_x_api_key() {
        let mut headers = HeaderMap::new();
        headers.insert(API_KEY_HEADER, HeaderValue::from_static("x-api-key-value"));
        headers.insert(AUTH_HEADER, HeaderValue::from_static("Bearer bearer-value"));
        
        let result = extract_api_key(&headers).unwrap();
        assert_eq!(result, "x-api-key-value");
    }

    #[test]
    fn test_get_client_ip_from_forwarded_header() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", HeaderValue::from_static("192.168.1.1, 10.0.0.1"));
        
        let request = Request::builder().body(()).unwrap();
        let ip = get_client_ip(&headers, &request);
        assert_eq!(ip, "192.168.1.1");
    }

    #[test]
    fn test_get_client_ip_from_real_ip_header() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Real-IP", HeaderValue::from_static("10.0.0.5"));
        
        let request = Request::builder().body(()).unwrap();
        let ip = get_client_ip(&headers, &request);
        assert_eq!(ip, "10.0.0.5");
    }

    #[test]
    fn test_get_client_ip_no_headers() {
        let headers = HeaderMap::new();
        let request = Request::builder().body(()).unwrap();
        
        let ip = get_client_ip(&headers, &request);
        assert_eq!(ip, "unknown");
    }

    #[test]
    fn test_get_client_ip_invalid_forwarded_header() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", HeaderValue::from_bytes(&[0xFF, 0xFE]).unwrap());
        
        let request = Request::builder().body(()).unwrap();
        let ip = get_client_ip(&headers, &request);
        assert_eq!(ip, "unknown");
    }

    #[test]
    fn test_get_client_ip_empty_forwarded_header() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", HeaderValue::from_static(""));
        
        let request = Request::builder().body(()).unwrap();
        let ip = get_client_ip(&headers, &request);
        assert_eq!(ip, "unknown");
    }

    #[test]
    fn test_get_client_ip_whitespace_in_forwarded() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", HeaderValue::from_static("  192.168.1.100  , 10.0.0.1"));
        
        let request = Request::builder().body(()).unwrap();
        let ip = get_client_ip(&headers, &request);
        assert_eq!(ip, "192.168.1.100");
    }

    #[test]
    fn test_get_client_ip_prefers_forwarded_over_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", HeaderValue::from_static("192.168.1.1"));
        headers.insert("X-Real-IP", HeaderValue::from_static("10.0.0.1"));
        
        let request = Request::builder().body(()).unwrap();
        let ip = get_client_ip(&headers, &request);
        assert_eq!(ip, "192.168.1.1");
    }

    #[test]
    fn test_get_client_ip_invalid_real_ip_header() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Real-IP", HeaderValue::from_bytes(&[0xFF, 0xFE]).unwrap());
        
        let request = Request::builder().body(()).unwrap();
        let ip = get_client_ip(&headers, &request);
        assert_eq!(ip, "unknown");
    }
}