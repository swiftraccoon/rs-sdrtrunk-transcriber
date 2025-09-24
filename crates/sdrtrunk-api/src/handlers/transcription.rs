//! Transcription webhook callback handler

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::state::AppState;
use std::sync::Arc;
use sdrtrunk_database::queries::{RadioCallQueries, TranscriptionUpdate};

/// Webhook callback payload from WhisperX service
#[derive(Debug, Deserialize)]
pub struct TranscriptionCallback {
    pub request_id: Uuid,
    pub call_id: Uuid,
    pub status: String,
    pub text: Option<String>,
    pub language: Option<String>,
    pub confidence: Option<f32>,
    pub processing_time_ms: u64,
    pub segments: Option<Vec<serde_json::Value>>,
    pub speaker_segments: Option<Vec<serde_json::Value>>,
    pub speaker_count: Option<usize>,
    pub words: Option<Vec<serde_json::Value>>,
    pub error: Option<String>,
    pub completed_at: String,
}

/// Response to webhook callback
#[derive(Debug, Serialize)]
pub struct CallbackResponse {
    pub status: String,
    pub message: String,
}

/// Handle transcription completion webhook from WhisperX
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
    let speaker_segments_json = payload
        .speaker_segments
        .as_ref()
        .and_then(|segments| {
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
        Ok(_) => {
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
                    message: format!("Failed to update database: {}", e),
                }),
            )
        }
    }
}

/// Health check endpoint for transcription service
pub async fn transcription_health() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "service": "transcription_callback",
        "version": "1.0.0"
    }))
}