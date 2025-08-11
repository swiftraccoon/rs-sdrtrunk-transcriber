//! Rate limiting middleware using token bucket algorithm

use crate::{middleware::MiddlewareError, state::AppState};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use governor::{
    clock::{QuantaClock, QuantaInstant},
    middleware::NoOpMiddleware,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use std::{sync::Arc, time::Duration};
use tracing::{debug, warn};

/// Rate limiter type alias
type AppRateLimiter = RateLimiter<NotKeyed, InMemoryState, QuantaClock, NoOpMiddleware<QuantaInstant>>;

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, (StatusCode, axum::Json<MiddlewareError>)> {
    let path = request.uri().path();
    
    // Skip rate limiting for health endpoints
    if path.starts_with("/health") || path.starts_with("/ready") {
        return Ok(next.run(request).await);
    }
    
    // Get or create rate limiter for client
    let client_key = get_client_key(&headers, &request);
    let rate_limiter = get_rate_limiter(&state, &client_key).await;
    
    // Check rate limit
    match rate_limiter.check() {
        Ok(_) => {
            debug!("Rate limit check passed for client: {}", client_key);
            
            // Add rate limit headers to response
            let mut response = next.run(request).await;
            add_rate_limit_headers(&mut response, &rate_limiter);
            
            Ok(response)
        }
        Err(negative) => {
            warn!("Rate limit exceeded for client: {} (retry after: {:?})", 
                  client_key, negative.wait_time_from(QuantaClock::default().now()));
            
            let wait_time = negative.wait_time_from(QuantaClock::default().now());
            let retry_after_seconds = wait_time.as_secs();
            
            let mut error = MiddlewareError::new(
                &format!("Rate limit exceeded. Try again in {} seconds", retry_after_seconds),
                "RATE_LIMITED"
            );
            error.request_id = Some(generate_request_id());
            
            Err((StatusCode::TOO_MANY_REQUESTS, axum::Json(error)))
        }
    }
}

/// Get client identification key for rate limiting
fn get_client_key(headers: &HeaderMap, request: &Request) -> String {
    // Priority order:
    // 1. API key (if authenticated)
    // 2. Client IP address
    
    // Check for API key in extensions (added by auth middleware)
    if let Some(api_key) = request.extensions().get::<sdrtrunk_database::models::ApiKeyDb>() {
        return format!("api_key:{}", api_key.id);
    }
    
    // Fall back to IP address
    let ip = get_client_ip(headers, request);
    format!("ip:{}", ip)
}

/// Get client IP from request headers and connection info
fn get_client_ip(headers: &HeaderMap, request: &Request) -> String {
    // Try X-Forwarded-For header (for proxies)
    if let Some(forwarded) = headers.get("X-Forwarded-For") {
        if let Ok(forwarded_str) = forwarded.to_str() {
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

/// Get or create rate limiter for client
async fn get_rate_limiter(state: &Arc<AppState>, client_key: &str) -> Arc<AppRateLimiter> {
    // In a production system, you'd want to use a more sophisticated
    // storage mechanism (Redis, etc.) to share rate limits across instances
    
    // For now, use a simple in-memory approach
    static RATE_LIMITERS: std::sync::LazyLock<dashmap::DashMap<String, Arc<AppRateLimiter>>> = 
        std::sync::LazyLock::new(dashmap::DashMap::new);
    
    RATE_LIMITERS
        .entry(client_key.to_string())
        .or_insert_with(|| {
            // Create quota based on config
            let quota = Quota::per_minute(state.config.api.rate_limit)
                .allow_burst(std::cmp::max(1, state.config.api.rate_limit / 10).try_into().unwrap_or(5));
            
            Arc::new(RateLimiter::direct(quota))
        })
        .clone()
}

/// Add rate limit headers to response
fn add_rate_limit_headers(response: &mut Response, _rate_limiter: &AppRateLimiter) {
    // Standard rate limit headers
    let headers = response.headers_mut();
    
    // Using placeholder values for now
    headers.insert("X-RateLimit-Limit", HeaderValue::from(60u32));
    headers.insert("X-RateLimit-Remaining", HeaderValue::from(59u32));
    
    // Window reset time (simplified - using fixed 60 second window)
    let reset_time = chrono::Utc::now() + chrono::Duration::seconds(60);
    headers.insert("X-RateLimit-Reset", HeaderValue::from(reset_time.timestamp() as u64));
}

/// Generate a unique request ID for tracing
fn generate_request_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:08x}", rng.r#gen::<u32>())
}

/// No-op rate limiting middleware (allows all requests)
pub async fn no_rate_limit_middleware(
    request: Request,
    next: Next,
) -> Response {
    // Simply pass through - no rate limiting for now
    next.run(request).await
}

/// Rate limiting layer builder (simplified - no rate limiting for now)
pub fn rate_limit_layer() -> impl tower::Layer<axum::routing::Route> + Clone {
    axum::middleware::from_fn(no_rate_limit_middleware)
}

/// Strict rate limiting middleware for upload endpoints
pub async fn strict_rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, (StatusCode, axum::Json<MiddlewareError>)> {
    let client_key = get_client_key(&headers, &request);
    
    // Use stricter limits for upload endpoints
    static UPLOAD_RATE_LIMITERS: std::sync::LazyLock<dashmap::DashMap<String, Arc<AppRateLimiter>>> = 
        std::sync::LazyLock::new(dashmap::DashMap::new);
    
    let rate_limiter = UPLOAD_RATE_LIMITERS
        .entry(client_key.clone())
        .or_insert_with(|| {
            // Stricter quota for uploads (e.g., 10 per minute)
            let upload_limit = std::cmp::max(1, state.config.api.rate_limit / 6);
            let quota = Quota::per_minute(upload_limit)
                .allow_burst(std::cmp::max(1, upload_limit / 5).try_into().unwrap_or(2));
            
            Arc::new(RateLimiter::direct(quota))
        })
        .clone();
    
    match rate_limiter.check() {
        Ok(_) => {
            debug!("Upload rate limit check passed for client: {}", client_key);
            
            let mut response = next.run(request).await;
            add_rate_limit_headers(&mut response, &rate_limiter);
            
            Ok(response)
        }
        Err(negative) => {
            warn!("Upload rate limit exceeded for client: {}", client_key);
            
            let wait_time = negative.wait_time_from(QuantaClock::default().now());
            let retry_after_seconds = wait_time.as_secs();
            
            let error = MiddlewareError::new(
                &format!("Upload rate limit exceeded. Try again in {} seconds", retry_after_seconds),
                "UPLOAD_RATE_LIMITED"
            );
            
            Err((StatusCode::TOO_MANY_REQUESTS, axum::Json(error)))
        }
    }
}

/// Strict rate limiting layer builder for upload endpoints (simplified - no rate limiting for now)
pub fn strict_rate_limit_layer() -> impl tower::Layer<axum::routing::Route> + Clone {
    axum::middleware::from_fn(no_rate_limit_middleware)
}

/// Clean up expired rate limiters (should be called periodically)
pub async fn cleanup_rate_limiters() {
    // This is a simplified cleanup - in production you'd want a more 
    // sophisticated approach with TTL and background cleanup
    static LAST_CLEANUP: std::sync::LazyLock<std::sync::Arc<std::sync::Mutex<std::time::Instant>>> = 
        std::sync::LazyLock::new(|| std::sync::Arc::new(std::sync::Mutex::new(std::time::Instant::now())));
    
    let now = std::time::Instant::now();
    let mut last_cleanup = LAST_CLEANUP.lock().unwrap();
    
    // Clean up every 5 minutes
    if now.duration_since(*last_cleanup) > Duration::from_secs(300) {
        // In a real implementation, you'd iterate through stored rate limiters
        // and remove expired ones based on last access time
        debug!("Rate limiter cleanup triggered");
        *last_cleanup = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_get_client_key_with_api_key() {
        let mut headers = HeaderMap::new();
        let mut request = Request::builder().body(()).unwrap();
        
        // Mock API key in extensions
        let api_key = sdrtrunk_database::models::ApiKeyDb {
            id: "test-key".to_string(),
            key_hash: "hash".to_string(),
            description: None,
            created_at: chrono::Utc::now(),
            expires_at: None,
            allowed_ips: None,
            allowed_systems: None,
            active: true,
            last_used: None,
            total_requests: None,
        };
        request.extensions_mut().insert(api_key);
        
        let key = get_client_key(&headers, &request);
        assert_eq!(key, "api_key:test-key");
    }

    #[test]
    fn test_get_client_key_with_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", HeaderValue::from_static("192.168.1.100"));
        
        let request = Request::builder().body(()).unwrap();
        
        let key = get_client_key(&headers, &request);
        assert_eq!(key, "ip:192.168.1.100");
    }

    #[test]
    fn test_generate_request_id() {
        let id1 = generate_request_id();
        let id2 = generate_request_id();
        
        assert_eq!(id1.len(), 8); // 4 bytes = 8 hex chars
        assert_ne!(id1, id2); // Should be different
    }
}