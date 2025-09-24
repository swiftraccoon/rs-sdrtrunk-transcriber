//! Custom extractors for request processing

pub mod pagination;
pub mod api_key;
pub mod validated_json;

use axum::{
    async_trait,
    extract::{FromRequestParts, Request},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::fmt;

/// Custom error type for extractors
#[derive(Debug)]
pub struct ExtractorError {
    /// Error message
    pub message: String,
    /// HTTP status code
    pub status: StatusCode,
    /// Error code for API responses
    pub code: String,
}

impl ExtractorError {
    /// Create a new extractor error
    pub fn new(message: impl Into<String>, status: StatusCode, code: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status,
            code: code.into(),
        }
    }
    
    /// Create a bad request error
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(message, StatusCode::BAD_REQUEST, "BAD_REQUEST")
    }
    
    /// Create an unauthorized error
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(message, StatusCode::UNAUTHORIZED, "UNAUTHORIZED")
    }
    
    /// Create an internal server error
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new(message, StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR")
    }
}

impl fmt::Display for ExtractorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ExtractorError {}

/// Error response for extractors
#[derive(Debug, Serialize)]
pub struct ExtractorErrorResponse {
    /// Error message
    pub error: String,
    /// Error code
    pub code: String,
    /// Additional context
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for ExtractorError {
    fn into_response(self) -> Response {
        let response = ExtractorErrorResponse {
            error: self.message,
            code: self.code,
            details: None,
        };
        
        (self.status, Json(response)).into_response()
    }
}

/// Extractor for client information
#[derive(Debug, Clone)]
pub struct ClientInfo {
    /// Client IP address
    pub ip: String,
    /// User agent string
    pub user_agent: Option<String>,
    /// Request ID for tracing
    pub request_id: String,
}

#[async_trait]
impl<S> FromRequestParts<S> for ClientInfo
where
    S: Send + Sync,
{
    type Rejection = ExtractorError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let headers = &parts.headers;
        
        // Extract IP address
        let ip = if let Some(forwarded) = headers.get("X-Forwarded-For") {
            forwarded
                .to_str()
                .ok()
                .and_then(|s| s.split(',').next())
                .unwrap_or("unknown")
                .trim()
                .to_string()
        } else if let Some(real_ip) = headers.get("X-Real-IP") {
            real_ip.to_str().unwrap_or("unknown").to_string()
        } else {
            // Try to get from connection info if available
            parts
                .extensions
                .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
                .map(|info| info.0.ip().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        };
        
        // Extract user agent
        let user_agent = headers
            .get("User-Agent")
            .and_then(|ua| ua.to_str().ok())
            .map(String::from);
        
        // Generate or extract request ID
        let request_id = headers
            .get("X-Request-ID")
            .and_then(|id| id.to_str().ok())
            .map(String::from)
            .unwrap_or_else(|| generate_request_id());
        
        Ok(ClientInfo {
            ip,
            user_agent,
            request_id,
        })
    }
}

/// Extractor for current user (from authenticated API key)
#[derive(Debug, Clone)]
pub struct CurrentUser {
    /// API key information
    pub api_key: sdrtrunk_database::models::ApiKeyDb,
}

#[async_trait]
impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
{
    type Rejection = ExtractorError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Get API key from request extensions (added by auth middleware)
        let api_key = parts
            .extensions
            .get::<sdrtrunk_database::models::ApiKeyDb>()
            .cloned()
            .ok_or_else(|| {
                ExtractorError::unauthorized("Authentication required")
            })?;
        
        Ok(CurrentUser { api_key })
    }
}

/// Extractor for optional current user (doesn't fail if not authenticated)
#[derive(Debug, Clone)]
pub struct OptionalCurrentUser(pub Option<CurrentUser>);

#[async_trait]
impl<S> FromRequestParts<S> for OptionalCurrentUser
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match CurrentUser::from_request_parts(parts, state).await {
            Ok(user) => Ok(OptionalCurrentUser(Some(user))),
            Err(_) => Ok(OptionalCurrentUser(None)),
        }
    }
}

