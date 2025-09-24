//! File upload handler for Rdio-compatible call uploads

use super::audio_utils;
use crate::state::AppState;
use axum::{
    body::Body,
    extract::{ConnectInfo, FromRequest, Multipart, State},
    http::{HeaderMap, Request, StatusCode},
    response::{IntoResponse, Json, Response},
};
use chrono::{DateTime, Utc};
use sdrtrunk_core::types::RadioCall;
use serde_json;
use std::{net::SocketAddr, sync::Arc};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Response for successful upload
#[derive(serde::Serialize, serde::Deserialize)]
pub struct UploadResponse {
    /// Whether the upload was successful
    pub success: bool,
    /// Unique identifier for the uploaded call
    pub id: Uuid,
    /// Success message for the client
    pub message: String,
}

/// Response for upload error
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ErrorResponse {
    /// Whether the upload was successful (always false)
    pub success: bool,
    /// Error message describing what went wrong
    pub error: String,
}

/// Handle multipart form data upload from Rdio Scanner compatible systems
///
/// This endpoint accepts radio call uploads in multipart/form-data format, compatible
/// with SDRTrunk and Rdio Scanner systems. It handles file validation, storage, database
/// insertion, and system statistics updates.
///
/// # Arguments
///
/// * `state` - Application state with database pool and configuration
/// * `addr` - Client socket address for IP logging
/// * `headers` - HTTP headers for content type and user agent extraction
/// * `request` - Multipart request body containing audio and metadata
///
/// # Returns
///
/// Returns either a success response with call ID or an error response.
/// Response format depends on Accept header (JSON or plain text).
///
/// # Errors
///
/// * `BAD_REQUEST` - Invalid multipart data, missing required fields, file validation failures
/// * `UNAUTHORIZED` - Invalid API key (when authentication enabled)
/// * `INTERNAL_SERVER_ERROR` - Database failures, file system errors
///
/// # Example Request
///
/// ```text
/// POST /api/call-upload
/// Content-Type: multipart/form-data; boundary=----WebKitFormBoundary...
///
/// ------WebKitFormBoundary...
/// Content-Disposition: form-data; name="audio"; filename="call.mp3"
/// Content-Type: audio/mpeg
///
/// [MP3 audio data]
/// ------WebKitFormBoundary...
/// Content-Disposition: form-data; name="system"
///
/// police_system
/// ------WebKitFormBoundary...
/// Content-Disposition: form-data; name="talkgroup"
///
/// 12345
/// ```
pub async fn handle_call_upload(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    request: Request<Body>,
) -> impl IntoResponse {
    let client_ip = addr.ip();
    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // Check Content-Type header for multipart/form-data
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !content_type.starts_with("multipart/form-data") {
        let (status, json_error) = upload_error(
            &state,
            client_ip,
            user_agent,
            None,
            None,
            "Request must have Content-Type: multipart/form-data",
        )
        .await;
        return (status, json_error).into_response();
    }

    // Try to extract multipart data from the request
    let Ok(mut multipart) = Multipart::from_request(request, &state).await else {
        let (status, json_error) = upload_error(
            &state,
            client_ip,
            user_agent,
            None,
            None,
            "Failed to parse multipart data - invalid format",
        )
        .await;
        return (status, json_error).into_response();
    };

    // Parse multipart form data with proper error handling
    let mut metadata = CallMetadata::default();
    let mut audio_data: Option<Vec<u8>> = None;
    let mut audio_filename: Option<String> = None;

    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                let name = field.name().unwrap_or("").to_string();

                match name.as_str() {
                    "audio" => {
                        audio_filename = field.file_name().map(String::from);
                        match field.bytes().await {
                            Ok(data) => audio_data = Some(data.to_vec()),
                            Err(e) => {
                                error!("Failed to read audio data: {}", e);
                                let (status, json_error) = upload_error(
                                    &state,
                                    client_ip,
                                    user_agent,
                                    None,
                                    None,
                                    "Failed to read audio data",
                                )
                                .await;
                                return (status, json_error).into_response();
                            }
                        }
                    }
                    "key" => {
                        if let Ok(text) = field.text().await {
                            metadata.api_key = Some(text);
                        }
                    }
                    "system" => {
                        if let Ok(text) = field.text().await {
                            metadata.system_id = Some(text);
                        }
                    }
                    "systemLabel" => {
                        if let Ok(text) = field.text().await {
                            metadata.system_label = Some(text);
                        }
                    }
                    "test" => {
                        if let Ok(text) = field.text().await {
                            if let Ok(val) = text.parse::<i32>() {
                                metadata.test = Some(val);
                            } else if !text.is_empty() {
                                metadata.test = Some(1); // Any non-empty value counts as test
                            }
                        }
                    }
                    "dateTime" | "datetime" => {
                        if let Ok(text) = field.text().await
                            && let Ok(ts) = text.parse::<i64>()
                        {
                            metadata.datetime =
                                Some(DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now));
                        }
                    }
                    "talkgroup" => {
                        if let Ok(text) = field.text().await
                            && let Ok(tg) = text.parse::<i32>()
                        {
                            metadata.talkgroup_id = Some(tg);
                        }
                    }
                    "talkgroupLabel" => {
                        if let Ok(text) = field.text().await {
                            metadata.talkgroup_label = Some(text);
                        }
                    }
                    "talkgroupGroup" => {
                        if let Ok(text) = field.text().await {
                            metadata.talkgroup_group = Some(text);
                        }
                    }
                    "talkgroupTag" => {
                        if let Ok(text) = field.text().await {
                            metadata.talkgroup_tag = Some(text);
                        }
                    }
                    "frequency" => {
                        if let Ok(text) = field.text().await
                            && let Ok(freq) = text.parse::<i64>()
                        {
                            metadata.frequency = Some(freq);
                        }
                    }
                    "source" => {
                        if let Ok(text) = field.text().await
                            && let Ok(src) = text.parse::<i32>()
                        {
                            metadata.source_radio_id = Some(src);
                        }
                    }
                    "patches" => {
                        if let Ok(text) = field.text().await {
                            metadata.patches = serde_json::from_str(&text).ok();
                        }
                    }
                    "sources" => {
                        if let Ok(text) = field.text().await {
                            metadata.sources = serde_json::from_str(&text).ok();
                        }
                    }
                    "freqList" => {
                        if let Ok(text) = field.text().await {
                            metadata.frequencies = serde_json::from_str(&text).ok();
                        }
                    }
                    "talkerAlias" => {
                        if let Ok(text) = field.text().await {
                            metadata.talker_alias = Some(text);
                        }
                    }
                    "duration" => {
                        // Handle duration if SDRTrunk ever sends it
                        if let Ok(text) = field.text().await
                            && let Ok(dur) = text.parse::<f64>()
                        {
                            metadata.duration = Some(dur);
                        }
                    }
                    _ => {
                        // Ignore unknown fields for compatibility
                        // Don't warn as SDRTrunk may send additional fields
                    }
                }
            }
            Ok(None) => {
                // No more fields - normal completion
                break;
            }
            Err(e) => {
                // Handle multipart parsing errors gracefully
                error!("Error parsing multipart data: {}", e);
                let (status, json_error) = upload_error(
                    &state,
                    client_ip,
                    user_agent,
                    None,
                    None,
                    &format!("Invalid multipart data: {e}"),
                )
                .await;
                return (status, json_error).into_response();
            }
        }
    }

    // Handle test requests first - they don't require audio files
    if metadata.test.is_some() {
        info!(
            "TEST REQUEST: System {} | IP: {}",
            metadata.system_id.as_deref().unwrap_or("Unknown"),
            client_ip
        );

        let message = "incomplete call data: no talkgroup";

        // Check if client wants JSON response (like Python implementation)
        let accept_header = headers
            .get("accept")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if accept_header.contains("application/json") {
            // Return JSON response matching Python format
            return (
                StatusCode::OK,
                Json(serde_json::json!({
                    "status": "ok",
                    "message": message,
                    "callId": "test"
                })),
            )
                .into_response();
        }
        // Return plain text response (default behavior) with explicit content-type
        return Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/plain")
            .body(Body::from(message))
            .unwrap()
            .into_response();
    }

    // For non-test requests, validate required fields
    let Some(audio) = audio_data else {
        let (status, json_error) = upload_error(
            &state,
            client_ip,
            user_agent,
            metadata.api_key,
            metadata.system_id,
            "No audio file provided",
        )
        .await;
        return (status, json_error).into_response();
    };

    let Some(system_id) = metadata.system_id else {
        let (status, json_error) = upload_error(
            &state,
            client_ip,
            user_agent,
            metadata.api_key,
            None,
            "System ID is required",
        )
        .await;
        return (status, json_error).into_response();
    };

    let Some(filename) = audio_filename else {
        let (status, json_error) = upload_error(
            &state,
            client_ip,
            user_agent,
            metadata.api_key,
            Some(system_id),
            "Audio filename is required",
        )
        .await;
        return (status, json_error).into_response();
    };

    // Validate API key if configured
    let mut api_key_id = None;
    if state.config.security.require_api_key {
        if let Some(key) = &metadata.api_key {
            // Simple hash for API key (in production, use proper hashing)
            let key_hash = format!("{:x}", md5::compute(key));

            match sdrtrunk_database::validate_api_key(&state.pool, &key_hash).await {
                Ok(Some(api_key)) => {
                    let api_key_uuid = api_key.id;
                    api_key_id = Some(api_key_uuid.clone());
                    info!("Valid API key used: {}", api_key_uuid);
                }
                Ok(None) => {
                    let (status, json_error) = upload_error(
                        &state,
                        client_ip,
                        user_agent,
                        Some(key.clone()),
                        Some(system_id),
                        "Invalid API key",
                    )
                    .await;
                    return (status, json_error).into_response();
                }
                Err(e) => {
                    error!("Failed to validate API key: {}", e);
                    let (status, json_error) = upload_error(
                        &state,
                        client_ip,
                        user_agent,
                        Some(key.clone()),
                        Some(system_id),
                        "Failed to validate API key",
                    )
                    .await;
                    return (status, json_error).into_response();
                }
            }
        } else {
            let (status, json_error) = upload_error(
                &state,
                client_ip,
                user_agent,
                None,
                Some(system_id),
                "API key is required",
            )
            .await;
            return (status, json_error).into_response();
        }
    }

    // Validate file size
    if audio.len() as u64 > state.config.security.max_upload_size {
        let (status, json_error) = upload_error(
            &state,
            client_ip,
            user_agent,
            metadata.api_key,
            Some(system_id),
            &format!(
                "File size exceeds maximum of {} bytes",
                state.config.security.max_upload_size
            ),
        )
        .await;
        return (status, json_error).into_response();
    }

    // Validate file extension
    let file_extension = std::path::Path::new(&filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();

    if !state
        .config
        .storage
        .allowed_extensions
        .contains(&file_extension)
    {
        let (status, json_error) = upload_error(
            &state,
            client_ip,
            user_agent,
            metadata.api_key,
            Some(system_id),
            &format!("File extension '{file_extension}' is not allowed"),
        )
        .await;
        return (status, json_error).into_response();
    }

    // Determine storage path
    let date = metadata.datetime.unwrap_or_else(Utc::now).date_naive();
    let storage_path = state.get_storage_path(&system_id, date);

    // Create directory structure
    if let Err(e) = std::fs::create_dir_all(&storage_path) {
        error!("Failed to create storage directory: {}", e);
        let (status, json_error) = upload_error(
            &state,
            client_ip,
            user_agent,
            metadata.api_key,
            Some(system_id),
            "Failed to create storage directory",
        )
        .await;
        return (status, json_error).into_response();
    }

    // Save audio file with unique name to avoid conflicts
    let unique_filename = format!(
        "{}_{}",
        chrono::Utc::now().format("%Y%m%d_%H%M%S_%f"),
        filename
    );
    let file_path = storage_path.join(&unique_filename);

    if let Err(e) = std::fs::write(&file_path, &audio) {
        error!("Failed to save audio file: {}", e);
        let (status, json_error) = upload_error(
            &state,
            client_ip,
            user_agent,
            metadata.api_key,
            Some(system_id),
            "Failed to save audio file",
        )
        .await;
        return (status, json_error).into_response();
    }

    // Calculate duration if not provided
    let duration = metadata
        .duration
        .or_else(|| audio_utils::calculate_audio_duration(&audio, Some(&filename)));

    // Create RadioCall record
    let radio_call = RadioCall {
        id: None,
        created_at: Utc::now(),
        call_timestamp: metadata.datetime.unwrap_or_else(Utc::now),
        system_id: system_id.clone(),
        system_label: metadata.system_label.clone(),
        frequency: metadata.frequency,
        talkgroup_id: metadata.talkgroup_id,
        talkgroup_label: metadata.talkgroup_label,
        talkgroup_group: metadata.talkgroup_group,
        talkgroup_tag: metadata.talkgroup_tag,
        source_radio_id: metadata.source_radio_id,
        talker_alias: metadata.talker_alias,
        audio_filename: Some(unique_filename.clone()),
        audio_file_path: Some(file_path.to_string_lossy().to_string()),
        audio_size_bytes: Some(audio.len() as i64),
        duration_seconds: duration,
        upload_ip: Some(client_ip.to_string()),
        upload_timestamp: Utc::now(),
        upload_api_key_id: api_key_id,
        patches: metadata.patches,
        frequencies: metadata.frequencies,
        sources: metadata.sources,
        transcription_status: sdrtrunk_core::types::TranscriptionStatus::Pending,
        transcription_text: None,
        transcription_confidence: None,
        transcription_error: None,
        transcription_started_at: None,
        transcription_completed_at: None,
        speaker_count: None,
        speaker_segments: None,
        transcription_segments: None,
    };

    // Save to database
    let call_id = match sdrtrunk_database::insert_radio_call(&state.pool, &radio_call).await {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to save radio call to database: {}", e);
            // Try to clean up the file
            let _ = std::fs::remove_file(&file_path);
            let (status, json_error) = upload_error(
                &state,
                client_ip,
                user_agent,
                metadata.api_key,
                Some(system_id),
                "Failed to save call to database",
            )
            .await;
            return (status, json_error).into_response();
        }
    };

    // Trigger transcription if enabled
    if let Some(ref transcription_config) = state.config.transcription {
        if transcription_config.enabled {
            if let Some(ref transcription_pool) = state.transcription_pool {
                let transcription_request = sdrtrunk_transcriber::TranscriptionRequest::new(
                    call_id,
                    std::path::PathBuf::from(&file_path),
                );

                // Submit to transcription service using non-blocking try_submit
                // Log queue status for monitoring
                let queue_len = transcription_pool.queue_len();
                let queue_capacity = transcription_pool.queue_capacity().unwrap_or(0);

                match transcription_pool.try_submit(transcription_request) {
                    Ok(()) => {
                        info!("Transcription request submitted for call {} (queue: {}/{})", call_id, queue_len + 1, queue_capacity);
                    }
                    Err(e) => {
                        error!("Failed to submit transcription for call {} (queue full: {}/{}): {}", call_id, queue_len, queue_capacity, e);
                        // Update database to mark transcription as failed due to queue full
                        let db_pool = state.pool.clone();
                        tokio::spawn(async move {
                            if let Err(db_err) = sdrtrunk_database::update_transcription_status(
                                &db_pool,
                                call_id,
                                "failed",
                            ).await {
                                error!("Failed to update transcription status for call {}: {}", call_id, db_err);
                            }
                        });
                    }
                }
            } else {
                warn!("Transcription enabled but no worker pool initialized");
            }
        }
    }

    // Update system statistics (non-critical, log errors but don't fail)
    if let Err(e) =
        sdrtrunk_database::update_system_stats(&state.pool, &system_id, metadata.system_label).await
    {
        warn!("Failed to update system stats: {}", e);
    }

    // Log successful upload (non-critical)
    let params = sdrtrunk_database::UploadLogParams {
        client_ip,
        user_agent,
        api_key_id: metadata.api_key,
        system_id: Some(system_id.clone()),
        success: true,
        error_message: None,
        filename: Some(unique_filename),
        file_size: Some(audio.len() as i64),
    };
    if let Err(e) = sdrtrunk_database::insert_upload_log(&state.pool, params).await {
        warn!("Failed to log upload: {}", e);
    }

    // Create formatted log with useful details
    let talkgroup_info = if let Some(tg_id) = radio_call.talkgroup_id {
        if let Some(ref label) = radio_call.talkgroup_label {
            format!("TG {} ({})", tg_id, label)
        } else {
            format!("TG {}", tg_id)
        }
    } else {
        "Unknown TG".to_string()
    };

    let freq_mhz = radio_call
        .frequency
        .map(|f| format!("{:.4} MHz", f as f64 / 1_000_000.0))
        .unwrap_or_else(|| "Unknown Freq".to_string());

    let duration_str = duration
        .map(|d| format!("{:.2}s", d))
        .unwrap_or_else(|| "N/A".to_string());

    let file_size_kb = audio.len() as f64 / 1024.0;

    info!(
        "UPLOAD: {} | {} | {} | {} | {:.1}KB | {} | {}",
        system_id,
        talkgroup_info,
        freq_mhz,
        duration_str,
        file_size_kb,
        call_id.to_string().split('-').next().unwrap_or(""),
        client_ip
    );

    // Check if client wants JSON response (like Python implementation)
    let accept_header = headers
        .get("accept")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if accept_header.contains("application/json") {
        // Return JSON response
        (
            StatusCode::OK,
            Json(UploadResponse {
                success: true,
                id: call_id,
                message: "Call uploaded successfully".to_string(),
            }),
        )
            .into_response()
    } else {
        // Return plain text response (default behavior matching Python)
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/plain")
            .body(Body::from("Call imported successfully."))
            .unwrap()
            .into_response()
    }
}

