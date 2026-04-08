//! Middleware for authentication, rate limiting, and request processing

pub mod auth;
pub mod rate_limit;
pub mod logging;
pub mod cors;
pub mod performance;

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
    clippy::match_same_arms,
)]
mod tests {
    use super::*;

    #[test]
    fn test_middleware_error_new() {
        let error = MiddlewareError::new("Test error", "TEST_CODE");
        
        assert_eq!(error.error, "Test error");
        assert_eq!(error.code, "TEST_CODE");
        assert!(error.request_id.is_none());
    }

    #[test]
    fn test_middleware_error_with_request_id() {
        let error = MiddlewareError::with_request_id(
            "Test error", 
            "TEST_CODE", 
            "req_123456789".to_string()
        );
        
        assert_eq!(error.error, "Test error");
        assert_eq!(error.code, "TEST_CODE");
        assert_eq!(error.request_id, Some("req_123456789".to_string()));
    }

    #[test]
    fn test_middleware_error_serialization() {
        let error = MiddlewareError::with_request_id(
            "Authentication failed", 
            "UNAUTHORIZED", 
            "req_abc123".to_string()
        );
        
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("Authentication failed"));
        assert!(json.contains("UNAUTHORIZED"));
        assert!(json.contains("req_abc123"));
    }

    #[test]
    fn test_middleware_error_conversion_unauthorized() {
        let error = MiddlewareError::new("Invalid token", "UNAUTHORIZED");
        let (status, _) = error.into();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_middleware_error_conversion_invalid_api_key() {
        let error = MiddlewareError::new("API key invalid", "INVALID_API_KEY");
        let (status, _) = error.into();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_middleware_error_conversion_forbidden() {
        let error = MiddlewareError::new("Access denied", "FORBIDDEN");
        let (status, _) = error.into();
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_middleware_error_conversion_ip_blocked() {
        let error = MiddlewareError::new("IP blocked", "IP_BLOCKED");
        let (status, _) = error.into();
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_middleware_error_conversion_rate_limited() {
        let error = MiddlewareError::new("Too many requests", "RATE_LIMITED");
        let (status, _) = error.into();
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_middleware_error_conversion_bad_request() {
        let error = MiddlewareError::new("Invalid request", "INVALID_REQUEST");
        let (status, _) = error.into();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        
        let error2 = MiddlewareError::new("Header missing", "MISSING_HEADER");
        let (status2, _) = error2.into();
        assert_eq!(status2, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_middleware_error_conversion_internal_error() {
        let error = MiddlewareError::new("Something went wrong", "INTERNAL_ERROR");
        let (status, _) = error.into();
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        
        // Test unknown error code defaults to internal error
        let error2 = MiddlewareError::new("Unknown error", "UNKNOWN_CODE");
        let (status2, _) = error2.into();
        assert_eq!(status2, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_middleware_error_json_response() {
        let error = MiddlewareError::with_request_id(
            "Rate limit exceeded", 
            "RATE_LIMITED", 
            "req_xyz789".to_string()
        );
        
        let (status, json_response) = error.into();
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        
        // The JSON response should contain the error
        // Note: We can't easily inspect the Json<T> content in tests without more setup
    }

    #[test]
    fn test_middleware_error_debug_formatting() {
        let error = MiddlewareError::new("Debug test", "DEBUG_CODE");
        let debug_str = format!("{:?}", error);
        
        assert!(debug_str.contains("Debug test"));
        assert!(debug_str.contains("DEBUG_CODE"));
    }

    #[test]
    fn test_middleware_error_empty_strings() {
        let error = MiddlewareError::new("", "");
        
        assert_eq!(error.error, "");
        assert_eq!(error.code, "");
        assert!(error.request_id.is_none());
        
        let (status, _) = error.into();
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR); // Empty code defaults to internal error
    }

    #[test]
    fn test_all_status_code_mappings() {
        let test_cases = vec![
            ("UNAUTHORIZED", StatusCode::UNAUTHORIZED),
            ("INVALID_API_KEY", StatusCode::UNAUTHORIZED),
            ("FORBIDDEN", StatusCode::FORBIDDEN),
            ("IP_BLOCKED", StatusCode::FORBIDDEN),
            ("RATE_LIMITED", StatusCode::TOO_MANY_REQUESTS),
            ("INVALID_REQUEST", StatusCode::BAD_REQUEST),
            ("MISSING_HEADER", StatusCode::BAD_REQUEST),
            ("UNKNOWN_ERROR_CODE", StatusCode::INTERNAL_SERVER_ERROR),
            ("", StatusCode::INTERNAL_SERVER_ERROR),
        ];
        
        for (code, expected_status) in test_cases {
            let error = MiddlewareError::new("Test message", code);
            let (actual_status, _) = error.into();
            assert_eq!(actual_status, expected_status, "Failed for code: {}", code);
        }
    }

    #[test]
    fn test_middleware_error_case_sensitivity() {
        // Test that error code matching is case sensitive
        let error = MiddlewareError::new("Test", "unauthorized"); // lowercase
        let (status, _) = error.into();
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR); // Should not match UNAUTHORIZED
        
        let error2 = MiddlewareError::new("Test", "UNAUTHORIZED"); // uppercase
        let (status2, _) = error2.into();
        assert_eq!(status2, StatusCode::UNAUTHORIZED); // Should match
    }

    #[test]
    fn test_middleware_error_long_messages() {
        let long_message = "A".repeat(1000);
        let long_code = "B".repeat(100);
        let long_request_id = "req_".to_string() + &"C".repeat(50);
        
        let error = MiddlewareError::with_request_id(&long_message, &long_code, long_request_id.clone());
        
        assert_eq!(error.error, long_message);
        assert_eq!(error.code, long_code);
        assert_eq!(error.request_id, Some(long_request_id));
    }

    #[test]
    fn test_middleware_error_special_characters() {
        let error = MiddlewareError::with_request_id(
            "Error with unicode: 测试 🚀", 
            "SPECIAL_CHARS", 
            "req_测试_🚀".to_string()
        );
        
        assert!(error.error.contains("测试"));
        assert!(error.error.contains("🚀"));
        assert!(error.request_id.as_ref().unwrap().contains("测试"));
        assert!(error.request_id.as_ref().unwrap().contains("🚀"));
        
        // Should serialize without issues
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("测试"));
    }
}