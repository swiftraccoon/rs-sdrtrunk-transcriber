//! Middleware for authentication, rate limiting, and request processing

pub mod auth;
pub mod rate_limit;
pub mod logging;
pub mod cors;

use axum::{http::StatusCode, response::Json};
use serde::Serialize;

/// Standard error response for middleware
#[derive(Debug, Serialize)]
pub struct MiddlewareError {
    /// Error message
    pub error: String,
    /// Error code
    pub code: String,
    /// Request ID for tracing
    pub request_id: Option<String>,
}

impl MiddlewareError {
    /// Create a new middleware error
    pub fn new(error: &str, code: &str) -> Self {
        Self {
            error: error.to_string(),
            code: code.to_string(),
            request_id: None,
        }
    }
    
    /// Create a middleware error with request ID
    pub fn with_request_id(error: &str, code: &str, request_id: String) -> Self {
        Self {
            error: error.to_string(),
            code: code.to_string(),
            request_id: Some(request_id),
        }
    }
}

/// Convert middleware error to HTTP response
impl From<MiddlewareError> for (StatusCode, Json<MiddlewareError>) {
    fn from(error: MiddlewareError) -> Self {
        let status = match error.code.as_str() {
            "UNAUTHORIZED" | "INVALID_API_KEY" => StatusCode::UNAUTHORIZED,
            "FORBIDDEN" | "IP_BLOCKED" => StatusCode::FORBIDDEN,
            "RATE_LIMITED" => StatusCode::TOO_MANY_REQUESTS,
            "INVALID_REQUEST" | "MISSING_HEADER" => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(error))
    }
}