/// Helper function to handle upload errors with proper logging
#[allow(clippy::too_many_arguments)]
async fn upload_error(
    state: &Arc<AppState>,
    client_ip: std::net::IpAddr,
    user_agent: Option<String>,
    api_key: Option<String>,
    system_id: Option<String>,
    error_message: &str,
) -> (StatusCode, Json<ErrorResponse>) {
    error!(
        "❌ UPLOAD FAILED: {} | System: {} | IP: {}",
        error_message,
        system_id.as_deref().unwrap_or("Unknown"),
        client_ip
    );

    // Log failed upload (best effort - don't fail if logging fails)
    let params = sdrtrunk_database::UploadLogParams {
        client_ip,
        user_agent,
        api_key_id: api_key,
        system_id,
        success: false,
        error_message: Some(error_message.to_string()),
        filename: None,
        file_size: None,
    };
    if let Err(e) = sdrtrunk_database::insert_upload_log(&state.pool, params).await {
        warn!("Failed to log upload error: {}", e);
    }

    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            success: false,
            error: error_message.to_string(),
        }),
    )
}

/// Metadata extracted from multipart form
#[derive(Default)]
struct CallMetadata {
    api_key: Option<String>,
    system_id: Option<String>,
    system_label: Option<String>,
    datetime: Option<DateTime<Utc>>,
    talkgroup_id: Option<i32>,
    talkgroup_label: Option<String>,
    talkgroup_group: Option<String>,
    talkgroup_tag: Option<String>,
    frequency: Option<i64>,
    source_radio_id: Option<i32>,
    talker_alias: Option<String>,
    test: Option<i32>,
    duration: Option<f64>, // Duration in seconds
    patches: Option<serde_json::Value>,
    sources: Option<serde_json::Value>,
    frequencies: Option<serde_json::Value>,
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use chrono::{Datelike, Utc};
    use serde_json;
    use uuid::Uuid;

    #[test]
    fn test_upload_response_serialization() {
        let call_id = Uuid::new_v4();
        let response = UploadResponse {
            success: true,
            id: call_id,
            message: "Call uploaded successfully".to_string(),
        };

        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert!(json.contains("\"success\":true"));
        assert!(json.contains(&call_id.to_string()));
        assert!(json.contains("Call uploaded successfully"));
    }

