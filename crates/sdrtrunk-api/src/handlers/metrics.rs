//! Prometheus metrics handler

use axum::{extract::State, http::StatusCode, response::IntoResponse};
use std::sync::Arc;
use tracing::{error, warn};

use crate::state::AppState;

/// Serve Prometheus metrics
///
/// # Errors
///
/// Returns HTTP 500 if database queries fail
pub async fn serve_metrics(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    // Gather metrics from database
    let metrics = match gather_metrics(&state).await {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to gather metrics: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Format as Prometheus exposition format
    let output = format_prometheus_metrics(&metrics);

    Ok((
        [(http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        output,
    ))
}

/// Internal metrics structure
#[derive(Debug, Default)]
struct Metrics {
    total_calls: i64,
    calls_last_24h: i64,
    total_systems: i64,
    transcriptions_pending: i64,
    transcriptions_processing: i64,
    transcriptions_completed: i64,
    transcriptions_failed: i64,
    upload_success_count: i64,
    upload_error_count: i64,
}

/// Gather metrics from database
///
/// # Errors
///
/// This function currently does not propagate errors but returns a placeholder
/// `Result` to support future error handling.
async fn gather_metrics(state: &AppState) -> Result<Metrics, String> {
    let pool = &state.pool;

    // Execute all metrics queries in parallel
    let (total_calls, recent_calls, systems, pending, processing, completed, failed) = tokio::join!(
        sdrtrunk_storage::count_radio_calls(pool),
        sdrtrunk_storage::count_recent_calls(pool, 24),
        sdrtrunk_storage::count_systems(pool),
        count_calls_by_status(pool, "pending"),
        count_calls_by_status(pool, "processing"),
        count_calls_by_status(pool, "completed"),
        count_calls_by_status(pool, "failed"),
    );

    // Log warnings for failed queries but continue with available data
    let total_calls = total_calls.unwrap_or_else(|e| {
        warn!("Failed to get total_calls metric: {}", e);
        0
    });

    let calls_last_24h = recent_calls.unwrap_or_else(|e| {
        warn!("Failed to get calls_last_24h metric: {}", e);
        0
    });

    let total_systems = systems.unwrap_or_else(|e| {
        warn!("Failed to get total_systems metric: {}", e);
        0
    });

    let transcriptions_pending = pending.unwrap_or(0);
    let transcriptions_processing = processing.unwrap_or(0);
    let transcriptions_completed = completed.unwrap_or(0);
    let transcriptions_failed = failed.unwrap_or(0);

    // TODO: Add upload log metrics from upload_log table when implemented
    let upload_success_count = 0;
    let upload_error_count = 0;

    Ok(Metrics {
        total_calls,
        calls_last_24h,
        total_systems,
        transcriptions_pending,
        transcriptions_processing,
        transcriptions_completed,
        transcriptions_failed,
        upload_success_count,
        upload_error_count,
    })
}

/// Count calls by transcription status
///
/// # Errors
///
/// Returns an error if the database query fails.
async fn count_calls_by_status(
    pool: &sqlx::PgPool,
    status: &str,
) -> Result<i64, sdrtrunk_types::AppError> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM radio_calls WHERE transcription_status = $1")
            .bind(status)
            .fetch_one(pool)
            .await
            .map_err(|e| sdrtrunk_types::AppError::Database(e.to_string()))?;

    Ok(count)
}

/// Format metrics in Prometheus exposition format
fn format_prometheus_metrics(metrics: &Metrics) -> String {
    format!(
        r#"# HELP sdrtrunk_calls_total Total number of radio calls processed
# TYPE sdrtrunk_calls_total counter
sdrtrunk_calls_total {}

# HELP sdrtrunk_calls_last_24h Number of calls in the last 24 hours
# TYPE sdrtrunk_calls_last_24h gauge
sdrtrunk_calls_last_24h {}

# HELP sdrtrunk_systems_total Total number of systems
# TYPE sdrtrunk_systems_total gauge
sdrtrunk_systems_total {}

# HELP sdrtrunk_transcriptions_total Total transcriptions by status
# TYPE sdrtrunk_transcriptions_total gauge
sdrtrunk_transcriptions_total{{status="pending"}} {}
sdrtrunk_transcriptions_total{{status="processing"}} {}
sdrtrunk_transcriptions_total{{status="completed"}} {}
sdrtrunk_transcriptions_total{{status="failed"}} {}

# HELP sdrtrunk_uploads_total Total uploads by result
# TYPE sdrtrunk_uploads_total counter
sdrtrunk_uploads_total{{result="success"}} {}
sdrtrunk_uploads_total{{result="error"}} {}

# HELP sdrtrunk_info Application information
# TYPE sdrtrunk_info gauge
sdrtrunk_info{{version="0.1.0"}} 1
"#,
        metrics.total_calls,
        metrics.calls_last_24h,
        metrics.total_systems,
        metrics.transcriptions_pending,
        metrics.transcriptions_processing,
        metrics.transcriptions_completed,
        metrics.transcriptions_failed,
        metrics.upload_success_count,
        metrics.upload_error_count,
    )
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
    clippy::match_same_arms
)]
mod tests {
    use super::*;

    #[test]
    fn test_format_prometheus_metrics() {
        let metrics = Metrics {
            total_calls: 1000,
            calls_last_24h: 42,
            total_systems: 5,
            transcriptions_pending: 10,
            transcriptions_processing: 3,
            transcriptions_completed: 900,
            transcriptions_failed: 87,
            upload_success_count: 950,
            upload_error_count: 50,
        };

        let output = format_prometheus_metrics(&metrics);

        assert!(output.contains("sdrtrunk_calls_total 1000"));
        assert!(output.contains("sdrtrunk_calls_last_24h 42"));
        assert!(output.contains("sdrtrunk_systems_total 5"));
        assert!(output.contains(r#"sdrtrunk_transcriptions_total{status="pending"} 10"#));
        assert!(output.contains(r#"sdrtrunk_transcriptions_total{status="completed"} 900"#));
        assert!(output.contains("# HELP"));
        assert!(output.contains("# TYPE"));
    }

    #[test]
    fn test_prometheus_format_structure() {
        let metrics = Metrics::default();
        let output = format_prometheus_metrics(&metrics);

        // Check Prometheus format conventions
        assert!(output.starts_with("# HELP"));
        assert!(output.contains("# TYPE"));
        assert!(output.contains("counter"));
        assert!(output.contains("gauge"));

        // Check all metrics are present
        assert!(output.contains("sdrtrunk_calls_total"));
        assert!(output.contains("sdrtrunk_systems_total"));
        assert!(output.contains("sdrtrunk_transcriptions_total"));
        assert!(output.contains("sdrtrunk_uploads_total"));
        assert!(output.contains("sdrtrunk_info"));
    }

    #[test]
    fn test_metrics_default_values() {
        let metrics = Metrics::default();
        assert_eq!(metrics.total_calls, 0);
        assert_eq!(metrics.total_systems, 0);
        assert_eq!(metrics.transcriptions_pending, 0);
    }
}
