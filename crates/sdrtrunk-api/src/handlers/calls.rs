//! Call listing and retrieval endpoints

use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;
use validator::Validate;

/// Query parameters for listing calls
#[derive(Debug, Deserialize, Validate)]
pub struct ListCallsQuery {
    /// Number of calls to return (max 1000)
    #[validate(range(min = 1, max = 1000))]
    pub limit: Option<i64>,

    /// Offset for pagination
    #[validate(range(min = 0))]
    pub offset: Option<i64>,

    /// Filter by system ID
    #[validate(length(max = 50))]
    pub system_id: Option<String>,

    /// Filter by talkgroup ID
    pub talkgroup_id: Option<i32>,

    /// Filter calls from this date (ISO 8601 format)
    pub from_date: Option<chrono::DateTime<chrono::Utc>>,

    /// Filter calls to this date (ISO 8601 format)
    pub to_date: Option<chrono::DateTime<chrono::Utc>>,

    /// Sort order (desc, asc)
    #[validate(custom(function = "validate_sort_order"))]
    pub sort: Option<String>,

    /// Include transcription data in response
    pub include_transcription: Option<bool>,
}

/// Response for listing calls
#[derive(Debug, Serialize)]
pub struct ListCallsResponse {
    /// List of radio calls
    pub calls: Vec<CallSummary>,

    /// Total number of calls matching filters
    pub total: i64,

    /// Number of calls returned
    pub count: i64,

    /// Current offset
    pub offset: i64,

    /// Pagination info
    pub pagination: PaginationInfo,
}

/// Pagination information
#[derive(Debug, Serialize)]
pub struct PaginationInfo {
    /// Whether there are more results
    pub has_next: bool,

    /// Whether there are previous results  
    pub has_prev: bool,

    /// Next page offset
    pub next_offset: Option<i64>,

    /// Previous page offset
    pub prev_offset: Option<i64>,
}

/// Simplified call information for listings
#[derive(Debug, Serialize)]
pub struct CallSummary {
    /// Call ID
    pub id: Uuid,

    /// Call timestamp
    pub call_timestamp: chrono::DateTime<chrono::Utc>,

    /// System information
    pub system_id: String,
    pub system_label: Option<String>,

    /// Talkgroup information
    pub talkgroup_id: Option<i32>,
    pub talkgroup_label: Option<String>,
    pub talkgroup_group: Option<String>,
    pub talkgroup_tag: Option<String>,

    /// Radio information
    pub source_radio_id: Option<i32>,
    pub talker_alias: Option<String>,

    /// Audio information
    pub audio_filename: Option<String>,
    pub audio_size_bytes: Option<i64>,
    pub duration_seconds: Option<rust_decimal::Decimal>,

    /// Transcription status
    pub transcription_status: Option<String>,
    pub transcription_confidence: Option<rust_decimal::Decimal>,

    /// Transcription text (only if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcription_text: Option<String>,

    /// Frequency
    pub frequency: Option<i64>,
}

/// Detailed call information
#[derive(Debug, Serialize)]
pub struct CallDetail {
    /// Call ID
    pub id: Uuid,

    /// Creation and call timestamps
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub call_timestamp: chrono::DateTime<chrono::Utc>,

    /// System information
    pub system_id: String,
    pub system_label: Option<String>,

    /// Talkgroup information
    pub talkgroup_id: Option<i32>,
    pub talkgroup_label: Option<String>,
    pub talkgroup_group: Option<String>,
    pub talkgroup_tag: Option<String>,

    /// Radio information
    pub source_radio_id: Option<i32>,
    pub talker_alias: Option<String>,

    /// Audio information
    pub audio_filename: Option<String>,
    pub audio_file_path: Option<String>,
    pub audio_size_bytes: Option<i64>,
    pub audio_content_type: Option<String>,
    pub duration_seconds: Option<rust_decimal::Decimal>,

    /// Transcription information
    pub transcription_text: Option<String>,
    pub transcription_confidence: Option<rust_decimal::Decimal>,
    pub transcription_language: Option<String>,
    pub transcription_status: Option<String>,
    pub speaker_segments: Option<serde_json::Value>,
    pub speaker_count: Option<i32>,

