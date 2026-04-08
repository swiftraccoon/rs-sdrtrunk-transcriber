//! WebSocket handler for real-time updates

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

use crate::state::AppState;

/// WebSocket event types that can be broadcast to clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketEvent {
    /// New call received
    #[serde(rename = "new_call")]
    NewCall {
        /// Call ID
        call_id: uuid::Uuid,
        /// System ID
        system_id: String,
        /// Talkgroup ID
        talkgroup_id: Option<i32>,
        /// Timestamp
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Call transcription status update
    #[serde(rename = "transcription_update")]
    TranscriptionUpdate {
        /// Call ID
        call_id: uuid::Uuid,
        /// New status
        status: String,
        /// Confidence score (if completed)
        confidence: Option<f32>,
    },
    /// System status update
    #[serde(rename = "system_status")]
    SystemStatus {
        /// System ID
        system_id: String,
        /// Status
        status: String,
        /// Last activity timestamp
        last_activity: chrono::DateTime<chrono::Utc>,
    },
    /// Statistics update
    #[serde(rename = "stats_update")]
    StatsUpdate {
        /// System ID (if system-specific)
        system_id: Option<String>,
        /// Total calls
        total_calls: i64,
        /// Calls in last hour
        calls_last_hour: i32,
    },
}

/// WebSocket handler
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket, _state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    // Create a broadcast channel for this connection
    let (_tx, mut rx) = broadcast::channel::<WebSocketEvent>(100);

    info!("WebSocket client connected");

    // Send initial connection confirmation
    let welcome = WebSocketEvent::SystemStatus {
        system_id: "system".to_string(),
        status: "connected".to_string(),
        last_activity: chrono::Utc::now(),
    };

    if let Ok(json) = serde_json::to_string(&welcome) {
        let _ = sender.send(Message::Text(json)).await;
    }

    // Spawn task to handle incoming messages from client
    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&event)
                && sender.send(Message::Text(json)).await.is_err()
            {
                break;
            }
        }
    });

    // Spawn task to receive messages from client (for ping/pong)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    info!("Received WebSocket message: {}", text);
                }
                Message::Close(_) => {
                    info!("WebSocket client disconnected");
                    break;
                }
                Message::Ping(_data) => {
                    // Respond to ping with pong
                    info!("Received ping");
                    // The sender is moved, so we can't respond here
                    // Axum handles ping/pong automatically
                }
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => {
            recv_task.abort();
        },
        _ = (&mut recv_task) => {
            send_task.abort();
        },
    }

    info!("WebSocket connection closed");
}

/// Broadcast an event to all connected WebSocket clients
///
/// This would be called from upload handler, transcription callback, etc.
#[allow(clippy::unused_async)]
pub async fn broadcast_event(_event: WebSocketEvent) {
    // TODO: Implement global broadcast channel
    // For now, this is a placeholder that would need to be wired
    // to a global Arc<broadcast::Sender> stored in AppState
    warn!("WebSocket broadcasting not yet fully wired - event not sent");
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
    fn test_websocket_event_serialization() {
        let event = WebSocketEvent::NewCall {
            call_id: uuid::Uuid::new_v4(),
            system_id: "test_system".to_string(),
            talkgroup_id: Some(12345),
            timestamp: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("new_call"));
        assert!(json.contains("test_system"));
    }

    #[test]
    fn test_transcription_update_event() {
        let event = WebSocketEvent::TranscriptionUpdate {
            call_id: uuid::Uuid::new_v4(),
            status: "completed".to_string(),
            confidence: Some(0.95),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("transcription_update"));
        assert!(json.contains("completed"));
        assert!(json.contains("0.95"));
    }

    #[test]
    fn test_system_status_event() {
        let event = WebSocketEvent::SystemStatus {
            system_id: "metro_pd".to_string(),
            status: "online".to_string(),
            last_activity: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("system_status"));
        assert!(json.contains("metro_pd"));
        assert!(json.contains("online"));
    }

    #[test]
    fn test_stats_update_event() {
        let event = WebSocketEvent::StatsUpdate {
            system_id: Some("test_system".to_string()),
            total_calls: 1000,
            calls_last_hour: 42,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("stats_update"));
        assert!(json.contains("1000"));
        assert!(json.contains("42"));
    }

    #[test]
    fn test_event_deserialization() {
        let json = r#"{"type":"new_call","call_id":"550e8400-e29b-41d4-a716-446655440000","system_id":"test","talkgroup_id":123,"timestamp":"2024-01-01T00:00:00Z"}"#;

        let event: WebSocketEvent = serde_json::from_str(json).unwrap();

        if let WebSocketEvent::NewCall {
            system_id,
            talkgroup_id,
            ..
        } = event
        {
            assert_eq!(system_id, "test");
            assert_eq!(talkgroup_id, Some(123));
        } else {
            panic!("Wrong event type");
        }
    }
}
