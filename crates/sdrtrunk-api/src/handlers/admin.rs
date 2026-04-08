//! Admin API handlers for system administration

use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sdrtrunk_storage::queries::{ApiKeyQueries, CreateApiKeyParams};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tracing::{error, info};

/// Request to create a new API key
#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    /// Description of the API key
    pub description: Option<String>,
    /// Expiration timestamp (ISO 8601 format)
    pub expires_at: Option<String>,
    /// Allowed IP addresses
    pub allowed_ips: Option<Vec<String>>,
    /// Allowed system IDs
    pub allowed_systems: Option<Vec<String>>,
}

/// Response containing a newly created API key
#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    /// Whether the operation was successful
    pub success: bool,
    /// The generated API key (plain text, only shown once)
    pub api_key: String,
    /// The key ID for management
    pub key_id: String,
    /// Additional message
    pub message: String,
}

/// Response containing API key details
#[derive(Debug, Serialize)]
pub struct ApiKeyDetailsResponse {
    /// Whether the operation was successful
    pub success: bool,
    /// Key ID
    pub id: String,
    /// Description
    pub description: Option<String>,
    /// Creation timestamp
    pub created_at: String,
    /// Expiration timestamp
    pub expires_at: Option<String>,
    /// Allowed IPs
    pub allowed_ips: Option<Vec<String>>,
    /// Allowed systems
    pub allowed_systems: Option<Vec<String>>,
    /// Whether the key is active
    pub active: bool,
    /// Last used timestamp
    pub last_used: Option<String>,
    /// Total number of requests
    pub total_requests: Option<i32>,
}

/// Response for API key deletion
#[derive(Debug, Serialize)]
pub struct DeleteApiKeyResponse {
    /// Whether the operation was successful
    pub success: bool,
    /// Message about the operation
    pub message: String,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Whether the operation was successful (always false)
    pub success: bool,
    /// Error message
    pub error: String,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(self)).into_response()
    }
}

/// Create a new API key
///
/// # Errors
///
/// Returns error if database operation fails or validation fails
pub async fn create_api_key(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateApiKeyRequest>,
) -> Result<Json<CreateApiKeyResponse>, ErrorResponse> {
    info!(
        "Creating new API key with description: {:?}",
        request.description
    );

    // Generate a random API key (32 bytes = 64 hex characters)
    let api_key = uuid::Uuid::new_v4().to_string().replace('-', "");

    // Hash the API key for storage using SHA-256 (cryptographically secure)
    let mut hasher = Sha256::new();
    hasher.update(api_key.as_bytes());
    let key_hash = format!("{:x}", hasher.finalize());

    // Parse expiration date if provided
    let expires_at = if let Some(expires_str) = request.expires_at {
        match chrono::DateTime::parse_from_rfc3339(&expires_str) {
            Ok(dt) => Some(dt.with_timezone(&chrono::Utc)),
            Err(e) => {
                return Err(ErrorResponse {
                    success: false,
                    error: format!("Invalid expiration date format: {e}"),
                });
            }
        }
    } else {
        None
    };

    // Create the API key in the database
    match ApiKeyQueries::create(
        &state.pool,
        CreateApiKeyParams {
            key_hash: &key_hash,
            description: request.description.as_deref(),
            expires_at,
            allowed_ips: request.allowed_ips,
            allowed_systems: request.allowed_systems,
        },
    )
    .await
    {
        Ok(api_key_db) => {
            info!("Successfully created API key: {}", api_key_db.id);
            Ok(Json(CreateApiKeyResponse {
                success: true,
                api_key: api_key.clone(),
                key_id: api_key_db.id,
                message: "API key created successfully. Store this key securely - it will not be shown again.".to_string(),
            }))
        }
        Err(e) => {
            error!("Failed to create API key: {e}");
            Err(ErrorResponse {
                success: false,
                error: format!("Failed to create API key: {e}"),
            })
        }
    }
}

/// Get API key details by ID
///
/// # Errors
///
/// Returns error if key not found or database error
pub async fn get_api_key_details(
    State(state): State<Arc<AppState>>,
    Path(key_id): Path<String>,
) -> Result<Json<ApiKeyDetailsResponse>, ErrorResponse> {
    info!("Fetching API key details for: {key_id}");

    match ApiKeyQueries::find_by_id(&state.pool, &key_id).await {
        Ok(api_key) => Ok(Json(ApiKeyDetailsResponse {
            success: true,
            id: api_key.id,
            description: api_key.description,
            created_at: api_key.created_at.to_rfc3339(),
            expires_at: api_key.expires_at.map(|dt| dt.to_rfc3339()),
            allowed_ips: api_key.allowed_ips,
            allowed_systems: api_key.allowed_systems,
            active: api_key.active,
            last_used: api_key.last_used.map(|dt| dt.to_rfc3339()),
            total_requests: api_key.total_requests,
        })),
        Err(e) => {
            error!("Failed to fetch API key {key_id}: {e}");
            Err(ErrorResponse {
                success: false,
                error: format!("API key not found: {e}"),
            })
        }
    }
}

/// Delete (deactivate) an API key
///
/// # Errors
///
/// Returns error if key not found or database error
pub async fn delete_api_key(
    State(state): State<Arc<AppState>>,
    Path(key_id): Path<String>,
) -> Result<Json<DeleteApiKeyResponse>, ErrorResponse> {
    info!("Deleting API key: {key_id}");

    match ApiKeyQueries::delete(&state.pool, &key_id).await {
        Ok(()) => {
            info!("Successfully deleted API key: {key_id}");
            Ok(Json(DeleteApiKeyResponse {
                success: true,
                message: format!("API key {key_id} has been deactivated"),
            }))
        }
        Err(e) => {
            error!("Failed to delete API key {key_id}: {e}");
            Err(ErrorResponse {
                success: false,
                error: format!("Failed to delete API key: {e}"),
            })
        }
    }
}

/// List all active API keys
///
/// # Errors
///
/// Returns error if database operation fails
pub async fn list_api_keys(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ApiKeyDetailsResponse>>, ErrorResponse> {
    info!("Listing all active API keys");

    match ApiKeyQueries::get_all_active(&state.pool).await {
        Ok(keys) => Ok(Json(
            keys.into_iter()
                .map(|k| ApiKeyDetailsResponse {
                    success: true,
                    id: k.id,
                    description: k.description,
                    created_at: k.created_at.to_rfc3339(),
                    expires_at: k.expires_at.map(|dt| dt.to_rfc3339()),
                    allowed_ips: k.allowed_ips,
                    allowed_systems: k.allowed_systems,
                    active: k.active,
                    last_used: k.last_used.map(|dt| dt.to_rfc3339()),
                    total_requests: k.total_requests,
                })
                .collect(),
        )),
        Err(e) => {
            error!("Failed to list API keys: {e}");
            Err(ErrorResponse {
                success: false,
                error: format!("Failed to list API keys: {e}"),
            })
        }
    }
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
    fn test_create_api_key_request_deserialization() {
        let json = r#"{"description":"Test key","expires_at":"2025-12-31T23:59:59Z"}"#;
        let request: CreateApiKeyRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.description, Some("Test key".to_string()));
        assert!(request.expires_at.is_some());
    }

    #[test]
    fn test_error_response_serialization() {
        let error = ErrorResponse {
            success: false,
            error: "Test error".to_string(),
        };
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("Test error"));
        assert!(json.contains("false"));
    }
}