    /// Technical details
    pub frequency: Option<i64>,
    pub patches: Option<String>,
    pub frequencies: Option<String>,
    pub sources: Option<String>,

    /// Upload information
    pub upload_timestamp: chrono::DateTime<chrono::Utc>,
    pub upload_ip: Option<sqlx::types::ipnetwork::IpNetwork>,
    pub upload_api_key_id: Option<String>,
}

/// Error response structure
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,
    /// Error code  
    pub code: String,
    /// Additional details
    pub details: Option<serde_json::Value>,
}

/// List radio calls with filtering and pagination
pub async fn list_calls(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListCallsQuery>,
) -> Result<Json<ListCallsResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate query parameters
    if let Err(validation_errors) = query.validate() {
        warn!("Invalid query parameters: {:?}", validation_errors);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid query parameters".to_string(),
                code: "INVALID_PARAMETERS".to_string(),
                details: Some(serde_json::json!(validation_errors)),
            }),
        ));
    }

    let limit = query.limit.unwrap_or(50).min(1000); // Default 50, max 1000
    let offset = query.offset.unwrap_or(0);
    let include_transcription = query.include_transcription.unwrap_or(false);

    info!(
        "Listing calls: limit={}, offset={}, system_id={:?}",
        limit, offset, query.system_id
    );

    // Build query with filters
    let filter = sdrtrunk_database::RadioCallFilter {
        system_id: query.system_id.as_deref(),
        talkgroup_id: query.talkgroup_id,
        from_date: query.from_date,
        to_date: query.to_date,
        limit,
        offset,
    };
    let calls = match sdrtrunk_database::list_radio_calls_filtered(&state.pool, filter).await {
        Ok(calls) => calls,
        Err(e) => {
            error!("Failed to list calls: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to retrieve calls".to_string(),
                    code: "DATABASE_ERROR".to_string(),
                    details: None,
                }),
            ));
        }
    };

    // Get total count for pagination
    let filter = sdrtrunk_database::RadioCallFilter {
        system_id: query.system_id.as_deref(),
        talkgroup_id: query.talkgroup_id,
        from_date: query.from_date,
        to_date: query.to_date,
        limit: 0,  // Not used for count
        offset: 0, // Not used for count
    };
    let total = match sdrtrunk_database::count_radio_calls_filtered(&state.pool, filter).await {
        Ok(count) => count,
        Err(e) => {
            warn!("Failed to get total count: {}", e);
            calls.len() as i64 // Fallback to current result count
        }
    };

    // Convert to summary format
    let call_summaries: Vec<CallSummary> = calls
        .into_iter()
        .map(|call| CallSummary {
            id: call.id,
            call_timestamp: call.call_timestamp,
            system_id: call.system_id,
            system_label: call.system_label,
            talkgroup_id: call.talkgroup_id,
            talkgroup_label: call.talkgroup_label,
            talkgroup_group: call.talkgroup_group,
            talkgroup_tag: call.talkgroup_tag,
            source_radio_id: call.source_radio_id,
            talker_alias: call.talker_alias,
            audio_filename: call.audio_filename,
            audio_size_bytes: call.audio_size_bytes,
            duration_seconds: call.duration_seconds,
            transcription_status: call.transcription_status,
            transcription_confidence: call.transcription_confidence,
            transcription_text: if include_transcription {
                call.transcription_text
            } else {
                None
            },
            frequency: call.frequency,
        })
        .collect();

    let count = call_summaries.len() as i64;

    // Calculate pagination info
    let pagination = PaginationInfo {
        has_next: offset + limit < total,
        has_prev: offset > 0,
        next_offset: if offset + limit < total {
            Some(offset + limit)
        } else {
            None
        },
        prev_offset: if offset > 0 {
            Some((offset - limit).max(0))
        } else {
            None
        },
    };

    let response = ListCallsResponse {
        calls: call_summaries,
        total,
        count,
        offset,
        pagination,
    };

    info!("Returned {} calls out of {} total", count, total);
    Ok(Json(response))
}