/// Generate a unique request ID
fn generate_request_id() -> String {
    format!("req_{}", &uuid::Uuid::new_v4().to_string()[..16])
}

/// Extractor for request timing
#[derive(Debug, Clone)]
pub struct RequestTiming {
    /// Request start time
    pub start_time: std::time::Instant,
}

impl RequestTiming {
    /// Get elapsed time since request start
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
    
    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed().as_millis() as u64
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for RequestTiming
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Try to get timing from extensions (added by middleware)
        let start_time = parts
            .extensions
            .get::<std::time::Instant>()
            .copied()
            .unwrap_or_else(std::time::Instant::now);
        
        Ok(RequestTiming { start_time })
    }
}

/// Extractor combinator for multiple extractors
#[derive(Debug, Clone)]
pub struct ExtractorPair<T, U>(pub T, pub U);

#[async_trait]
impl<S, T, U> FromRequestParts<S> for ExtractorPair<T, U>
where
    S: Send + Sync,
    T: FromRequestParts<S> + Send,
    U: FromRequestParts<S> + Send,
{
    type Rejection = ExtractorError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let first = T::from_request_parts(parts, state)
            .await
            .map_err(|_| ExtractorError::bad_request("Failed to extract first component"))?;
        
        let second = U::from_request_parts(parts, state)
            .await
            .map_err(|_| ExtractorError::bad_request("Failed to extract second component"))?;
        
        Ok(ExtractorPair(first, second))
    }
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    fn create_test_parts_with_headers(headers: HeaderMap) -> Parts {
        let mut request = axum::http::Request::builder();
        for (name, value) in headers.iter() {
            request = request.header(name, value);
        }
        let request = request.body(()).unwrap();
        let (parts, _) = request.into_parts();
        parts
    }

    #[tokio::test]
    async fn test_client_info_extractor_with_forwarded_header() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", HeaderValue::from_static("192.168.1.1, 10.0.0.1"));
        headers.insert("User-Agent", HeaderValue::from_static("TestAgent/1.0"));
        
        let mut parts = create_test_parts_with_headers(headers);
        let client_info = ClientInfo::from_request_parts(&mut parts, &()).await.unwrap();
        
        assert_eq!(client_info.ip, "192.168.1.1");
        assert_eq!(client_info.user_agent, Some("TestAgent/1.0".to_string()));
        assert!(client_info.request_id.starts_with("req_"));
    }

    #[tokio::test]
    async fn test_client_info_extractor_with_real_ip_header() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Real-IP", HeaderValue::from_static("203.0.113.1"));
        
        let mut parts = create_test_parts_with_headers(headers);
        let client_info = ClientInfo::from_request_parts(&mut parts, &()).await.unwrap();
        
        assert_eq!(client_info.ip, "203.0.113.1");
        assert_eq!(client_info.user_agent, None);
    }

    #[tokio::test]
    async fn test_client_info_extractor_unknown_ip() {
        let headers = HeaderMap::new();
        
        let mut parts = create_test_parts_with_headers(headers);
        let client_info = ClientInfo::from_request_parts(&mut parts, &()).await.unwrap();
        
        assert_eq!(client_info.ip, "unknown");
        assert_eq!(client_info.user_agent, None);
    }

    #[tokio::test]
    async fn test_current_user_extractor_missing_auth() {
        let headers = HeaderMap::new();
        let mut parts = create_test_parts_with_headers(headers);
        
        let result = CurrentUser::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert_eq!(error.status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_optional_current_user_extractor_missing_auth() {
        let headers = HeaderMap::new();
        let mut parts = create_test_parts_with_headers(headers);
        
        let result = OptionalCurrentUser::from_request_parts(&mut parts, &()).await.unwrap();
        assert!(result.0.is_none());
    }

    #[test]
    fn test_generate_request_id() {
        let id1 = generate_request_id();
        let id2 = generate_request_id();
        
        assert!(id1.starts_with("req_"));
        assert!(id2.starts_with("req_"));
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 20); // "req_" + 16 hex chars
    }

    #[tokio::test]
    async fn test_request_timing_extractor() {
        let headers = HeaderMap::new();
        let mut parts = create_test_parts_with_headers(headers);
        
        let timing = RequestTiming::from_request_parts(&mut parts, &()).await.unwrap();
        
        // Should be very recent
        assert!(timing.elapsed_ms() < 100);
    }

    #[test]
    fn test_extractor_error_creation() {
        let error = ExtractorError::new("Test message", StatusCode::BAD_REQUEST, "TEST_CODE");
        assert_eq!(error.message, "Test message");
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.code, "TEST_CODE");
    }

    #[test]
    fn test_extractor_error_helpers() {
        let bad_request = ExtractorError::bad_request("Bad request");
        assert_eq!(bad_request.status, StatusCode::BAD_REQUEST);
        assert_eq!(bad_request.code, "BAD_REQUEST");
        
        let unauthorized = ExtractorError::unauthorized("Unauthorized");
        assert_eq!(unauthorized.status, StatusCode::UNAUTHORIZED);
        assert_eq!(unauthorized.code, "UNAUTHORIZED");
        
        let internal = ExtractorError::internal_error("Internal error");
        assert_eq!(internal.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(internal.code, "INTERNAL_ERROR");
    }

    #[test]
    fn test_extractor_error_display() {
        let error = ExtractorError::new("Test message", StatusCode::BAD_REQUEST, "TEST_CODE");
        let display_str = format!("{}", error);
        assert_eq!(display_str, "TEST_CODE: Test message");
    }

    #[test]
    fn test_extractor_error_response() {
        let error = ExtractorError::bad_request("Invalid input");
        let response = error.into_response();
        
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_client_info_with_custom_request_id() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Request-ID", HeaderValue::from_static("custom-req-123"));
        headers.insert("User-Agent", HeaderValue::from_static("CustomAgent/2.0"));
        
        let mut parts = create_test_parts_with_headers(headers);
        let client_info = ClientInfo::from_request_parts(&mut parts, &()).await.unwrap();
        
        assert_eq!(client_info.request_id, "custom-req-123");
        assert_eq!(client_info.user_agent, Some("CustomAgent/2.0".to_string()));
    }

    #[tokio::test]
    async fn test_client_info_with_invalid_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", HeaderValue::from_bytes(&[0xFF, 0xFE]).unwrap());
        headers.insert("User-Agent", HeaderValue::from_bytes(&[0xFF, 0xFE]).unwrap());
        
        let mut parts = create_test_parts_with_headers(headers);
        let client_info = ClientInfo::from_request_parts(&mut parts, &()).await.unwrap();
        
        assert_eq!(client_info.ip, "unknown");
        assert_eq!(client_info.user_agent, None);
    }

    #[tokio::test]
    async fn test_client_info_forwarded_with_whitespace() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", HeaderValue::from_static("  192.168.1.5  , 10.0.0.2"));
        
        let mut parts = create_test_parts_with_headers(headers);
        let client_info = ClientInfo::from_request_parts(&mut parts, &()).await.unwrap();
        
        assert_eq!(client_info.ip, "192.168.1.5");
    }

    #[tokio::test]
    async fn test_client_info_empty_forwarded_header() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", HeaderValue::from_static(""));
        
        let mut parts = create_test_parts_with_headers(headers);
        let client_info = ClientInfo::from_request_parts(&mut parts, &()).await.unwrap();
        
        assert_eq!(client_info.ip, "unknown");
    }

    #[tokio::test]
    async fn test_current_user_with_api_key() {
        let headers = HeaderMap::new();
        let mut parts = create_test_parts_with_headers(headers);
        
        // Add mock API key to extensions
        let api_key = sdrtrunk_database::models::ApiKeyDb {
            id: "test-user".to_string(),
            key_hash: "hash123".to_string(),
            description: Some("Test User".to_string()),
            created_at: chrono::Utc::now(),
            expires_at: None,
            allowed_ips: None,
            allowed_systems: None,
            active: true,
            last_used: None,
            total_requests: None,
        };
        parts.extensions.insert(api_key.clone());
        
        let current_user = CurrentUser::from_request_parts(&mut parts, &()).await.unwrap();
        assert_eq!(current_user.api_key.id, "test-user");
    }

    #[tokio::test]
    async fn test_optional_current_user_with_api_key() {
        let headers = HeaderMap::new();
        let mut parts = create_test_parts_with_headers(headers);
        
        let api_key = sdrtrunk_database::models::ApiKeyDb {
            id: "test-user".to_string(),
            key_hash: "hash123".to_string(),
            description: Some("Test User".to_string()),
            created_at: chrono::Utc::now(),
            expires_at: None,
            allowed_ips: None,
            allowed_systems: None,
            active: true,
            last_used: None,
            total_requests: None,
        };
        parts.extensions.insert(api_key.clone());
        
        let optional_user = OptionalCurrentUser::from_request_parts(&mut parts, &()).await.unwrap();
        assert!(optional_user.0.is_some());
        assert_eq!(optional_user.0.unwrap().api_key.id, "test-user");
    }

    #[test]
    fn test_request_timing_methods() {
        let timing = RequestTiming {
            start_time: std::time::Instant::now() - std::time::Duration::from_millis(100)
        };
        
        let elapsed = timing.elapsed();
        assert!(elapsed.as_millis() >= 90); // Account for timing variations
        
        let elapsed_ms = timing.elapsed_ms();
        assert!(elapsed_ms >= 90);
    }

    #[tokio::test]
    async fn test_request_timing_with_extension() {
        let headers = HeaderMap::new();
        let mut parts = create_test_parts_with_headers(headers);
        
        // Add a custom start time to extensions
        let custom_start = std::time::Instant::now() - std::time::Duration::from_millis(50);
        parts.extensions.insert(custom_start);
        
        let timing = RequestTiming::from_request_parts(&mut parts, &()).await.unwrap();
        
        // Should use the custom start time from extensions
        assert!(timing.elapsed_ms() >= 40);
    }

    #[test]
    fn test_generate_request_id_uniqueness() {
        let mut ids = std::collections::HashSet::new();
        for _ in 0..1000 {
            let id = generate_request_id();
            assert!(id.starts_with("req_"));
            assert_eq!(id.len(), 20);
            assert!(ids.insert(id)); // Should all be unique
        }
    }

    #[test]
    fn test_extractor_error_response_serialization() {
        let response = ExtractorErrorResponse {
            error: "Test error".to_string(),
            code: "TEST_CODE".to_string(),
            details: Some(serde_json::json!({"field": "value"})),
        };
        
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("Test error"));
        assert!(json.contains("TEST_CODE"));
        assert!(json.contains("field"));
    }

    #[tokio::test]
    async fn test_extractor_pair_success() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Forwarded-For", HeaderValue::from_static("192.168.1.1"));
        
        let mut parts = create_test_parts_with_headers(headers);
        
        // This should successfully extract both ClientInfo and RequestTiming
        let pair: Result<ExtractorPair<ClientInfo, RequestTiming>, _> = 
            ExtractorPair::from_request_parts(&mut parts, &()).await;
        
        assert!(pair.is_ok());
        let ExtractorPair(client_info, timing) = pair.unwrap();
        assert_eq!(client_info.ip, "192.168.1.1");
        assert!(timing.elapsed_ms() < 100);
    }

    #[tokio::test]
    async fn test_extractor_pair_first_fails() {
        let headers = HeaderMap::new();
        let mut parts = create_test_parts_with_headers(headers);
        
        // This should fail because CurrentUser requires authentication
        let pair: Result<ExtractorPair<CurrentUser, RequestTiming>, _> = 
            ExtractorPair::from_request_parts(&mut parts, &()).await;
        
        assert!(pair.is_err());
    }
}