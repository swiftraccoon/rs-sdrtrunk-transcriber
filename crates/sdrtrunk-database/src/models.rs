//! Database models for `SDRTrunk` transcriber

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Database model for radio calls
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RadioCallDb {
    /// Unique identifier
    pub id: Uuid,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Call timestamp
    pub call_timestamp: DateTime<Utc>,

    /// System ID
    pub system_id: String,

    /// System label
    pub system_label: Option<String>,

    /// Frequency in Hz
    pub frequency: Option<i64>,

    /// Talkgroup ID
    pub talkgroup_id: Option<i32>,

    /// Talkgroup label
    pub talkgroup_label: Option<String>,

    /// Talkgroup group
    pub talkgroup_group: Option<String>,

    /// Talkgroup tag
    pub talkgroup_tag: Option<String>,

    /// Source radio ID
    pub source_radio_id: Option<i32>,

    /// Talker alias
    pub talker_alias: Option<String>,

    /// Audio filename
    pub audio_filename: Option<String>,

    /// Audio file path
    pub audio_file_path: Option<String>,

    /// Audio file size in bytes
    pub audio_size_bytes: Option<i64>,

    /// Audio content type
    pub audio_content_type: Option<String>,

    /// Duration in seconds
    pub duration_seconds: Option<rust_decimal::Decimal>,

    /// Transcription text
    pub transcription_text: Option<String>,

    /// Transcription confidence
    pub transcription_confidence: Option<rust_decimal::Decimal>,

    /// Transcription language
    pub transcription_language: Option<String>,

    /// Transcription status
    pub transcription_status: Option<String>,

    /// Speaker segments (JSON)
    pub speaker_segments: Option<serde_json::Value>,

    /// Number of speakers
    pub speaker_count: Option<i32>,

    /// Patches
    pub patches: Option<String>,

    /// Frequencies
    pub frequencies: Option<String>,

    /// Sources
    pub sources: Option<String>,

    /// Upload IP address
    pub upload_ip: Option<sqlx::types::ipnetwork::IpNetwork>,

    /// Upload timestamp
    pub upload_timestamp: DateTime<Utc>,

    /// API key ID used for upload
    pub upload_api_key_id: Option<String>,
}

/// Database model for upload logs
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UploadLogDb {
    /// Unique identifier
    pub id: Uuid,

    /// Timestamp of the upload
    pub timestamp: DateTime<Utc>,

    /// Client IP address
    pub client_ip: sqlx::types::ipnetwork::IpNetwork,

    /// User agent string
    pub user_agent: Option<String>,

    /// API key used
    pub api_key_used: Option<String>,

    /// System ID
    pub system_id: Option<String>,

    /// Whether the upload was successful
    pub success: bool,

    /// Error message if failed
    pub error_message: Option<String>,

    /// Filename
    pub filename: Option<String>,

    /// File size in bytes
    pub file_size: Option<i64>,

    /// Content type
    pub content_type: Option<String>,

    /// HTTP response code
    pub response_code: Option<i32>,

    /// Processing time in milliseconds
    pub processing_time_ms: Option<rust_decimal::Decimal>,
}

/// Database model for system statistics
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SystemStatsDb {
    /// Unique identifier
    pub id: Uuid,

    /// System ID
    pub system_id: String,

    /// System label
    pub system_label: Option<String>,

    /// Total number of calls
    pub total_calls: Option<i32>,

    /// Calls received today
    pub calls_today: Option<i32>,

    /// Calls received this hour
    pub calls_this_hour: Option<i32>,

    /// First seen timestamp
    pub first_seen: Option<DateTime<Utc>>,

    /// Last seen timestamp
    pub last_seen: Option<DateTime<Utc>>,

    /// Top talkgroups (JSON)
    pub top_talkgroups: Option<serde_json::Value>,

    /// Upload sources (JSON)
    pub upload_sources: Option<serde_json::Value>,

    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
}

/// Database model for API keys
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiKeyDb {
    /// Key ID
    pub id: String,

    /// Hashed key
    pub key_hash: String,

    /// Description
    pub description: Option<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Expiration timestamp
    pub expires_at: Option<DateTime<Utc>>,

    /// Allowed IP addresses
    pub allowed_ips: Option<Vec<String>>,

    /// Allowed systems
    pub allowed_systems: Option<Vec<String>>,

    /// Whether the key is active
    pub active: bool,

    /// Last used timestamp
    pub last_used: Option<DateTime<Utc>>,

    /// Total number of requests
    pub total_requests: Option<i32>,
}
