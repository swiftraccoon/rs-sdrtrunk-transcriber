//! API proxy handlers for communicating with backend

use crate::{api_client::ListCallsQuery, state::AppState};
use axum::extract::ws::{Message, WebSocket};
use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{Json, Response},
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};

/// API endpoint for calls data - proxies to backend API
pub async fn api_calls(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListCallsQuery>,
) -> Json<serde_json::Value> {
    match state.api_client.get_calls(&params).await {
        Ok(calls) => Json(calls),
        Err(e) => {
            error!("Failed to fetch calls from API: {}", e);
            Json(serde_json::json!({
                "error": "Failed to fetch calls",
                "message": e.to_string(),
                "calls": [],
                "total": 0,
                "count": 0,
                "offset": 0
            }))
        }
    }
}

/// API endpoint for global statistics
pub async fn api_global_stats(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    match state.api_client.get_global_stats().await {
        Ok(global_stats) => Json(global_stats),
        Err(e) => {
            error!("Failed to fetch global stats from API: {}", e);
            Json(serde_json::json!({
                "error": "Failed to fetch statistics",
                "message": e.to_string(),
                "total_calls": 0,
                "calls_today": 0,
                "success_rate": 100.0
            }))
        }
    }
}

/// WebSocket handler for real-time updates
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(move |socket| websocket_connection(socket, state))
}

/// Handle WebSocket connection for real-time updates
#[allow(clippy::cognitive_complexity)]
async fn websocket_connection(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    info!("WebSocket connection established");

    // Send periodic updates (frontend handles incrementally to avoid scroll resets)
    let mut update_interval = interval(Duration::from_secs(10));
    let mut ping_interval = interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            _ = update_interval.tick() => {
                // Fetch latest calls and send update (only completed transcriptions)
                if let Ok(calls) = state.api_client.get_calls(&ListCallsQuery {
                    limit: Some(20),
                    offset: Some(0),
                    system_id: None,
                    talkgroup_id: None,
                    transcription_status: Some("completed".to_string()),
                    from_date: None,
                    to_date: None,
                    sort: Some("desc".to_string()),
                    include_transcription: Some(true),
                }).await {
                    let update = serde_json::json!({
                        "type": "calls_update",
                        "data": calls
                    });

                    if sender.send(Message::Text(update.to_string())).await.is_err() {
                        break;
                    }
                }
            }
            _ = ping_interval.tick() => {
                if sender.send(Message::Ping(vec![])).await.is_err() {
                    break;
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) => break,
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {},
                }
            }
        }
    }

    info!("WebSocket connection closed");
}

/// Serve audio file for a specific call
///
/// # Errors
///
/// Returns `StatusCode::NOT_FOUND` if the call or audio file is not found.
/// Returns `StatusCode::INTERNAL_SERVER_ERROR` for database or file system errors.
#[allow(clippy::cognitive_complexity)]
pub async fn serve_audio(
    Path(call_id): Path<uuid::Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, StatusCode> {
    info!("Audio request for call: {}", call_id);

    // Get call details from API to find audio file path
    let call_data = match state.api_client.get_call_details(call_id).await {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to get call details: {}", e);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    // Extract audio file path
    let audio_filename = call_data
        .get("audio_filename")
        .and_then(|f| f.as_str())
        .ok_or_else(|| {
            warn!("No audio filename for call {}", call_id);
            StatusCode::NOT_FOUND
        })?;

    // Construct file path based on config
    let storage_base = &state.config.storage.base_dir;
    let upload_dir = &state.config.storage.upload_dir;
    let audio_path = storage_base.join(upload_dir).join(audio_filename);

    info!("Serving audio file: {:?}", audio_path);

    // Check if file exists
    if !audio_path.exists() {
        warn!("Audio file not found: {:?}", audio_path);
        return Err(StatusCode::NOT_FOUND);
    }

    // Read and serve the file
    let file_contents = tokio::fs::read(&audio_path).await.map_err(|e| {
        error!("Failed to read audio file {:?}: {}", audio_path, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Build response with proper headers
    let response = Response::builder()
        .header("Content-Type", "audio/mpeg")
        .header("Content-Length", file_contents.len())
        .header("Accept-Ranges", "bytes")
        .body(file_contents.into())
        .map_err(|e| {
            error!("Failed to build response: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(response)
}

/// Health check endpoint
pub async fn health_check() -> &'static str {
    "OK"
}
