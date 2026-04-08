//! Performance monitoring middleware with timing instrumentation

use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::{info, warn};

/// Performance thresholds for warnings (in milliseconds)
const WARN_THRESHOLD_MS: u128 = 1000;
const CRITICAL_THRESHOLD_MS: u128 = 5000;

/// Performance monitoring middleware
///
/// Tracks request timing and logs slow requests
pub async fn performance_middleware(
    request: Request,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path().to_string();

    // Process the request
    let response = next.run(request).await;

    // Calculate elapsed time
    let elapsed = start.elapsed();
    let elapsed_ms = elapsed.as_millis();

    // Log based on timing thresholds
    let status = response.status();

    if elapsed_ms >= CRITICAL_THRESHOLD_MS {
        warn!(
            method = %method,
            path = %path,
            status = %status,
            elapsed_ms = %elapsed_ms,
            "CRITICAL: Very slow request"
        );
    } else if elapsed_ms >= WARN_THRESHOLD_MS {
        warn!(
            method = %method,
            path = %path,
            status = %status,
            elapsed_ms = %elapsed_ms,
            "WARNING: Slow request"
        );
    } else {
        info!(
            method = %method,
            path = %path,
            status = %status,
            elapsed_ms = %elapsed_ms,
            "Request completed"
        );
    }

    response
}

/// Database query timing helper
#[derive(Debug)]
pub struct QueryTimer {
    start: Instant,
    query_name: String,
}

impl QueryTimer {
    /// Create a new query timer
    #[must_use]
    pub fn new(query_name: impl Into<String>) -> Self {
        Self {
            start: Instant::now(),
            query_name: query_name.into(),
        }
    }

    /// Finish timing and log the result
    pub fn finish(self) {
        let elapsed = self.start.elapsed();
        let elapsed_ms = elapsed.as_millis();

        if elapsed_ms >= 100 {
            warn!(
                query = %self.query_name,
                elapsed_ms = %elapsed_ms,
                "Slow database query"
            );
        } else if elapsed_ms >= 50 {
            info!(
                query = %self.query_name,
                elapsed_ms = %elapsed_ms,
                "Database query"
            );
        }
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
    use std::time::Duration;

    #[test]
    fn test_query_timer_creation() {
        let timer = QueryTimer::new("test_query");
        assert_eq!(timer.query_name, "test_query");
    }

    #[tokio::test]
    async fn test_query_timer_measurement() {
        let timer = QueryTimer::new("slow_query");
        tokio::time::sleep(Duration::from_millis(10)).await;
        timer.finish();
        // Should complete without panic
    }
}
