//! API proxy handlers for communicating with backend

use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    response::{Json, Response},
    http::{StatusCode, header},
};
use axum::extract::ws::{WebSocket, Message};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{error, info};
use crate::{api_client::ListCallsQuery, state::AppState};

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
pub async fn api_global_stats(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    match state.api_client.get_global_stats().await {
        Ok(stats) => Json(stats),
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
async fn websocket_connection(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    info!("WebSocket connection established");

    // Send periodic updates
    let mut update_interval = interval(Duration::from_secs(5));
    let mut ping_interval = interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            _ = update_interval.tick() => {
                // Fetch latest calls and send update
                if let Ok(calls) = state.api_client.get_calls(&ListCallsQuery {
                    limit: Some(5),
                    offset: Some(0),
                    system_id: None,
                    talkgroup_id: None,
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
                    Some(Ok(Message::Pong(_))) => continue,
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => continue,
                }
            }
        }
    }

    info!("WebSocket connection closed");
}

/// Serve audio file for a specific call
pub async fn serve_audio(
    Path(call_id): Path<uuid::Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, StatusCode> {
    // TODO: Get call details from API to find audio file path
    // For now, return a placeholder response
    info!("Audio request for call: {}", call_id);

    Err(StatusCode::NOT_IMPLEMENTED)
}

/// Health check endpoint
pub async fn health_check() -> &'static str {
    "OK"
}