//! Transcription webhook callback handler

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::state::AppState;
use sdrtrunk_storage::queries::{RadioCallQueries, TranscriptionUpdate};
use std::sync::Arc;

/// Webhook callback payload from `WhisperX` service
#[derive(Debug, Deserialize)]
pub struct TranscriptionCallback {
    /// Unique identifier for the transcription request
    pub request_id: Uuid,
    /// Identifier of the radio call being transcribed
    pub call_id: Uuid,
    /// Transcription status (completed, failed, etc.)
    pub status: String,
    /// Transcribed text output
    pub text: Option<String>,
    /// Detected language code
    pub language: Option<String>,
    /// Confidence score for the transcription
    pub confidence: Option<f32>,
    /// Time taken to process in milliseconds
    pub processing_time_ms: u64,
    /// Raw transcription segments
    pub segments: Option<Vec<serde_json::Value>>,
    /// Speaker-attributed segments
    pub speaker_segments: Option<Vec<serde_json::Value>>,
    /// Number of unique speakers detected
    pub speaker_count: Option<usize>,
    /// Word-level alignment data
    pub words: Option<Vec<serde_json::Value>>,
    /// Error message if transcription failed
    pub error: Option<String>,
    /// ISO 8601 timestamp when transcription completed
    pub completed_at: String,
}

/// Response to webhook callback
#[derive(Debug, Serialize)]
pub struct CallbackResponse {
    /// Status of the callback processing
    pub status: String,
    /// Human-readable status message
    pub message: String,
}

/// Handle transcription completion webhook from `WhisperX`
#[allow(
    clippy::cognitive_complexity,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]
pub async fn transcription_callback(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TranscriptionCallback>,
) -> impl IntoResponse {
    info!(
        "Received transcription callback for call {} with status: {}",
        payload.call_id, payload.status
    );

    // Determine database status
    let db_status = match payload.status.as_str() {
        "completed" => "completed",
        "failed" => "failed",
        _ => {
            warn!("Unknown transcription status: {}", payload.status);
            "failed"
        }
    };

    // Prepare speaker segments JSON if present
    let speaker_segments_json = payload.speaker_segments.as_ref().and_then(|segments| {
        if segments.is_empty() {
            None
        } else {
            Some(serde_json::Value::Array(segments.clone()))
        }
    });

    // Update database with transcription result
    let update_result = RadioCallQueries::update_transcription_status(
        &state.pool,
        TranscriptionUpdate {
            id: payload.call_id,
            status: db_status,
            text: payload.text.as_deref(),
            confidence: payload.confidence,
            error: payload.error.as_deref(),
            speaker_segments: speaker_segments_json.as_ref(),
            speaker_count: payload.speaker_count.map(|c| c as i32),
        },
    )
    .await;

    match update_result {
        Ok(()) => {
            info!(
                "Successfully updated transcription for call {} in database",
                payload.call_id
            );

            // Log transcription summary
            if let Some(text) = &payload.text {
                let preview = if text.len() > 100 {
                    format!("{}...", &text[..100])
                } else {
                    text.clone()
                };
                info!(
                    "Transcription for call {}: {} (confidence: {:.2}%, {} speakers, {} ms)",
                    payload.call_id,
                    preview,
                    payload.confidence.unwrap_or(0.0) * 100.0,
                    payload.speaker_count.unwrap_or(0),
                    payload.processing_time_ms
                );
            }

            (
                StatusCode::OK,
                Json(CallbackResponse {
                    status: "success".to_string(),
                    message: format!("Transcription updated for call {}", payload.call_id),
                }),
            )
        }
        Err(e) => {
            error!(
                "Failed to update transcription for call {} in database: {}",
                payload.call_id, e
            );

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CallbackResponse {
                    status: "error".to_string(),
                    message: format!("Failed to update database: {e}"),
                }),
            )
        }
    }
}

/// Health check endpoint for transcription service
#[allow(clippy::unused_async)]
pub async fn transcription_health() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "service": "transcription_callback",
        "version": "1.0.0"
    }))
}