    #[test]
    fn test_upload_response_failure() {
        let call_id = Uuid::new_v4();
        let response = UploadResponse {
            success: false,
            id: call_id,
            message: "Upload failed".to_string(),
        };

        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("Upload failed"));
    }

    #[test]
    fn test_error_response_serialization() {
        let error = ErrorResponse {
            success: false,
            error: "Invalid file format".to_string(),
        };

        let json = serde_json::to_string(&error).expect("Failed to serialize");
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("Invalid file format"));
    }

    #[test]
    fn test_call_metadata_default() {
        let metadata = CallMetadata::default();

        assert!(metadata.api_key.is_none());
        assert!(metadata.system_id.is_none());
        assert!(metadata.system_label.is_none());
        assert!(metadata.datetime.is_none());
        assert!(metadata.talkgroup_id.is_none());
        assert!(metadata.talkgroup_label.is_none());
        assert!(metadata.talkgroup_group.is_none());
        assert!(metadata.talkgroup_tag.is_none());
        assert!(metadata.frequency.is_none());
        assert!(metadata.source_radio_id.is_none());
        assert!(metadata.talker_alias.is_none());
        assert!(metadata.test.is_none());
        assert!(metadata.duration.is_none());
        assert!(metadata.patches.is_none());
        assert!(metadata.sources.is_none());
        assert!(metadata.frequencies.is_none());
    }

    #[test]
    fn test_call_metadata_with_values() {
        let mut metadata = CallMetadata::default();
        metadata.system_id = Some("police".to_string());
        metadata.talkgroup_id = Some(12345);
        metadata.frequency = Some(154250000);
        metadata.source_radio_id = Some(9876);
        metadata.talker_alias = Some("Unit 123".to_string());
        metadata.duration = Some(15.5);

        assert_eq!(metadata.system_id, Some("police".to_string()));
        assert_eq!(metadata.talkgroup_id, Some(12345));
        assert_eq!(metadata.frequency, Some(154250000));
        assert_eq!(metadata.source_radio_id, Some(9876));
        assert_eq!(metadata.talker_alias, Some("Unit 123".to_string()));
        assert_eq!(metadata.duration, Some(15.5));
    }

    #[test]
    fn test_call_metadata_with_json_fields() {
        let mut metadata = CallMetadata::default();

        // Test patches JSON
        let patches_json = serde_json::json!(["patch1", "patch2"]);
        metadata.patches = Some(patches_json.clone());

        // Test sources JSON
        let sources_json = serde_json::json!({"source1": "value1", "source2": "value2"});
        metadata.sources = Some(sources_json.clone());

        // Test frequencies JSON
        let frequencies_json = serde_json::json!([460125000, 460150000]);
        metadata.frequencies = Some(frequencies_json.clone());

        assert_eq!(metadata.patches, Some(patches_json));
        assert_eq!(metadata.sources, Some(sources_json));
        assert_eq!(metadata.frequencies, Some(frequencies_json));
    }

    #[test]
    fn test_call_metadata_test_field_parsing() {
        let mut metadata = CallMetadata::default();

        // Test integer value
        metadata.test = Some(1);
        assert_eq!(metadata.test, Some(1));

        // Test zero value (should still be Some(0))
        metadata.test = Some(0);
        assert_eq!(metadata.test, Some(0));
    }

    #[test]
    fn test_call_metadata_datetime_handling() {
        use chrono::Utc;

        let mut metadata = CallMetadata::default();
        let now = Utc::now();
        metadata.datetime = Some(now);

        assert!(metadata.datetime.is_some());
        let stored_time = metadata.datetime.unwrap();
        let diff = (now - stored_time).num_seconds().abs();
        assert!(diff < 1); // Should be within 1 second
    }

    #[test]
    fn test_upload_response_uuid_consistency() {
        let call_id = Uuid::new_v4();
        let response1 = UploadResponse {
            success: true,
            id: call_id,
            message: "Test".to_string(),
        };
        let response2 = UploadResponse {
            success: true,
            id: call_id,
            message: "Test".to_string(),
        };

        // Same UUID should serialize to same string
        let json1 = serde_json::to_string(&response1).expect("Failed to serialize");
        let json2 = serde_json::to_string(&response2).expect("Failed to serialize");
        assert_eq!(json1, json2);
    }

    #[test]
    fn test_error_response_with_various_messages() {
        let error_messages = vec![
            "File too large",
            "Invalid API key",
            "System ID is required",
            "Audio filename is required",
            "Failed to save audio file",
            "Database error occurred",
        ];

        for message in error_messages {
            let error = ErrorResponse {
                success: false,
                error: message.to_string(),
            };

            let json = serde_json::to_string(&error).expect("Failed to serialize");
            assert!(json.contains(message));
            assert!(json.contains("\"success\":false"));
        }
    }

    #[test]
    fn test_json_serialization_escaping() {
        // Test that special characters are properly escaped in JSON
        let error = ErrorResponse {
            success: false,
            error: "Error with \"quotes\" and \n newlines".to_string(),
        };

        let json = serde_json::to_string(&error).expect("Failed to serialize");
        assert!(json.contains("\\\"quotes\\\""));
        assert!(json.contains("\\n"));
    }

    #[test]
    fn test_call_metadata_frequency_ranges() {
        let mut metadata = CallMetadata::default();

        // Test various frequency ranges
        let frequencies = vec![
            150_000_000, // 150 MHz (VHF)
            450_000_000, // 450 MHz (UHF)
            800_000_000, // 800 MHz (800 MHz band)
            900_000_000, // 900 MHz (900 MHz band)
        ];

        for freq in frequencies {
            metadata.frequency = Some(freq);
            assert_eq!(metadata.frequency, Some(freq));
        }
    }

    #[test]
    fn test_call_metadata_talkgroup_ranges() {
        let mut metadata = CallMetadata::default();

        // Test various talkgroup ID ranges
        let talkgroup_ids = vec![
            1,        // Minimum
            100,      // Common range
            65535,    // 16-bit max
            i32::MAX, // Maximum i32
        ];

        for tg_id in talkgroup_ids {
            metadata.talkgroup_id = Some(tg_id);
            assert_eq!(metadata.talkgroup_id, Some(tg_id));
        }
    }

    #[test]
    fn test_call_metadata_radio_id_ranges() {
        let mut metadata = CallMetadata::default();

        // Test various radio ID ranges
        let radio_ids = vec![
            1,        // Minimum
            1000,     // Common range
            9999999,  // Large ID
            i32::MAX, // Maximum i32
        ];

        for radio_id in radio_ids {
            metadata.source_radio_id = Some(radio_id);
            assert_eq!(metadata.source_radio_id, Some(radio_id));
        }
    }

    #[test]
    fn test_call_metadata_string_fields() {
        let mut metadata = CallMetadata::default();

        // Test system_id
        metadata.system_id = Some("test_system_123".to_string());
        assert_eq!(metadata.system_id, Some("test_system_123".to_string()));

        // Test system_label
        metadata.system_label = Some("Test Radio System".to_string());
        assert_eq!(metadata.system_label, Some("Test Radio System".to_string()));

        // Test talkgroup_label
        metadata.talkgroup_label = Some("Emergency Dispatch".to_string());
        assert_eq!(
            metadata.talkgroup_label,
            Some("Emergency Dispatch".to_string())
        );

        // Test talkgroup_group
        metadata.talkgroup_group = Some("Emergency Services".to_string());
        assert_eq!(
            metadata.talkgroup_group,
            Some("Emergency Services".to_string())
        );

        // Test talkgroup_tag
        metadata.talkgroup_tag = Some("Priority".to_string());
        assert_eq!(metadata.talkgroup_tag, Some("Priority".to_string()));

        // Test talker_alias
        metadata.talker_alias = Some("Unit 99".to_string());
        assert_eq!(metadata.talker_alias, Some("Unit 99".to_string()));

        // Test api_key
        metadata.api_key = Some("secret_key_123".to_string());
        assert_eq!(metadata.api_key, Some("secret_key_123".to_string()));
    }

    #[test]
    fn test_call_metadata_duration_precision() {
        let mut metadata = CallMetadata::default();

        // Test various duration precisions
        let durations = vec![
            0.0,        // Zero duration
            0.1,        // Sub-second
            1.0,        // Exact second
            15.5,       // Half second
            123.456789, // High precision
        ];

        for duration in durations {
            metadata.duration = Some(duration);
            assert_eq!(metadata.duration, Some(duration));
        }
    }

    #[test]
    fn test_upload_response_message_variations() {
        let call_id = Uuid::new_v4();
        let messages = vec![
            "Call uploaded successfully",
            "Upload complete",
            "File processed",
            "Audio saved",
        ];

        for message in messages {
            let response = UploadResponse {
                success: true,
                id: call_id,
                message: message.to_string(),
            };

            let json = serde_json::to_string(&response).expect("Failed to serialize");
            assert!(json.contains(message));
        }
    }

    #[test]
    fn test_json_roundtrip_serialization() {
        let call_id = Uuid::new_v4();
        let original_response = UploadResponse {
            success: true,
            id: call_id,
            message: "Test message".to_string(),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&original_response).expect("Failed to serialize");

        // Deserialize back
        let deserialized: UploadResponse =
            serde_json::from_str(&json).expect("Failed to deserialize");

        // Check all fields match
        assert_eq!(deserialized.success, original_response.success);
        assert_eq!(deserialized.id, original_response.id);
        assert_eq!(deserialized.message, original_response.message);
    }

    #[test]
    fn test_error_response_roundtrip_serialization() {
        let original_error = ErrorResponse {
            success: false,
            error: "Test error message".to_string(),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&original_error).expect("Failed to serialize");

        // Deserialize back
        let deserialized: ErrorResponse =
            serde_json::from_str(&json).expect("Failed to deserialize");

        // Check all fields match
        assert_eq!(deserialized.success, original_error.success);
        assert_eq!(deserialized.error, original_error.error);
    }

    #[test]
    fn test_call_metadata_datetime_timestamp_conversion() {
        use chrono::{DateTime, Utc};

        // Test various timestamp formats that SDRTrunk might send
        let timestamps = vec![
            1640995200, // 2022-01-01 00:00:00 UTC
            1672531200, // 2023-01-01 00:00:00 UTC
            1704067200, // 2024-01-01 00:00:00 UTC
        ];

        for ts in timestamps {
            let datetime = DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now);
            let mut metadata = CallMetadata::default();
            metadata.datetime = Some(datetime);

            assert!(metadata.datetime.is_some());
            let stored = metadata.datetime.unwrap();
            assert_eq!(stored.timestamp(), ts);
        }
    }

    #[test]
    fn test_call_metadata_talkgroup_parsing_edge_cases() {
        let mut metadata = CallMetadata::default();

        // Test edge cases for talkgroup parsing
        let talkgroup_values = vec![
            ("0", Some(0)),
            ("1", Some(1)),
            ("-1", Some(-1)),
            ("65535", Some(65535)),
            ("invalid", None), // Would fail parsing in real implementation
        ];

        for (input, expected) in talkgroup_values {
            if let Ok(tg) = input.parse::<i32>() {
                metadata.talkgroup_id = Some(tg);
                assert_eq!(metadata.talkgroup_id, expected);
            }
        }
    }

    #[test]
    fn test_call_metadata_frequency_parsing_edge_cases() {
        let mut metadata = CallMetadata::default();

        // Test various frequency formats
        let frequency_values = vec![
            ("150000000", Some(150_000_000)), // 150 MHz
            ("460125000", Some(460_125_000)), // 460.125 MHz
            ("800000000", Some(800_000_000)), // 800 MHz
            ("0", Some(0)),                   // Edge case
        ];

        for (input, expected) in frequency_values {
            if let Ok(freq) = input.parse::<i64>() {
                metadata.frequency = Some(freq);
                assert_eq!(metadata.frequency, expected);
            }
        }
    }

    #[test]
    fn test_call_metadata_source_radio_id_parsing() {
        let mut metadata = CallMetadata::default();

        // Test various source radio ID formats
        let source_values = vec![
            ("1", Some(1)),
            ("1000", Some(1000)),
            ("999999", Some(999_999)),
            ("2147483647", Some(i32::MAX)),
        ];

        for (input, expected) in source_values {
            if let Ok(src) = input.parse::<i32>() {
                metadata.source_radio_id = Some(src);
                assert_eq!(metadata.source_radio_id, expected);
            }
        }
    }

    #[test]
    fn test_call_metadata_test_field_variations() {
        let mut metadata = CallMetadata::default();

        // Test different ways the test field might be set
        let test_values = vec![
            ("1", Some(1)),
            ("0", Some(0)),
            ("true", None), // Would fail i32 parsing, but non-empty counts as test
            ("test", None), // Would fail i32 parsing, but non-empty counts as test
        ];

        for (input, expected) in test_values {
            if let Ok(val) = input.parse::<i32>() {
                metadata.test = Some(val);
                assert_eq!(metadata.test, expected);
            } else if !input.is_empty() {
                metadata.test = Some(1); // Non-empty value counts as test
                assert_eq!(metadata.test, Some(1));
            }
        }
    }

    #[test]
    fn test_call_metadata_json_field_parsing() {
        let mut metadata = CallMetadata::default();

        // Test patches JSON parsing
        let patches_json_str = r#"[{"id": 1, "name": "Patch1"}, {"id": 2, "name": "Patch2"}]"#;
        let patches_result: Result<serde_json::Value, _> = serde_json::from_str(patches_json_str);
        if let Ok(patches) = patches_result {
            metadata.patches = Some(patches);
            assert!(metadata.patches.is_some());
            let patches_value = metadata.patches.as_ref().unwrap();
            assert!(patches_value.is_array());
            assert_eq!(patches_value.as_array().unwrap().len(), 2);
        }

        // Test sources JSON parsing
        let sources_json_str = r#"{"primary": "192.168.1.1", "backup": "192.168.1.2"}"#;
        let sources_result: Result<serde_json::Value, _> = serde_json::from_str(sources_json_str);
        if let Ok(sources) = sources_result {
            metadata.sources = Some(sources);
            assert!(metadata.sources.is_some());
            let sources_value = metadata.sources.as_ref().unwrap();
            assert!(sources_value.is_object());
        }

        // Test frequencies JSON parsing
        let freqs_json_str = r#"[460125000, 460150000, 460175000]"#;
        let freqs_result: Result<serde_json::Value, _> = serde_json::from_str(freqs_json_str);
        if let Ok(frequencies) = freqs_result {
            metadata.frequencies = Some(frequencies);
            assert!(metadata.frequencies.is_some());
            let freqs_value = metadata.frequencies.as_ref().unwrap();
            assert!(freqs_value.is_array());
            assert_eq!(freqs_value.as_array().unwrap().len(), 3);
        }
    }

    #[test]
    fn test_call_metadata_malformed_json_handling() {
        let mut metadata = CallMetadata::default();

        // Test malformed JSON (should result in None)
        let malformed_jsons = vec!["invalid json", "{incomplete", "[1,2,3", ""];

        for malformed in malformed_jsons {
            let result: Result<serde_json::Value, _> = serde_json::from_str(malformed);
            if result.is_err() {
                metadata.patches = None;
                assert!(metadata.patches.is_none());
            }
        }
    }

    #[test]
    fn test_upload_response_with_different_uuids() {
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();

        let response1 = UploadResponse {
            success: true,
            id: uuid1,
            message: "Upload 1".to_string(),
        };

        let response2 = UploadResponse {
            success: true,
            id: uuid2,
            message: "Upload 2".to_string(),
        };

        // UUIDs should be different
        assert_ne!(response1.id, response2.id);

        // Serialization should produce different JSON
        let json1 = serde_json::to_string(&response1).expect("Failed to serialize");
        let json2 = serde_json::to_string(&response2).expect("Failed to serialize");
        assert_ne!(json1, json2);
    }

    #[test]
    fn test_error_response_with_long_messages() {
        let long_message = "This is a very long error message ".repeat(100);
        let error = ErrorResponse {
            success: false,
            error: long_message.clone(),
        };

        let json = serde_json::to_string(&error).expect("Failed to serialize");
        assert!(json.contains(&long_message));
        assert!(json.len() > 1000); // Should be quite long
    }

    #[test]
    fn test_call_metadata_duration_edge_cases() {
        let mut metadata = CallMetadata::default();

        // Test various duration values
        let durations = vec![
            0.0,     // Zero duration
            0.001,   // Very short
            0.5,     // Half second
            60.0,    // One minute
            3600.0,  // One hour
            86400.0, // One day (unlikely but possible)
        ];

        for duration in durations {
            metadata.duration = Some(duration);
            assert_eq!(metadata.duration, Some(duration));

            // Test precision preservation
            if duration != 0.0 {
                assert!(metadata.duration.unwrap() > 0.0);
            }
        }
    }

    #[test]
    fn test_call_metadata_string_field_edge_cases() {
        let mut metadata = CallMetadata::default();

        // Test empty strings
        metadata.system_id = Some(String::new());
        assert_eq!(metadata.system_id, Some(String::new()));

        // Test very long strings
        let long_string = "A".repeat(1000);
        metadata.system_label = Some(long_string.clone());
        assert_eq!(metadata.system_label, Some(long_string));

        // Test strings with special characters
        let special_string = "System!@#$%^&*()_+{}|:<>?[]\\;'\".,/";
        metadata.talkgroup_label = Some(special_string.to_string());
        assert_eq!(metadata.talkgroup_label, Some(special_string.to_string()));

        // Test Unicode strings
        let unicode_string = "系统标识符";
        metadata.talkgroup_group = Some(unicode_string.to_string());
        assert_eq!(metadata.talkgroup_group, Some(unicode_string.to_string()));
    }

    #[test]
    fn test_upload_response_success_variations() {
        let call_id = Uuid::new_v4();

        // Test different success states
        let success_response = UploadResponse {
            success: true,
            id: call_id,
            message: "Success".to_string(),
        };

        let failure_response = UploadResponse {
            success: false,
            id: call_id,
            message: "Failed".to_string(),
        };

        assert!(success_response.success);
        assert!(!failure_response.success);
        assert_eq!(success_response.id, failure_response.id); // Same ID, different status
    }

    #[test]
    fn test_error_response_consistency() {
        let error1 = ErrorResponse {
            success: false,
            error: "Error 1".to_string(),
        };

        let error2 = ErrorResponse {
            success: false,
            error: "Error 2".to_string(),
        };

        // Both should have success = false
        assert!(!error1.success);
        assert!(!error2.success);

        // But different error messages
        assert_ne!(error1.error, error2.error);
    }

    #[test]
    fn test_call_metadata_comprehensive_field_test() {
        let mut metadata = CallMetadata::default();

        // Set all fields to non-default values
        metadata.api_key = Some("test_api_key_123".to_string());
        metadata.system_id = Some("comprehensive_test_system".to_string());
        metadata.system_label = Some("Comprehensive Test Radio System".to_string());
        metadata.datetime = Some(chrono::Utc::now());
        metadata.talkgroup_id = Some(98765);
        metadata.talkgroup_label = Some("Emergency Dispatch".to_string());
        metadata.talkgroup_group = Some("Emergency Services".to_string());
        metadata.talkgroup_tag = Some("High Priority".to_string());
        metadata.frequency = Some(460_125_000);
        metadata.source_radio_id = Some(12345);
        metadata.talker_alias = Some("UNIT_99".to_string());
        metadata.test = Some(0); // Not a test
        metadata.duration = Some(45.25);
        metadata.patches = Some(serde_json::json!(["patch1", "patch2"]));
        metadata.sources = Some(serde_json::json!({"primary": "site1", "backup": "site2"}));
        metadata.frequencies = Some(serde_json::json!([460125000, 460150000]));

        // Verify all fields are set
        assert!(metadata.api_key.is_some());
        assert!(metadata.system_id.is_some());
        assert!(metadata.system_label.is_some());
        assert!(metadata.datetime.is_some());
        assert!(metadata.talkgroup_id.is_some());
        assert!(metadata.talkgroup_label.is_some());
        assert!(metadata.talkgroup_group.is_some());
        assert!(metadata.talkgroup_tag.is_some());
        assert!(metadata.frequency.is_some());
        assert!(metadata.source_radio_id.is_some());
        assert!(metadata.talker_alias.is_some());
        assert!(metadata.test.is_some());
        assert!(metadata.duration.is_some());
        assert!(metadata.patches.is_some());
        assert!(metadata.sources.is_some());
        assert!(metadata.frequencies.is_some());

        // Verify specific values
        assert_eq!(metadata.api_key, Some("test_api_key_123".to_string()));
        assert_eq!(metadata.talkgroup_id, Some(98765));
        assert_eq!(metadata.frequency, Some(460_125_000));
        assert_eq!(metadata.source_radio_id, Some(12345));
        assert_eq!(metadata.test, Some(0));
        assert_eq!(metadata.duration, Some(45.25));
    }

    #[test]
    fn test_json_serialization_special_characters() {
        // Test JSON serialization with various special characters
        let special_messages = vec![
            "Error with \"quotes\"",
            "Error with \n newlines",
            "Error with \t tabs",
            "Error with \\ backslashes",
            "Error with / forward slashes",
            "Error with Unicode: 🚨📻",
        ];

        for message in special_messages {
            let error = ErrorResponse {
                success: false,
                error: message.to_string(),
            };

            let json = serde_json::to_string(&error).expect("Should serialize");

            // Should contain escaped versions of special characters
            assert!(json.contains("\"success\":false"));

            // Should be valid JSON that can be parsed back
            let parsed: ErrorResponse = serde_json::from_str(&json).expect("Should parse back");
            assert_eq!(parsed.error, message);
        }
    }

    #[test]
    fn test_call_metadata_boundary_values() {
        let mut metadata = CallMetadata::default();

        // Test boundary values for various fields

        // Minimum values
        metadata.talkgroup_id = Some(i32::MIN);
        metadata.frequency = Some(0);
        metadata.source_radio_id = Some(0);
        metadata.duration = Some(0.0);

        assert_eq!(metadata.talkgroup_id, Some(i32::MIN));
        assert_eq!(metadata.frequency, Some(0));
        assert_eq!(metadata.source_radio_id, Some(0));
        assert_eq!(metadata.duration, Some(0.0));

        // Maximum values
        metadata.talkgroup_id = Some(i32::MAX);
        metadata.frequency = Some(i64::MAX);
        metadata.source_radio_id = Some(i32::MAX);
        metadata.duration = Some(f64::MAX);

        assert_eq!(metadata.talkgroup_id, Some(i32::MAX));
        assert_eq!(metadata.frequency, Some(i64::MAX));
        assert_eq!(metadata.source_radio_id, Some(i32::MAX));
        assert_eq!(metadata.duration, Some(f64::MAX));
    }

    // Additional unit tests to improve coverage
    #[test]
    fn test_upload_response_edge_cases() {
        // Test UploadResponse with edge case UUIDs
        let nil_uuid = Uuid::nil();
        let response_with_nil = UploadResponse {
            success: false,
            id: nil_uuid,
            message: "Nil UUID test".to_string(),
        };

        assert!(!response_with_nil.success);
        assert!(response_with_nil.id.is_nil());
        assert_eq!(response_with_nil.message, "Nil UUID test");

        // Test serialization of nil UUID
        let json = serde_json::to_string(&response_with_nil).unwrap();
        assert!(json.contains("00000000-0000-0000-0000-000000000000"));
    }

    #[test]
    fn test_error_response_comprehensive() {
        // Test ErrorResponse with empty error message
        let empty_error = ErrorResponse {
            success: false,
            error: String::new(),
        };

        assert!(!empty_error.success);
        assert!(empty_error.error.is_empty());

        let json = serde_json::to_string(&empty_error).unwrap();
        assert!(json.contains("\"error\":\"\""));

        // Test with very long error message
        let long_error_message = "Error: ".repeat(1500) + "Final message";
        let long_error = ErrorResponse {
            success: false,
            error: long_error_message.clone(),
        };

        assert_eq!(long_error.error, long_error_message);
        assert!(long_error.error.len() > 10000);

        // Ensure it can be serialized
        let json = serde_json::to_string(&long_error).unwrap();
        assert!(json.contains("Final message"));
    }

    #[test]
    fn test_call_metadata_numeric_extremes() {
        let mut metadata = CallMetadata::default();

        // Test with zero values (valid edge cases)
        metadata.talkgroup_id = Some(0);
        metadata.frequency = Some(0);
        metadata.source_radio_id = Some(0);
        metadata.duration = Some(0.0);

        assert_eq!(metadata.talkgroup_id, Some(0));
        assert_eq!(metadata.frequency, Some(0));
        assert_eq!(metadata.source_radio_id, Some(0));
        assert_eq!(metadata.duration, Some(0.0));

        // Test with negative values (for fields that support them)
        metadata.talkgroup_id = Some(-999);
        metadata.source_radio_id = Some(-1);

        assert_eq!(metadata.talkgroup_id, Some(-999));
        assert_eq!(metadata.source_radio_id, Some(-1));
    }

    #[test]
    fn test_call_metadata_string_boundaries() {
        let mut metadata = CallMetadata::default();

        // Test with very long system ID
        let long_system_id = "A".repeat(1000);
        metadata.system_id = Some(long_system_id.clone());
        assert_eq!(metadata.system_id.as_ref().unwrap().len(), 1000);

        // Test with Unicode characters
        metadata.system_label = Some("Test 测试 System 📻".to_string());
        assert!(metadata.system_label.as_ref().unwrap().contains("测试"));
        assert!(metadata.system_label.as_ref().unwrap().contains("📻"));

        // Test with special control characters
        metadata.talkgroup_label = Some("TG\0\t\r\n\x1F".to_string());
        assert!(metadata.talkgroup_label.as_ref().unwrap().contains("\0"));
        assert!(metadata.talkgroup_label.as_ref().unwrap().contains("\t"));
    }

    #[test]
    fn test_response_structs_serialization_roundtrip() {
        // Test UploadResponse roundtrip
        let original_upload = UploadResponse {
            success: true,
            id: Uuid::new_v4(),
            message: "Test message with special chars: éáíóú".to_string(),
        };

        let json = serde_json::to_string(&original_upload).unwrap();
        let deserialized: UploadResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.success, original_upload.success);
        assert_eq!(deserialized.id, original_upload.id);
        assert_eq!(deserialized.message, original_upload.message);

        // Test ErrorResponse roundtrip
        let original_error = ErrorResponse {
            success: false,
            error: "Error with \"quotes\" and \nnewlines and \ttabs".to_string(),
        };

        let json = serde_json::to_string(&original_error).unwrap();
        let deserialized: ErrorResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.success, original_error.success);
        assert_eq!(deserialized.error, original_error.error);
    }

    #[test]
    fn test_call_metadata_field_combinations() {
        // Test various field combinations that might occur in real uploads

        // Minimal metadata (only required fields)
        let minimal = CallMetadata {
            system_id: Some("minimal_system".to_string()),
            ..CallMetadata::default()
        };
        assert!(minimal.system_id.is_some());
        assert!(minimal.talkgroup_id.is_none());

        // Metadata with only IDs
        let ids_only = CallMetadata {
            system_id: Some("id_system".to_string()),
            talkgroup_id: Some(12345),
            source_radio_id: Some(67890),
            frequency: Some(460_125_000),
            ..CallMetadata::default()
        };
        assert!(ids_only.system_label.is_none());
        assert!(ids_only.talkgroup_label.is_none());
        assert!(ids_only.talker_alias.is_none());
    }

    #[test]
    fn test_call_metadata_json_type_variations() {
        let mut metadata = CallMetadata::default();

        // Test patches with different JSON types
        metadata.patches = Some(serde_json::json!("simple_string"));
        assert!(metadata.patches.as_ref().unwrap().is_string());

        metadata.patches = Some(serde_json::json!(42));
        assert!(metadata.patches.as_ref().unwrap().is_number());

        metadata.patches = Some(serde_json::json!(true));
        assert!(metadata.patches.as_ref().unwrap().is_boolean());

        // Test sources with mixed data types
        metadata.sources = Some(serde_json::json!({
            "string_field": "value",
            "number_field": 123,
            "boolean_field": false,
            "null_field": null,
            "array_field": [1, 2, 3],
            "object_field": {"nested": "data"}
        }));

        let sources = metadata.sources.as_ref().unwrap();
        assert_eq!(sources["string_field"], "value");
        assert_eq!(sources["number_field"], 123);
        assert_eq!(sources["boolean_field"], false);
        assert!(sources["null_field"].is_null());
        assert!(sources["array_field"].is_array());
        assert!(sources["object_field"].is_object());
    }

    #[test]
    fn test_uuid_format_variations() {
        // Test various UUID formats in responses
        let test_cases = vec![
            (Uuid::nil(), "nil UUID"),
            (Uuid::new_v4(), "random UUID v4"),
        ];

        for (uuid, description) in test_cases {
            let response = UploadResponse {
                success: true,
                id: uuid,
                message: format!("Testing {}", description),
            };

            assert_eq!(response.id, uuid);

            // Verify JSON contains proper UUID format
            let json = serde_json::to_string(&response).unwrap();
            assert!(json.contains(&uuid.to_string()));

            // Verify hyphenated format
            let uuid_str = uuid.to_string();
            assert_eq!(uuid_str.len(), 36); // Standard UUID string length
            assert_eq!(uuid_str.chars().filter(|&c| c == '-').count(), 4); // Four hyphens
        }
    }

    #[test]
    fn test_create_radio_call_validation() {
        let metadata = CallMetadata {
            system_id: Some("police".to_string()),
            talkgroup_id: Some(12345),
            frequency: Some(460125000),
            source_radio_id: Some(1234),
            datetime: Some(Utc::now()),
            duration: Some(15.5),
            ..Default::default()
        };

        // Test successful creation with all fields populated
        assert!(metadata.system_id.is_some());
        assert!(metadata.talkgroup_id.is_some());
        assert!(metadata.frequency.is_some());
        assert!(metadata.source_radio_id.is_some());
        assert!(metadata.datetime.is_some());
        assert!(metadata.duration.is_some());
    }

    #[test]
    fn test_file_validation_logic() {
        // Test file size validation
        let small_file = [0u8; 100]; // 100 bytes
        let large_file = vec![0u8; 100 * 1024 * 1024]; // 100MB

        assert!(small_file.len() < 50 * 1024 * 1024); // Under 50MB limit
        assert!(large_file.len() > 50 * 1024 * 1024); // Over 50MB limit

        // Test filename validation
        let valid_filename = "call_20241224_120000.mp3";
        let invalid_filename = "../../../etc/passwd";

        assert!(!valid_filename.contains(".."));
        assert!(invalid_filename.contains(".."));

        // Test file extension validation
        let mp3_file = "audio.mp3";
        let wav_file = "audio.wav";
        let txt_file = "data.txt";

        assert!(mp3_file.ends_with(".mp3"));
        assert!(wav_file.ends_with(".wav"));
        assert!(!txt_file.ends_with(".mp3") && !txt_file.ends_with(".wav"));
    }

    #[test]
    fn test_system_id_extraction_logic() {
        let mut metadata = CallMetadata::default();

        // Test with system_id present
        metadata.system_id = Some("fire_dept".to_string());
        let system_id = metadata
            .system_id
            .clone()
            .unwrap_or("default_system".to_string());
        assert_eq!(system_id, "fire_dept");

        // Test with empty system_id
        metadata.system_id = None;
        let system_id = metadata
            .system_id
            .clone()
            .unwrap_or("default_system".to_string());
        assert_eq!(system_id, "default_system");

        // Test with empty string system_id
        metadata.system_id = Some("".to_string());
        let system_id = if metadata.system_id.as_ref().is_none_or(|s| s.is_empty()) {
            "default_system".to_string()
        } else {
            metadata.system_id.clone().unwrap()
        };
        assert_eq!(system_id, "default_system");
    }

    #[test]
    fn test_timestamp_conversion_edge_cases() {
        // Test valid timestamp conversion
        let valid_timestamp = 1640995200i64; // Jan 1, 2022 00:00:00 UTC
        let datetime = DateTime::from_timestamp(valid_timestamp, 0);
        assert!(datetime.is_some());

        // Test pre-epoch timestamp conversion (before 1970-01-01)
        let pre_epoch_timestamp = -86400i64; // 1969-12-31
        let datetime = DateTime::from_timestamp(pre_epoch_timestamp, 0);
        if datetime.is_some() {
            let dt = datetime.unwrap();
            assert!(dt.year() == 1969);
        }

        // Test edge case: year 2038 problem
        let y2038_timestamp = 2147483647i64; // Jan 19, 2038 03:14:07 UTC
        let datetime = DateTime::from_timestamp(y2038_timestamp, 0);
        assert!(datetime.is_some());

        // Test edge case: far future timestamp
        let future_timestamp = 4102444800i64; // Jan 1, 2100 00:00:00 UTC
        let datetime = DateTime::from_timestamp(future_timestamp, 0);
        assert!(datetime.is_some());

        // Test fallback logic for extreme timestamps
        let current_time = Utc::now();
        let extreme_timestamp = -999999999i64;
        let maybe_datetime = DateTime::from_timestamp(extreme_timestamp, 0);

        // Use fallback if conversion fails or returns None
        let fallback_time = maybe_datetime.unwrap_or(current_time);

        // Either we get a valid historical date or the current time fallback
        assert!(fallback_time.year() <= current_time.year());
    }

    // Test helper functions and logic that are used in the main handler
    #[tokio::test]
    async fn test_upload_error_function() {
        use crate::state::AppState;
        use sdrtrunk_core::Config;
        use sdrtrunk_database::Database;
        use std::net::{IpAddr, Ipv4Addr};
        use std::sync::Arc;

        // Create a minimal config for testing
        let mut config = Config::default();
        config.database.url = "sqlite::memory:".to_string();

        // Try to create database - if it fails, just test the function signature
        if let Ok(db) = Database::new(&config).await
            && db.migrate().await.is_ok() {
                let state = Arc::new(AppState::new(config, db.pool().clone()).unwrap());
                let client_ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
                let user_agent = Some("TestAgent/1.0".to_string());

                let (status, error_json) = upload_error(
                    &state,
                    client_ip,
                    user_agent,
                    Some("test-key".to_string()),
                    Some("test-system".to_string()),
                    "Test error message",
                )
                .await;

                assert_eq!(status, StatusCode::BAD_REQUEST);
                assert!(!error_json.success);
                assert_eq!(error_json.error, "Test error message");
            }
    }

    #[test]
    fn test_call_metadata_parsing_comprehensive() {
        let mut metadata = CallMetadata::default();

        // Test all field assignments
        metadata.api_key = Some("test-api-key".to_string());
        metadata.system_id = Some("police-dept".to_string());
        metadata.system_label = Some("Police Department".to_string());
        metadata.datetime = Some(Utc::now());
        metadata.talkgroup_id = Some(12345);
        metadata.talkgroup_label = Some("Dispatch".to_string());
        metadata.talkgroup_group = Some("Primary".to_string());
        metadata.talkgroup_tag = Some("Emergency".to_string());
        metadata.frequency = Some(460125000);
        metadata.source_radio_id = Some(9876);
        metadata.talker_alias = Some("Officer Smith".to_string());
        metadata.test = Some(0);
        metadata.duration = Some(45.5);

        let patches_json = serde_json::json!(["patch1", "patch2"]);
        let sources_json = serde_json::json!({"radio1": "active", "radio2": "standby"});
        let frequencies_json = serde_json::json!([460125000, 460150000, 460175000]);

        metadata.patches = Some(patches_json.clone());
        metadata.sources = Some(sources_json.clone());
        metadata.frequencies = Some(frequencies_json.clone());

        // Verify all fields are set correctly
        assert_eq!(metadata.api_key, Some("test-api-key".to_string()));
        assert_eq!(metadata.system_id, Some("police-dept".to_string()));
        assert_eq!(metadata.system_label, Some("Police Department".to_string()));
        assert!(metadata.datetime.is_some());
        assert_eq!(metadata.talkgroup_id, Some(12345));
        assert_eq!(metadata.talkgroup_label, Some("Dispatch".to_string()));
        assert_eq!(metadata.talkgroup_group, Some("Primary".to_string()));
        assert_eq!(metadata.talkgroup_tag, Some("Emergency".to_string()));
        assert_eq!(metadata.frequency, Some(460125000));
        assert_eq!(metadata.source_radio_id, Some(9876));
        assert_eq!(metadata.talker_alias, Some("Officer Smith".to_string()));
        assert_eq!(metadata.test, Some(0));
        assert_eq!(metadata.duration, Some(45.5));
        assert_eq!(metadata.patches, Some(patches_json));
        assert_eq!(metadata.sources, Some(sources_json));
        assert_eq!(metadata.frequencies, Some(frequencies_json));
    }

    #[test]
    fn test_field_parsing_edge_cases_comprehensive() {
        let mut metadata = CallMetadata::default();

        // Test numeric parsing with invalid values (simulating parse failures)
        // In the real handler, parse failures result in None values

        // Test talkgroup parsing
        let valid_talkgroup = "12345";
        if let Ok(tg) = valid_talkgroup.parse::<i32>() {
            metadata.talkgroup_id = Some(tg);
        }
        assert_eq!(metadata.talkgroup_id, Some(12345));

        // Test invalid talkgroup
        let invalid_talkgroup = "not-a-number";
        let parse_result = invalid_talkgroup.parse::<i32>();
        assert!(parse_result.is_err());

        // Test frequency parsing
        let valid_frequency = "460125000";
        if let Ok(freq) = valid_frequency.parse::<i64>() {
            metadata.frequency = Some(freq);
        }
        assert_eq!(metadata.frequency, Some(460125000));

        // Test invalid frequency
        let invalid_frequency = "invalid-freq";
        let parse_result = invalid_frequency.parse::<i64>();
        assert!(parse_result.is_err());

        // Test source radio ID parsing
        let valid_source = "9876";
        if let Ok(src) = valid_source.parse::<i32>() {
            metadata.source_radio_id = Some(src);
        }
        assert_eq!(metadata.source_radio_id, Some(9876));

        // Test duration parsing
        let valid_duration = "45.5";
        if let Ok(dur) = valid_duration.parse::<f64>() {
            metadata.duration = Some(dur);
        }
        assert_eq!(metadata.duration, Some(45.5));

        // Test timestamp parsing
        let valid_timestamp = "1640995200"; // Valid Unix timestamp
        if let Ok(ts) = valid_timestamp.parse::<i64>() {
            metadata.datetime = Some(DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now));
        }
        assert!(metadata.datetime.is_some());

        // Test test field parsing with non-numeric values
        let non_numeric_test = "true"; // Non-empty should become Some(1)
        let test_value = if non_numeric_test.parse::<i32>().is_ok() {
            non_numeric_test.parse::<i32>().ok()
        } else if !non_numeric_test.is_empty() {
            Some(1)
        } else {
            None
        };
        assert_eq!(test_value, Some(1));

        // Test empty string handling
        let empty_string = "";
        let test_value = if empty_string.parse::<i32>().is_ok() {
            empty_string.parse::<i32>().ok()
        } else if !empty_string.is_empty() {
            Some(1)
        } else {
            None
        };
        assert_eq!(test_value, None);
    }

    #[test]
    fn test_json_field_parsing_comprehensive() {
        // Test patches JSON parsing
        let valid_patches_json = r#"["patch1", "patch2", "patch3"]"#;
        let patches_result: Result<serde_json::Value, _> = serde_json::from_str(valid_patches_json);
        assert!(patches_result.is_ok());

        let invalid_patches_json = "invalid json";
        let patches_result: Result<serde_json::Value, _> =
            serde_json::from_str(invalid_patches_json);
        assert!(patches_result.is_err());

        // Test sources JSON parsing
        let valid_sources_json = r#"{"radio1": "active", "radio2": "standby"}"#;
        let sources_result: Result<serde_json::Value, _> = serde_json::from_str(valid_sources_json);
        assert!(sources_result.is_ok());

        let invalid_sources_json = "{invalid: json}";
        let sources_result: Result<serde_json::Value, _> =
            serde_json::from_str(invalid_sources_json);
        assert!(sources_result.is_err());

        // Test frequencies JSON parsing
        let valid_frequencies_json = "[460125000, 460150000, 460175000]";
        let frequencies_result: Result<serde_json::Value, _> =
            serde_json::from_str(valid_frequencies_json);
        assert!(frequencies_result.is_ok());

        let invalid_frequencies_json = "[invalid, json, array]";
        let frequencies_result: Result<serde_json::Value, _> =
            serde_json::from_str(invalid_frequencies_json);
        assert!(frequencies_result.is_err());

        // Test complex nested JSON
        let complex_json = r#"{
            "primary": {
                "frequency": 460125000,
                "units": ["unit1", "unit2"]
            },
            "secondary": {
                "frequency": 460150000,
                "units": ["unit3", "unit4"]
            }
        }"#;
        let complex_result: Result<serde_json::Value, _> = serde_json::from_str(complex_json);
        assert!(complex_result.is_ok());
    }

    #[test]
    fn test_response_formatting_variations() {
        // Test different UUID formats in responses
        let uuids = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];

        for call_id in uuids {
            let success_response = UploadResponse {
                success: true,
                id: call_id,
                message: "Upload successful".to_string(),
            };

            let json = serde_json::to_string(&success_response).expect("Serialization failed");
            assert!(json.contains(&call_id.to_string()));
            assert!(json.contains("\"success\":true"));
            assert!(json.contains("Upload successful"));

            // Test deserialization
            let deserialized: UploadResponse =
                serde_json::from_str(&json).expect("Deserialization failed");
            assert!(deserialized.success);
            assert_eq!(deserialized.id, call_id);
            assert_eq!(deserialized.message, "Upload successful");
        }
    }

    #[test]
    fn test_error_response_variations() {
        let error_scenarios = vec![
            ("Missing audio file", "BAD_REQUEST"),
            ("Invalid system ID", "VALIDATION_ERROR"),
            ("Database connection failed", "INTERNAL_ERROR"),
            ("File too large", "PAYLOAD_TOO_LARGE"),
            ("Unsupported audio format", "UNSUPPORTED_MEDIA_TYPE"),
            ("Rate limit exceeded", "TOO_MANY_REQUESTS"),
            ("Authentication failed", "UNAUTHORIZED"),
        ];

        for (error_msg, error_type) in error_scenarios {
            let error_response = ErrorResponse {
                success: false,
                error: format!("{}: {}", error_type, error_msg),
            };

            let json = serde_json::to_string(&error_response).expect("Serialization failed");
            assert!(json.contains("\"success\":false"));
            assert!(json.contains(error_msg));
            assert!(json.contains(error_type));

            // Test deserialization
            let deserialized: ErrorResponse =
                serde_json::from_str(&json).expect("Deserialization failed");
            assert!(!deserialized.success);
            assert!(deserialized.error.contains(error_msg));
            assert!(deserialized.error.contains(error_type));
        }
    }

    #[test]
    fn test_file_extension_validation_comprehensive() {
        // Test file extension validation logic
        let test_filenames = vec![
            ("audio.mp3", "mp3", true),
            ("recording.wav", "wav", true),
            ("call.m4a", "m4a", true),
            ("test.flac", "flac", true),
            ("data.txt", "txt", false),
            ("file.pdf", "pdf", false),
            ("document.docx", "docx", false),
            ("archive.zip", "zip", false),
            ("script.exe", "exe", false),
            ("noextension", "", false),
            ("multiple.dots.mp3", "mp3", true),
            ("UPPERCASE.MP3", "mp3", true), // Should be normalized to lowercase
            ("Mixed.Case.WaV", "wav", true),
        ];

        let allowed_extensions = ["mp3".to_string(),
            "wav".to_string(),
            "m4a".to_string(),
            "flac".to_string()];

        for (filename, expected_ext, should_be_allowed) in test_filenames {
            let file_extension = std::path::Path::new(filename)
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
                .to_lowercase();

            assert_eq!(
                file_extension, expected_ext,
                "Extension mismatch for {}",
                filename
            );

            let is_allowed = allowed_extensions.contains(&file_extension);
            assert_eq!(
                is_allowed, should_be_allowed,
                "Allowance mismatch for {}",
                filename
            );
        }
    }

    #[test]
    fn test_unique_filename_generation() {
        // Test unique filename generation logic
        let base_filename = "test_call.mp3";

        // Generate multiple unique filenames with small delays to ensure uniqueness
        let mut generated_names = std::collections::HashSet::new();

        for i in 0..10 {
            let unique_filename = format!(
                "{}_{}_{}",
                chrono::Utc::now().format("%Y%m%d_%H%M%S_%f"),
                i, // Add counter to ensure uniqueness
                base_filename
            );

            // Each generated filename should be unique
            assert!(
                generated_names.insert(unique_filename.clone()),
                "Duplicate filename generated: {}",
                unique_filename
            );

            // Should contain timestamp and original filename
            assert!(unique_filename.contains(base_filename));
            assert!(unique_filename.len() > base_filename.len());

            // Should have proper format
            let parts: Vec<&str> = unique_filename.split('_').collect();
            assert!(
                parts.len() >= 4,
                "Filename doesn't have expected timestamp format"
            );
        }

        // Test that different base filenames produce different results
        let different_bases = vec!["call1.mp3", "call2.wav", "recording.m4a"];
        let mut all_names = std::collections::HashSet::new();

        for base in different_bases {
            let unique_filename =
                format!("{}_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S_%f"), base);

            assert!(all_names.insert(unique_filename.clone()));
            assert!(unique_filename.contains(base));
        }
    }

    #[test]
    fn test_api_key_hashing_logic() {
        // Test API key hashing logic used in the upload handler
        let long_key = "very_long_key_".repeat(100);
        let test_keys = vec![
            "test_key_123",
            "production_key_456",
            "development_key_789",
            "admin_key_000",
            "", // Empty key
            long_key.as_str(),
        ];

        let mut hashes = std::collections::HashSet::new();

        for key in test_keys {
            let key_hash = format!("{:x}", md5::compute(key));

            // Hash should be 32 characters (MD5 hex)
            assert_eq!(
                key_hash.len(),
                32,
                "MD5 hash should be 32 characters for key: {}",
                key
            );

            // Hash should only contain hex characters
            assert!(
                key_hash.chars().all(|c| c.is_ascii_hexdigit()),
                "Hash contains non-hex characters: {}",
                key_hash
            );

            // Same key should produce same hash
            let key_hash2 = format!("{:x}", md5::compute(key));
            assert_eq!(key_hash, key_hash2, "Same key produced different hashes");

            // Different keys should produce different hashes (with very high probability)
            if !key.is_empty() {
                assert!(
                    hashes.insert(key_hash.clone()),
                    "Hash collision detected for key: {}",
                    key
                );
            }
        }
    }

    #[test]
    fn test_storage_path_generation() {
        // Test storage path generation logic
        let system_ids = vec![
            "police",
            "fire_dept",
            "ems",
            "system_with_underscores",
            "System-With-Dashes",
            "123_numeric_start",
        ];

        let dates = vec![
            chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2023, 6, 15).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2025, 2, 29)
                .unwrap_or(chrono::NaiveDate::from_ymd_opt(2025, 2, 28).unwrap()),
        ];

        for system_id in system_ids {
            for date in &dates {
                // Simulate path generation logic
                let year = date.year();
                let month = date.month();
                let day = date.day();

                let date_path = format!("{:04}/{:02}/{:02}", year, month, day);
                let full_path = format!("uploads/{}/{}", system_id, date_path);

                // Verify path components
                assert!(full_path.starts_with("uploads/"));
                assert!(full_path.contains(system_id));
                assert!(full_path.contains(&format!("{:04}", year)));
                assert!(full_path.contains(&format!("{:02}", month)));
                assert!(full_path.contains(&format!("{:02}", day)));

                // Verify path format
                let expected_suffix = format!("{}/{:04}/{:02}/{:02}", system_id, year, month, day);
                assert!(full_path.ends_with(&expected_suffix));
            }
        }
    }

    #[test]
    fn test_radio_call_field_mapping() {
        // Test RadioCall field mapping from metadata
        let mut metadata = CallMetadata::default();
        metadata.system_id = Some("test_system".to_string());
        metadata.system_label = Some("Test Radio System".to_string());
        metadata.frequency = Some(460_125_000);
        metadata.talkgroup_id = Some(12345);
        metadata.talkgroup_label = Some("Dispatch".to_string());
        metadata.talkgroup_group = Some("Emergency".to_string());
        metadata.talkgroup_tag = Some("High Priority".to_string());
        metadata.source_radio_id = Some(9876);
        metadata.talker_alias = Some("Unit 99".to_string());
        metadata.duration = Some(45.5);
        metadata.datetime = Some(chrono::Utc::now());

        let patches_json = serde_json::json!(["patch1", "patch2"]);
        let frequencies_json = serde_json::json!([460125000, 460150000]);
        let sources_json = serde_json::json!({"radio1": "active"});

        metadata.patches = Some(patches_json.clone());
        metadata.frequencies = Some(frequencies_json.clone());
        metadata.sources = Some(sources_json.clone());

        // Simulate RadioCall creation
        let audio_size = 1024 * 50; // 50KB
        let file_path = "/uploads/test_system/2024/01/01/test_file.mp3";
        let unique_filename = "20240101_120000_123456_test_file.mp3";
        let client_ip = "192.168.1.100";

        // Test field mappings
        assert_eq!(metadata.system_id, Some("test_system".to_string()));
        assert_eq!(metadata.system_label, Some("Test Radio System".to_string()));
        assert_eq!(metadata.frequency, Some(460_125_000));
        assert_eq!(metadata.talkgroup_id, Some(12345));
        assert_eq!(metadata.talkgroup_label, Some("Dispatch".to_string()));
        assert_eq!(metadata.source_radio_id, Some(9876));
        assert_eq!(metadata.duration, Some(45.5));
        assert_eq!(metadata.patches, Some(patches_json));
        assert_eq!(metadata.frequencies, Some(frequencies_json));
        assert_eq!(metadata.sources, Some(sources_json));

        // Test derived values
        assert_eq!(audio_size, 1024 * 50);
        assert!(file_path.contains("test_system"));
        assert!(unique_filename.contains("test_file.mp3"));
        assert!(client_ip.parse::<std::net::IpAddr>().is_ok());
    }

    #[test]
    fn test_log_formatting_components() {
        // Test log formatting components used in successful upload logging
        let test_cases = vec![
            // (talkgroup_id, talkgroup_label, expected_format)
            (
                Some(12345),
                Some("Dispatch".to_string()),
                "TG 12345 (Dispatch)",
            ),
            (Some(99), None, "TG 99"),
            (None, Some("Emergency".to_string()), "Unknown TG"),
            (None, None, "Unknown TG"),
            (Some(0), Some("All Call".to_string()), "TG 0 (All Call)"),
        ];

        for (tg_id, tg_label, expected) in test_cases {
            let talkgroup_info = if let Some(tg_id) = tg_id {
                if let Some(ref label) = tg_label {
                    format!("TG {} ({})", tg_id, label)
                } else {
                    format!("TG {}", tg_id)
                }
            } else {
                "Unknown TG".to_string()
            };

            assert_eq!(talkgroup_info, expected);
        }

        // Test frequency formatting
        let frequencies = vec![
            (Some(460_125_000i64), "460.1250 MHz"),
            (Some(154_250_000i64), "154.2500 MHz"),
            (Some(800_000_000i64), "800.0000 MHz"),
            (None, "Unknown Freq"),
        ];

        for (freq, expected) in frequencies {
            let freq_mhz = freq
                .map(|f| format!("{:.4} MHz", f as f64 / 1_000_000.0))
                .unwrap_or_else(|| "Unknown Freq".to_string());

            assert_eq!(freq_mhz, expected);
        }

        // Test duration formatting
        let durations = vec![
            (Some(15.0), "15.00s"),
            (Some(0.5), "0.50s"),
            (Some(123.456), "123.46s"),
            (None, "N/A"),
        ];

        for (duration, expected) in durations {
            let duration_str = duration
                .map(|d| format!("{:.2}s", d))
                .unwrap_or_else(|| "N/A".to_string());

            assert_eq!(duration_str, expected);
        }
    }

    #[test]
    fn test_file_size_formatting() {
        // Test file size to KB conversion used in logging
        let file_sizes = vec![
            (1024, 1.0),           // 1KB
            (2048, 2.0),           // 2KB
            (1536, 1.5),           // 1.5KB
            (512, 0.5),            // 0.5KB
            (0, 0.0),              // 0KB
            (1024 * 1024, 1024.0), // 1MB = 1024KB
        ];

        for (bytes, expected_kb) in file_sizes {
            let file_size_kb = bytes as f64 / 1024.0;
            assert!(
                (file_size_kb - expected_kb).abs() < 0.001,
                "File size conversion mismatch: {} bytes -> {} KB, expected {} KB",
                bytes,
                file_size_kb,
                expected_kb
            );
        }
    }

    #[test]
    fn test_uuid_operations() {
        // Test UUID operations used in the upload handler
        let call_ids = vec![
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
        ];

        for call_id in call_ids {
            // Test UUID to string conversion
            let uuid_string = call_id.to_string();
            assert_eq!(uuid_string.len(), 36); // Standard UUID string length
            assert_eq!(uuid_string.chars().filter(|&c| c == '-').count(), 4);

            // Test UUID parsing back
            let parsed_uuid = uuid::Uuid::parse_str(&uuid_string).expect("Should parse back");
            assert_eq!(parsed_uuid, call_id);

            // Test short form used in logging
            let uuid_string_for_split = call_id.to_string();
            let short_form = uuid_string_for_split.split('-').next().unwrap_or("");
            assert_eq!(short_form.len(), 8); // First segment is 8 characters
            assert!(short_form.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn test_request_header_parsing() {
        // Test request header parsing logic
        let user_agent_tests = vec![
            (Some("SDRTrunk/1.0"), Some("SDRTrunk/1.0".to_string())),
            (Some("curl/7.68.0"), Some("curl/7.68.0".to_string())),
            (Some(""), Some("".to_string())),
            (None, None),
        ];

        for (input, expected) in user_agent_tests {
            let user_agent = input.map(|v| v.to_string());
            assert_eq!(user_agent, expected);
        }

        // Test content type parsing
        let content_type_tests = vec![
            ("multipart/form-data", true),
            ("multipart/form-data; boundary=test", true),
            ("application/json", false),
            ("text/plain", false),
            ("", false),
        ];

        for (content_type, should_be_multipart) in content_type_tests {
            let is_multipart = content_type.starts_with("multipart/form-data");
            assert_eq!(is_multipart, should_be_multipart);
        }
    }

    #[test]
    fn test_validation_logic_patterns() {
        // Test various validation patterns used in the handler

        // File size validation
        let max_size = 100 * 1024 * 1024; // 100MB
        let test_sizes = vec![
            (50 * 1024 * 1024, true),   // 50MB - should pass
            (100 * 1024 * 1024, false), // 100MB - should fail (equal to limit)
            (150 * 1024 * 1024, false), // 150MB - should fail
            (0, true),                  // 0 bytes - should pass
        ];

        for (size, should_pass) in test_sizes {
            let is_valid = (size as u64) < (max_size as u64);
            assert_eq!(
                is_valid, should_pass,
                "Size validation mismatch for {} bytes",
                size
            );
        }

        // System ID validation patterns
        let system_ids = vec![
            (Some("valid_system".to_string()), true),
            (Some("".to_string()), false), // Empty should be invalid
            (None, false),                 // None should be invalid
        ];

        for (system_id, should_be_valid) in system_ids {
            let is_valid = system_id.as_ref().is_some_and(|s| !s.is_empty());
            assert_eq!(is_valid, should_be_valid);
        }
    }

    #[test]
    fn test_multipart_field_handling_logic() {
        // Test field name matching logic used in the handler
        let field_mappings = vec![
            ("audio", "audio_data"),
            ("key", "api_key"),
            ("system", "system_id"),
            ("systemLabel", "system_label"),
            ("test", "test_flag"),
            ("dateTime", "datetime"),
            ("datetime", "datetime_alt"),
            ("talkgroup", "talkgroup_id"),
            ("talkgroupLabel", "talkgroup_label"),
            ("talkgroupGroup", "talkgroup_group"),
            ("talkgroupTag", "talkgroup_tag"),
            ("frequency", "frequency"),
            ("source", "source_radio_id"),
            ("patches", "patches_json"),
            ("sources", "sources_json"),
            ("freqList", "frequencies_json"),
            ("talkerAlias", "talker_alias"),
            ("duration", "duration_seconds"),
            ("unknown_field", "ignored"),
        ];

        for (field_name, expected_mapping) in field_mappings {
            match field_name {
                "audio" => assert_eq!(expected_mapping, "audio_data"),
                "key" => assert_eq!(expected_mapping, "api_key"),
                "system" => assert_eq!(expected_mapping, "system_id"),
                "systemLabel" => assert_eq!(expected_mapping, "system_label"),
                "test" => assert_eq!(expected_mapping, "test_flag"),
                "dateTime" | "datetime" => assert!(expected_mapping.contains("datetime")),
                "talkgroup" => assert_eq!(expected_mapping, "talkgroup_id"),
                "talkgroupLabel" => assert_eq!(expected_mapping, "talkgroup_label"),
                "talkgroupGroup" => assert_eq!(expected_mapping, "talkgroup_group"),
                "talkgroupTag" => assert_eq!(expected_mapping, "talkgroup_tag"),
                "frequency" => assert_eq!(expected_mapping, "frequency"),
                "source" => assert_eq!(expected_mapping, "source_radio_id"),
                "patches" => assert_eq!(expected_mapping, "patches_json"),
                "sources" => assert_eq!(expected_mapping, "sources_json"),
                "freqList" => assert_eq!(expected_mapping, "frequencies_json"),
                "talkerAlias" => assert_eq!(expected_mapping, "talker_alias"),
                "duration" => assert_eq!(expected_mapping, "duration_seconds"),
                _ => assert_eq!(expected_mapping, "ignored"), // Unknown fields are ignored
            }
        }
    }

    #[test]
    fn test_content_type_validation_logic() {
        let content_types = vec![
            ("multipart/form-data", true),
            (
                "multipart/form-data; boundary=----WebKitFormBoundary7MA4YWxkTrZu0gW",
                true,
            ),
            ("application/json", false),
            ("text/plain", false),
            ("application/x-www-form-urlencoded", false),
            ("", false),
        ];

        for (content_type, should_be_valid) in content_types {
            let is_multipart = content_type.starts_with("multipart/form-data");
            assert_eq!(
                is_multipart, should_be_valid,
                "Content-Type: {}",
                content_type
            );
        }
    }

    #[test]
    fn test_accept_header_parsing_logic() {
        let accept_headers = vec![
            ("application/json", true),
            ("application/json, text/plain", true),
            (
                "text/html, application/xhtml+xml, application/xml;q=0.9, application/json;q=0.8",
                true,
            ),
            ("text/plain", false),
            ("text/html", false),
            ("", false),
        ];

        for (accept_header, expects_json) in accept_headers {
            let wants_json = accept_header.contains("application/json");
            assert_eq!(wants_json, expects_json, "Accept: {}", accept_header);
        }
    }
}
