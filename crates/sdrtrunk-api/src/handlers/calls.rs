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
    /// System display name or label
    pub system_label: Option<String>,

    /// Talkgroup information
    pub talkgroup_id: Option<i32>,
    /// Talkgroup display name or label
    pub talkgroup_label: Option<String>,
    /// Talkgroup group classification
    pub talkgroup_group: Option<String>,
    /// Talkgroup tag for categorization
    pub talkgroup_tag: Option<String>,

    /// Radio information
    pub source_radio_id: Option<i32>,
    /// Radio user's alias or call sign
    pub talker_alias: Option<String>,

    /// Audio information
    pub audio_filename: Option<String>,
    /// Size of audio file in bytes
    pub audio_size_bytes: Option<i64>,
    /// Duration of audio recording in seconds
    pub duration_seconds: Option<rust_decimal::Decimal>,

    /// Transcription status
    pub transcription_status: Option<String>,
    /// Confidence score for transcription (0.0-1.0)
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
    /// When the radio call occurred
    pub call_timestamp: chrono::DateTime<chrono::Utc>,

    /// System information
    pub system_id: String,
    /// System display name or label
    pub system_label: Option<String>,

    /// Talkgroup information
    pub talkgroup_id: Option<i32>,
    /// Talkgroup display name or label
    pub talkgroup_label: Option<String>,
    /// Talkgroup group classification
    pub talkgroup_group: Option<String>,
    /// Talkgroup tag for categorization
    pub talkgroup_tag: Option<String>,

    /// Radio information
    pub source_radio_id: Option<i32>,
    /// Radio user's alias or call sign
    pub talker_alias: Option<String>,

    /// Audio information
    pub audio_filename: Option<String>,
    /// Full path to the audio file
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
///
/// This endpoint provides paginated access to radio calls with comprehensive filtering options.
/// Supports filtering by system, talkgroup, date ranges, and optional transcription inclusion.
///
/// # Arguments
///
/// * `state` - Application state containing database pool and configuration
/// * `query` - Query parameters for filtering and pagination
///
/// # Returns
///
/// Returns a paginated list of radio calls matching the specified filters, or an error response
/// for invalid parameters or database failures.
///
/// # Errors
///
/// * `BAD_REQUEST` - Invalid query parameters (validation failures)
/// * `INTERNAL_SERVER_ERROR` - Database query failures
///
/// # Example
///
/// ```text
/// GET /api/calls?system_id=police&limit=50&offset=0&include_transcription=true
/// ```
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

/// Get detailed information for a specific radio call
///
/// Retrieves complete details for a radio call including audio metadata, transcription
/// data, speaker information, and upload details. Returns 404 if the call is not found.
///
/// # Arguments
///
/// * `state` - Application state containing database pool
/// * `call_id` - UUID of the radio call to retrieve
///
/// # Returns
///
/// Returns detailed call information or an error response for non-existent calls
/// or database failures.
///
/// # Errors
///
/// * `NOT_FOUND` - Call with specified ID does not exist
/// * `INTERNAL_SERVER_ERROR` - Database query failure
///
/// # Example
///
/// ```text
/// GET /api/calls/550e8400-e29b-41d4-a716-446655440000
/// ```
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

