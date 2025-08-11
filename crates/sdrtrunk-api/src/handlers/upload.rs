//! File upload handler for Rdio-compatible call uploads

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
#[derive(serde::Serialize)]
pub struct UploadResponse {
    pub success: bool,
    pub id: Uuid,
    pub message: String,
}

/// Response for upload error
#[derive(serde::Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}

/// Handle multipart form data upload from Rdio Scanner
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
    let mut multipart = match Multipart::from_request(request, &state).await {
        Ok(multipart) => multipart,
        Err(_) => {
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
        }
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
                        if let Ok(text) = field.text().await {
                            if let Ok(ts) = text.parse::<i64>() {
                                metadata.datetime =
                                    Some(DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now));
                            }
                        }
                    }
                    "talkgroup" => {
                        if let Ok(text) = field.text().await {
                            if let Ok(tg) = text.parse::<i32>() {
                                metadata.talkgroup_id = Some(tg);
                            }
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
                        if let Ok(text) = field.text().await {
                            if let Ok(freq) = text.parse::<i64>() {
                                metadata.frequency = Some(freq);
                            }
                        }
                    }
                    "source" => {
                        if let Ok(text) = field.text().await {
                            if let Ok(src) = text.parse::<i32>() {
                                metadata.source_radio_id = Some(src);
                            }
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
                    &format!("Invalid multipart data: {}", e),
                )
                .await;
                return (status, json_error).into_response();
            }
        }
    }

    // Handle test requests first - they don't require audio files
    if metadata.test.is_some() {
        info!("Test request from system {:?}", metadata.system_id);

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
        } else {
            // Return plain text response (default behavior) with explicit content-type
            return Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/plain")
                .body(Body::from(message))
                .unwrap()
                .into_response();
        }
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
            &format!("File extension '{}' is not allowed", file_extension),
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
        duration_seconds: None, // Will be calculated later if needed
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

    info!(
        "Successfully uploaded call {} for system {}",
        call_id, system_id
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
    error!("Upload error: {}", error_message);

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
    patches: Option<serde_json::Value>,
    sources: Option<serde_json::Value>,
    frequencies: Option<serde_json::Value>,
}