/// Get a specific call by ID
pub async fn get_call(
    State(state): State<Arc<AppState>>,
    Path(call_id): Path<Uuid>,
) -> Result<Json<CallDetail>, (StatusCode, Json<ErrorResponse>)> {
    info!("Retrieving call: {}", call_id);

    let call = match sdrtrunk_database::get_radio_call(&state.pool, call_id).await {
        Ok(Some(call)) => call,
        Ok(None) => {
            info!("Call not found: {}", call_id);
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Call {call_id} not found"),
                    code: "CALL_NOT_FOUND".to_string(),
                    details: None,
                }),
            ));
        }
        Err(e) => {
            error!("Failed to retrieve call {}: {}", call_id, e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to retrieve call".to_string(),
                    code: "DATABASE_ERROR".to_string(),
                    details: None,
                }),
            ));
        }
    };

    // Convert to detailed format
    let call_detail = CallDetail {
        id: call.id,
        created_at: call.created_at,
        call_timestamp: call.call_timestamp,
        system_id: call.system_id,
        system_label: call.system_label,
        talkgroup_id: call.talkgroup_id,
        talkgroup_label: call.talkgroup_label,
        talkgroup_group: call.talkgroup_group,
        talkgroup_tag: call.talkgroup_tag,
        source_radio_id: call.source_radio_id,
        talker_alias: call.talker_alias,
        audio_filename: call.audio_filename,
        audio_file_path: call.audio_file_path,
        audio_size_bytes: call.audio_size_bytes,
        audio_content_type: call.audio_content_type,
        duration_seconds: call.duration_seconds,
        transcription_text: call.transcription_text,
        transcription_confidence: call.transcription_confidence,
        transcription_language: call.transcription_language,
        transcription_status: call.transcription_status,
        speaker_segments: call.speaker_segments,
        speaker_count: call.speaker_count,
        frequency: call.frequency,
        patches: call.patches,
        frequencies: call.frequencies,
        sources: call.sources,
        upload_timestamp: call.upload_timestamp,
        upload_ip: call.upload_ip,
        upload_api_key_id: call.upload_api_key_id,
    };

    info!("Successfully retrieved call: {}", call_id);
    Ok(Json(call_detail))
}

/// Validation function for sort order
fn validate_sort_order(sort: &str) -> Result<(), validator::ValidationError> {
    match sort {
        "asc" | "desc" => Ok(()),
        _ => Err(validator::ValidationError::new("invalid_sort_order")),
    }
}

/* Get call audio file (streaming endpoint) - Disabled for minimal build
pub async fn get_call_audio(
    State(state): State<Arc<AppState>>,
    Path(call_id): Path<Uuid>,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    info!("Retrieving audio for call: {}", call_id);

    // First, get the call to find the audio file path
    let call = match sdrtrunk_database::get_radio_call(&state.pool, call_id).await {
        Ok(Some(call)) => call,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Call {call_id} not found"),
                    code: "CALL_NOT_FOUND".to_string(),
                    details: None,
                }),
            ));
        }
        Err(e) => {
            error!("Failed to retrieve call {}: {}", call_id, e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to retrieve call".to_string(),
                    code: "DATABASE_ERROR".to_string(),
                    details: None,
                }),
            ));
        }
    };

    // Check if audio file exists
    let audio_path = match call.audio_file_path {
        Some(path) => std::path::PathBuf::from(path),
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "No audio file associated with this call".to_string(),
                    code: "NO_AUDIO_FILE".to_string(),
                    details: None,
                }),
            ));
        }
    };

    if !audio_path.exists() {
        warn!("Audio file does not exist: {:?}", audio_path);
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Audio file not found on disk".to_string(),
                code: "AUDIO_FILE_NOT_FOUND".to_string(),
                details: Some(serde_json::json!({ "path": audio_path.display().to_string() })),
            }),
        ));
    }

    // Serve the file using tower_http::services::ServeFile
    match tower_http::services::ServeFile::new(&audio_path).try_call(
        axum::http::Request::builder()
            .method("GET")
            .uri("/")
            .body(axum::body::Body::empty())
            .unwrap(),
    ).await {
        Ok(response) => Ok(response),
        Err(e) => {
            error!("Failed to serve audio file: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to serve audio file".to_string(),
                    code: "FILE_SERVE_ERROR".to_string(),
                    details: None,
                }),
            ))
        }
    }
}
*/