/// Validates sort order parameter values
///
/// Ensures that sort order is either "asc" (ascending) or "desc" (descending).
/// This function is used by the validator framework for query parameter validation.
///
/// # Arguments
///
/// * `sort` - Sort order string to validate
///
/// # Returns
///
/// Returns `Ok(())` for valid sort orders or `ValidationError` for invalid values.
///
/// # Errors
///
/// Returns a validation error if the sort order is not "asc" or "desc".
fn validate_sort_order(sort: &str) -> Result<(), validator::ValidationError> {
    match sort {
        "asc" | "desc" => Ok(()),
        _ => Err(validator::ValidationError::new("invalid_sort_order")),
    }
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use serde_json;
    use std::str::FromStr;
    use uuid::Uuid;
    use validator::Validate;

    #[test]
    fn test_validate_sort_order() {
        // Valid sort orders
        assert!(validate_sort_order("asc").is_ok());
        assert!(validate_sort_order("desc").is_ok());

        // Invalid sort orders
        assert!(validate_sort_order("invalid").is_err());
        assert!(validate_sort_order("ascending").is_err());
        assert!(validate_sort_order("DESC").is_err()); // Case sensitive
        assert!(validate_sort_order("").is_err());
    }

    #[test]
    fn test_list_calls_query_validation() {
        // Valid query
        let valid_query = ListCallsQuery {
            limit: Some(50),
            offset: Some(0),
            system_id: Some("police".to_string()),
            talkgroup_id: Some(12345),
            from_date: Some(Utc::now() - chrono::Duration::hours(24)),
            to_date: Some(Utc::now()),
            sort: Some("desc".to_string()),
            include_transcription: Some(true),
        };
        assert!(valid_query.validate().is_ok());

        // Invalid limit (too high)
        let invalid_limit = ListCallsQuery {
            limit: Some(2000), // Over max of 1000
            offset: Some(0),
            system_id: None,
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            sort: None,
            include_transcription: None,
        };
        assert!(invalid_limit.validate().is_err());

        // Invalid offset (negative)
        let invalid_offset = ListCallsQuery {
            limit: Some(50),
            offset: Some(-1),
            system_id: None,
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            sort: None,
            include_transcription: None,
        };
        assert!(invalid_offset.validate().is_err());

        // Invalid system_id (too long)
        let invalid_system_id = ListCallsQuery {
            limit: Some(50),
            offset: Some(0),
            system_id: Some("a".repeat(51)), // Over max of 50
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            sort: None,
            include_transcription: None,
        };
        assert!(invalid_system_id.validate().is_err());

        // Invalid sort order
        let invalid_sort = ListCallsQuery {
            limit: Some(50),
            offset: Some(0),
            system_id: None,
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            sort: Some("invalid".to_string()),
            include_transcription: None,
        };
        assert!(invalid_sort.validate().is_err());
    }

    #[test]
    fn test_call_summary_serialization() {
        let call_id = Uuid::new_v4();
        let timestamp = Utc::now();

        let call_summary = CallSummary {
            id: call_id,
            call_timestamp: timestamp,
            system_id: "police".to_string(),
            system_label: Some("Police Department".to_string()),
            talkgroup_id: Some(12345),
            talkgroup_label: Some("Dispatch".to_string()),
            talkgroup_group: Some("Operations".to_string()),
            talkgroup_tag: Some("Emergency".to_string()),
            source_radio_id: Some(9876),
            talker_alias: Some("Unit 123".to_string()),
            audio_filename: Some("call.mp3".to_string()),
            audio_size_bytes: Some(1024000),
            duration_seconds: Some(Decimal::from_str("15.5").unwrap()),
            transcription_status: Some("completed".to_string()),
            transcription_confidence: Some(Decimal::from_str("0.95").unwrap()),
            transcription_text: Some("This is a test call".to_string()),
            frequency: Some(154250000),
        };

        let json = serde_json::to_string(&call_summary).expect("Failed to serialize");
        assert!(json.contains(&call_id.to_string()));
        assert!(json.contains("police"));
        assert!(json.contains("Police Department"));
        assert!(json.contains("12345"));
        assert!(json.contains("This is a test call"));
    }

    #[test]
    fn test_call_summary_without_transcription() {
        let call_id = Uuid::new_v4();
        let timestamp = Utc::now();

        let call_summary = CallSummary {
            id: call_id,
            call_timestamp: timestamp,
            system_id: "fire".to_string(),
            system_label: None,
            talkgroup_id: Some(67890),
            talkgroup_label: None,
            talkgroup_group: None,
            talkgroup_tag: None,
            source_radio_id: None,
            talker_alias: None,
            audio_filename: Some("call.wav".to_string()),
            audio_size_bytes: Some(512000),
            duration_seconds: Some(Decimal::from_str("8.2").unwrap()),
            transcription_status: Some("pending".to_string()),
            transcription_confidence: None,
            transcription_text: None, // Should be omitted from JSON
            frequency: Some(460125000),
        };

        let json = serde_json::to_string(&call_summary).expect("Failed to serialize");
        assert!(json.contains("fire"));
        assert!(json.contains("67890"));
        assert!(!json.contains("transcription_text")); // Should be omitted
    }

    #[test]
    fn test_call_detail_serialization() {
        let call_id = Uuid::new_v4();
        let created_at = Utc::now() - chrono::Duration::minutes(5);
        let call_timestamp = Utc::now();
        let upload_timestamp = created_at;

        let call_detail = CallDetail {
            id: call_id,
            created_at,
            call_timestamp,
            system_id: "ems".to_string(),
            system_label: Some("Emergency Medical Services".to_string()),
            talkgroup_id: Some(54321),
            talkgroup_label: Some("Ambulance Dispatch".to_string()),
            talkgroup_group: Some("Medical".to_string()),
            talkgroup_tag: Some("Priority".to_string()),
            source_radio_id: Some(5432),
            talker_alias: Some("Medic 15".to_string()),
            audio_filename: Some("emergency_call.mp3".to_string()),
            audio_file_path: Some("/storage/ems/2024/01/15/emergency_call.mp3".to_string()),
            audio_size_bytes: Some(2048000),
            audio_content_type: Some("audio/mpeg".to_string()),
            duration_seconds: Some(Decimal::from_str("32.7").unwrap()),
            transcription_text: Some("Medical emergency at Main Street".to_string()),
            transcription_confidence: Some(Decimal::from_str("0.92").unwrap()),
            transcription_language: Some("en-US".to_string()),
            transcription_status: Some("completed".to_string()),
            speaker_segments: Some(
                serde_json::json!({"segments": [{"speaker": 1, "start": 0.0, "end": 32.7}]}),
            ),
            speaker_count: Some(1),
            frequency: Some(462650000),
            patches: Some("patch1,patch2".to_string()),
            frequencies: Some("462650000,462675000".to_string()),
            sources: Some("source1,source2".to_string()),
            upload_timestamp,
            upload_ip: None,
            upload_api_key_id: Some("api-key-123".to_string()),
        };

        let json = serde_json::to_string(&call_detail).expect("Failed to serialize");
        assert!(json.contains("ems"));
        assert!(json.contains("Emergency Medical Services"));
        assert!(json.contains("Medical emergency at Main Street"));
        assert!(json.contains("audio/mpeg"));
        assert!(json.contains("api-key-123"));
    }

    #[test]
    fn test_list_calls_response_serialization() {
        let call_id = Uuid::new_v4();
        let timestamp = Utc::now();

        let call = CallSummary {
            id: call_id,
            call_timestamp: timestamp,
            system_id: "test_system".to_string(),
            system_label: Some("Test System".to_string()),
            talkgroup_id: Some(100),
            talkgroup_label: Some("Test Group".to_string()),
            talkgroup_group: Some("Testing".to_string()),
            talkgroup_tag: Some("Test".to_string()),
            source_radio_id: Some(200),
            talker_alias: Some("Test Unit".to_string()),
            audio_filename: Some("test.mp3".to_string()),
            audio_size_bytes: Some(100000),
            duration_seconds: Some(Decimal::from_str("10.0").unwrap()),
            transcription_status: Some("pending".to_string()),
            transcription_confidence: None,
            transcription_text: None,
            frequency: Some(150000000),
        };

        let response = ListCallsResponse {
            calls: vec![call],
            total: 1,
            count: 1,
            offset: 0,
            pagination: PaginationInfo {
                has_next: false,
                has_prev: false,
                next_offset: None,
                prev_offset: None,
            },
        };

        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert!(json.contains("test_system"));
        assert!(json.contains("\"total\":1"));
        assert!(json.contains("\"count\":1"));
        assert!(json.contains("\"has_next\":false"));
    }

    #[test]
    fn test_pagination_info() {
        // Test pagination with next page
        let pagination = PaginationInfo {
            has_next: true,
            has_prev: false,
            next_offset: Some(50),
            prev_offset: None,
        };

        let json = serde_json::to_string(&pagination).expect("Failed to serialize");
        assert!(json.contains("\"has_next\":true"));
        assert!(json.contains("\"has_prev\":false"));
        assert!(json.contains("\"next_offset\":50"));

        // Test pagination with previous page
        let pagination = PaginationInfo {
            has_next: false,
            has_prev: true,
            next_offset: None,
            prev_offset: Some(0),
        };

        let json = serde_json::to_string(&pagination).expect("Failed to serialize");
        assert!(json.contains("\"has_next\":false"));
        assert!(json.contains("\"has_prev\":true"));
        assert!(json.contains("\"prev_offset\":0"));

        // Test pagination in middle
        let pagination = PaginationInfo {
            has_next: true,
            has_prev: true,
            next_offset: Some(100),
            prev_offset: Some(0),
        };

        assert!(pagination.has_next);
        assert!(pagination.has_prev);
        assert_eq!(pagination.next_offset, Some(100));
        assert_eq!(pagination.prev_offset, Some(0));
    }

    #[test]
    fn test_error_response_serialization() {
        let error = ErrorResponse {
            error: "Call not found".to_string(),
            code: "CALL_NOT_FOUND".to_string(),
            details: Some(serde_json::json!({"call_id": "123e4567-e89b-12d3-a456-426614174000"})),
        };

        let json = serde_json::to_string(&error).expect("Failed to serialize");
        assert!(json.contains("Call not found"));
        assert!(json.contains("CALL_NOT_FOUND"));
        assert!(json.contains("call_id"));
    }

    #[test]
    fn test_query_parameter_defaults() {
        let query = ListCallsQuery {
            limit: None,
            offset: None,
            system_id: None,
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            sort: None,
            include_transcription: None,
        };

        // Should validate OK with all None values
        assert!(query.validate().is_ok());
    }

    #[test]
    fn test_decimal_precision() {
        let call_summary = CallSummary {
            id: Uuid::new_v4(),
            call_timestamp: Utc::now(),
            system_id: "test".to_string(),
            system_label: None,
            talkgroup_id: None,
            talkgroup_label: None,
            talkgroup_group: None,
            talkgroup_tag: None,
            source_radio_id: None,
            talker_alias: None,
            audio_filename: None,
            audio_size_bytes: None,
            duration_seconds: Some(Decimal::from_str("123.456789").unwrap()),
            transcription_status: None,
            transcription_confidence: Some(Decimal::from_str("0.987654321").unwrap()),
            transcription_text: None,
            frequency: None,
        };

        // Should serialize without losing precision
        let json = serde_json::to_string(&call_summary).expect("Failed to serialize");
        assert!(json.contains("123.456789"));
        assert!(json.contains("0.987654321"));
    }

    #[tokio::test]
    async fn test_list_calls_query_edge_cases() {
        // Test with exact limits
        let query_max_limit = ListCallsQuery {
            limit: Some(1000), // Exactly at max
            offset: Some(0),
            system_id: Some("a".repeat(50)), // Exactly at max length
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            sort: None,
            include_transcription: None,
        };
        assert!(query_max_limit.validate().is_ok());

        // Test with boundary values
        let query_min_values = ListCallsQuery {
            limit: Some(1),                   // Minimum valid
            offset: Some(0),                  // Minimum valid
            system_id: Some("x".to_string()), // Minimum length
            talkgroup_id: Some(i32::MIN),     // Test extreme value
            from_date: None,
            to_date: None,
            sort: Some("asc".to_string()),
            include_transcription: Some(false),
        };
        assert!(query_min_values.validate().is_ok());
    }

    #[test]
    fn test_uuid_generation_and_serialization() {
        let call_id = Uuid::new_v4();
        let call_summary = CallSummary {
            id: call_id,
            call_timestamp: Utc::now(),
            system_id: "uuid_test".to_string(),
            system_label: None,
            talkgroup_id: None,
            talkgroup_label: None,
            talkgroup_group: None,
            talkgroup_tag: None,
            source_radio_id: None,
            talker_alias: None,
            audio_filename: None,
            audio_size_bytes: None,
            duration_seconds: None,
            transcription_status: None,
            transcription_confidence: None,
            transcription_text: None,
            frequency: None,
        };

        let json = serde_json::to_string(&call_summary).expect("Failed to serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Failed to parse");

        let serialized_id = parsed["id"].as_str().expect("ID should be string");
        assert_eq!(serialized_id, call_id.to_string());
    }

    #[test]
    fn test_datetime_serialization_format() {
        let timestamp = chrono::DateTime::parse_from_rfc3339("2024-01-15T14:30:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let call_summary = CallSummary {
            id: Uuid::new_v4(),
            call_timestamp: timestamp,
            system_id: "datetime_test".to_string(),
            system_label: None,
            talkgroup_id: None,
            talkgroup_label: None,
            talkgroup_group: None,
            talkgroup_tag: None,
            source_radio_id: None,
            talker_alias: None,
            audio_filename: None,
            audio_size_bytes: None,
            duration_seconds: None,
            transcription_status: None,
            transcription_confidence: None,
            transcription_text: None,
            frequency: None,
        };

        let json = serde_json::to_string(&call_summary).expect("Failed to serialize");
        // Should contain ISO 8601 formatted timestamp
        assert!(json.contains("2024-01-15T14:30:00Z"));
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
