//! WebSocket client for real-time updates

use futures_util::{SinkExt, StreamExt};
use sdrtrunk_core::{Error, Result};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

/// WebSocket client for receiving real-time updates
pub struct WebSocketClient {
    url: String,
}

impl WebSocketClient {
    /// Create a new WebSocket client
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }

    /// Connect to the WebSocket server and handle messages
    pub async fn connect(&self) -> Result<()> {
        info!("Connecting to WebSocket at {}", self.url);

        let (ws_stream, _) = connect_async(&self.url)
            .await
            .map_err(|e| Error::Other(format!("WebSocket connection failed: {e}")))?;

        let (mut write, mut read) = ws_stream.split();

        // TODO: Implement message handling loop
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(update) = serde_json::from_str::<WebSocketMessage>(&text) {
                        self.handle_message(update).await;
                    } else {
                        warn!("Failed to parse WebSocket message: {}", text);
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket connection closed");
                    break;
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Handle incoming WebSocket messages
    async fn handle_message(&self, message: WebSocketMessage) {
        match message {
            WebSocketMessage::CallUpdate { call_id, status } => {
                info!("Received call update: {} -> {:?}", call_id, status);
                // TODO: Update UI with call status change
            }
            WebSocketMessage::NewCall { call } => {
                info!("Received new call: {:?}", call);
                // TODO: Add new call to UI
            }
            WebSocketMessage::SystemStatus { system_id, status } => {
                info!("Received system status: {} -> {:?}", system_id, status);
                // TODO: Update system status in UI
            }
        }
    }
}

/// WebSocket message types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    /// Call status update
    #[serde(rename = "call_update")]
    CallUpdate {
        call_id: uuid::Uuid,
        status: CallStatus,
    },
    /// New call received
    #[serde(rename = "new_call")]
    NewCall {
        call: CallData,
    },
    /// System status update
    #[serde(rename = "system_status")]
    SystemStatus {
        system_id: String,
        status: SystemStatus,
    },
}

/// Call status for WebSocket updates
#[derive(Debug, Serialize, Deserialize)]
pub enum CallStatus {
    /// Call received and pending transcription
    Pending,
    /// Transcription in progress
    Processing,
    /// Transcription completed
    Completed,
    /// Transcription failed
    Failed,
}

/// Call data for WebSocket messages
#[derive(Debug, Serialize, Deserialize)]
pub struct CallData {
    pub id: uuid::Uuid,
    pub system_id: String,
    pub talkgroup_id: Option<i32>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// System status for WebSocket updates
#[derive(Debug, Serialize, Deserialize)]
pub enum SystemStatus {
    /// System is online and receiving calls
    Online,
    /// System is offline
    Offline,
    /// System has errors
    Error(String),
}

