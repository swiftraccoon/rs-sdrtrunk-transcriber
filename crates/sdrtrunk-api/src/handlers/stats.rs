//! System statistics endpoint for monitoring and analytics

use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};
use validator::Validate;

/// Query parameters for statistics
#[derive(Debug, Deserialize, Validate)]
pub struct StatsQuery {
    /// Include detailed talkgroup statistics
    pub include_talkgroups: Option<bool>,

    /// Include upload source statistics
    pub include_sources: Option<bool>,

    /// Include hourly call distribution
    pub include_hourly: Option<bool>,

    /// Time period for statistics (hours)
    #[validate(range(min = 1, max = 8760))] // Max 1 year
    pub time_period_hours: Option<i32>,
}

/// System statistics response
#[derive(Debug, Serialize)]
pub struct SystemStatsResponse {
    /// System information
    pub system_id: String,
    pub system_label: Option<String>,

    /// Call counts
    pub call_counts: CallCounts,

    /// Time information
    pub time_info: TimeInfo,

    /// Top talkgroups (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_talkgroups: Option<Vec<TalkgroupStats>>,

    /// Upload sources (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_sources: Option<Vec<UploadSourceStats>>,

    /// Hourly call distribution (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hourly_distribution: Option<Vec<HourlyStats>>,

    /// Last update timestamp
    pub last_updated: chrono::DateTime<chrono::Utc>,

    /// Statistics generation timestamp
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

/// Call count information
#[derive(Debug, Serialize)]
pub struct CallCounts {
    /// Total calls ever recorded
    pub total_calls: i32,

    /// Calls received today
    pub calls_today: i32,

    /// Calls received this hour
    pub calls_this_hour: i32,

    /// Calls in the last 24 hours
    pub calls_last_24h: i32,

    /// Calls in the last 7 days
    pub calls_last_7d: i32,

    /// Average calls per day (last 30 days)
    pub avg_calls_per_day: f64,
}

/// Time-related information
#[derive(Debug, Serialize)]
pub struct TimeInfo {
    /// When the system was first seen
    pub first_seen: Option<chrono::DateTime<chrono::Utc>>,

    /// When the system was last seen
    pub last_seen: Option<chrono::DateTime<chrono::Utc>>,

    /// Days since first seen
    pub days_active: Option<i32>,

    /// System activity status
    pub activity_status: ActivityStatus,
}

/// Activity status enum
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityStatus {
    /// Active (calls in last hour)
    Active,
    /// Recently active (calls in last 24 hours)
    RecentlyActive,
    /// Inactive (no calls in last 24 hours)
    Inactive,
    /// Unknown status
    Unknown,
}

/// Talkgroup statistics
#[derive(Debug, Serialize)]
pub struct TalkgroupStats {
    /// Talkgroup ID
    pub talkgroup_id: i32,

    /// Talkgroup label
    pub talkgroup_label: Option<String>,

    /// Talkgroup group
    pub talkgroup_group: Option<String>,

    /// Number of calls
    pub call_count: i32,

    /// Percentage of total calls
    pub percentage: f64,

    /// Last activity timestamp
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
}

/// Upload source statistics
#[derive(Debug, Serialize)]
pub struct UploadSourceStats {
    /// Source IP address
    pub source_ip: String,

    /// Number of uploads
    pub upload_count: i32,

    /// Percentage of total uploads
    pub percentage: f64,

    /// Last upload timestamp
    pub last_upload: Option<chrono::DateTime<chrono::Utc>>,

    /// API key used (if any)
    pub api_key_id: Option<String>,
}

/// Hourly call statistics
#[derive(Debug, Serialize)]
pub struct HourlyStats {
    /// Hour (0-23)
    pub hour: i32,

    /// Number of calls in this hour
    pub call_count: i32,

    /// Average call duration in seconds
    pub avg_duration: Option<f64>,
}

/// Global statistics response
#[derive(Debug, Serialize)]
pub struct GlobalStatsResponse {
    /// Total number of systems
    pub total_systems: i32,

    /// Total calls across all systems
    pub total_calls: i64,

    /// Calls in the last 24 hours
    pub calls_last_24h: i64,

    /// Most active systems
    pub top_systems: Vec<SystemSummary>,

    /// Recent activity timeline
    pub recent_activity: Vec<ActivityPeriod>,

    /// Storage statistics
    pub storage_stats: StorageStats,

    /// Generated timestamp
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

/// System summary for global stats
#[derive(Debug, Serialize)]
pub struct SystemSummary {
    /// System ID
    pub system_id: String,

    /// System label
    pub system_label: Option<String>,

    /// Total calls
    pub call_count: i32,

    /// Last activity
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
}

/// Activity period for timeline
#[derive(Debug, Serialize)]
pub struct ActivityPeriod {
    /// Start of period
    pub period_start: chrono::DateTime<chrono::Utc>,

    /// Number of calls in period
    pub call_count: i32,

    /// Number of active systems
    pub active_systems: i32,
}

/// Storage statistics
#[derive(Debug, Serialize)]
pub struct StorageStats {
    /// Total files stored
    pub total_files: i64,

    /// Total storage used in bytes
    pub total_size_bytes: i64,

    /// Average file size in bytes
    pub avg_file_size: i64,

    /// Storage path
    pub storage_path: String,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,
    /// Error code
    pub code: String,
}

/// Get system statistics
pub async fn get_system_stats(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
    Query(query): Query<StatsQuery>,
) -> Result<Json<SystemStatsResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate query parameters
    if let Err(validation_errors) = query.validate() {
        warn!("Invalid query parameters: {:?}", validation_errors);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid query parameters".to_string(),
                code: "INVALID_PARAMETERS".to_string(),
            }),
        ));
    }

    info!("Retrieving statistics for system: {}", system_id);

    // Get basic system stats from database
    let system_stats = match sdrtrunk_database::get_system_stats(&state.pool, &system_id).await {
        Ok(system_stats) => system_stats,
        Err(e) => {
            error!("Failed to retrieve system stats: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to retrieve statistics".to_string(),
                    code: "DATABASE_ERROR".to_string(),
                }),
            ));
        }
    };

    // Calculate additional metrics
    let (calls_last_24h, calls_last_7d, avg_calls_per_day) =
        match calculate_additional_metrics(&state.pool, &system_id).await {
            Ok(metrics) => metrics,
            Err(e) => {
                warn!("Failed to calculate additional metrics: {}", e);
                (0, 0, 0.0) // Fallback values
            }
        };

    // Determine activity status
    let activity_status =
        determine_activity_status(system_stats.calls_this_hour.unwrap_or(0), calls_last_24h);

    // Calculate days active
    let days_active = system_stats
        .first_seen
        .map(|first_seen| (chrono::Utc::now() - first_seen).num_days() as i32);

    // Build response
    let mut response = SystemStatsResponse {
        system_id: system_stats.system_id.clone(),
        system_label: system_stats.system_label.clone(),
        call_counts: CallCounts {
            total_calls: system_stats.total_calls.unwrap_or(0),
            calls_today: system_stats.calls_today.unwrap_or(0),
            calls_this_hour: system_stats.calls_this_hour.unwrap_or(0),
            calls_last_24h,
            calls_last_7d,
            avg_calls_per_day,
        },
        time_info: TimeInfo {
            first_seen: system_stats.first_seen,
            last_seen: system_stats.last_seen,
            days_active,
            activity_status,
        },
        top_talkgroups: None,
        upload_sources: None,
        hourly_distribution: None,
        last_updated: system_stats.last_updated,
        generated_at: chrono::Utc::now(),
    };

    // Add optional detailed stats
    if query.include_talkgroups.unwrap_or(false) {
        response.top_talkgroups = Some(get_talkgroup_stats(&state.pool, &system_id));
    }

    if query.include_sources.unwrap_or(false) {
        response.upload_sources = Some(get_upload_source_stats(&state.pool, &system_id));
    }

    if query.include_hourly.unwrap_or(false) {
        response.hourly_distribution = Some(get_hourly_stats(&state.pool, &system_id));
    }

    info!(
        "Successfully retrieved statistics for system: {}",
        system_id
    );
    Ok(Json(response))
}

/// Get global statistics across all systems
pub async fn get_global_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<GlobalStatsResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Retrieving global statistics");

    // Get total systems count
    let total_systems = match sdrtrunk_database::count_systems(&state.pool).await {
        Ok(count) => count,
        Err(e) => {
            error!("Failed to count systems: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to retrieve global statistics".to_string(),
                    code: "DATABASE_ERROR".to_string(),
                }),
            ));
        }
    };

    // Get total calls
    let total_calls = match sdrtrunk_database::count_radio_calls(&state.pool).await {
        Ok(count) => count,
        Err(e) => {
            error!("Failed to count calls: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to retrieve global statistics".to_string(),
                    code: "DATABASE_ERROR".to_string(),
                }),
            ));
        }
    };

    // Get calls in last 24 hours
    let calls_last_24h = match sdrtrunk_database::count_recent_calls(&state.pool, 24).await {
        Ok(count) => count,
        Err(e) => {
            warn!("Failed to count recent calls: {}", e);
            0
        }
    };

    // Get top systems
    let top_systems = match sdrtrunk_database::get_top_systems(&state.pool, 10).await {
        Ok(systems) => systems
            .into_iter()
            .map(|(system_id, call_count)| SystemSummary {
                system_id,
                system_label: None,
                call_count: call_count.try_into().unwrap_or(0),
                last_activity: None,
            })
            .collect(),
        Err(e) => {
            warn!("Failed to get top systems: {}", e);
            Vec::new()
        }
    };

    // Get storage stats
    let storage_stats = calculate_storage_stats(&state.config.storage.base_dir);

    let response = GlobalStatsResponse {
        total_systems: total_systems.try_into().unwrap_or(0),
        total_calls,
        calls_last_24h,
        top_systems,
        recent_activity: Vec::new(), // TODO: Implement activity timeline
        storage_stats,
        generated_at: chrono::Utc::now(),
    };

    info!("Successfully retrieved global statistics");
    Ok(Json(response))
}

/// Calculate additional metrics for a system
async fn calculate_additional_metrics(
    pool: &sqlx::PgPool,
    system_id: &str,
) -> Result<(i32, i32, f64), sdrtrunk_core::Error> {
    // This would be implemented with proper SQL queries
    // For now, returning placeholder values
    let calls_last_24h = sdrtrunk_database::count_system_calls_since(pool, system_id, 24)
        .await
        .unwrap_or(0);

    let calls_last_7d = sdrtrunk_database::count_system_calls_since(
        pool, system_id, 168, // 7 days * 24 hours
    )
    .await
    .unwrap_or(0);

    #[allow(clippy::cast_precision_loss)]
    let avg_calls_per_day = calls_last_7d as f64 / 7.0;

    Ok((
        calls_last_24h.try_into().unwrap_or(0),
        calls_last_7d.try_into().unwrap_or(0),
        avg_calls_per_day,
    ))
}

/// Determine activity status based on call counts
fn determine_activity_status(calls_this_hour: i32, calls_last_24h: i32) -> ActivityStatus {
    if calls_this_hour > 0 {
        ActivityStatus::Active
    } else if calls_last_24h > 0 {
        ActivityStatus::RecentlyActive
    } else {
        ActivityStatus::Inactive
    }
}

/// Get talkgroup statistics for a system
fn get_talkgroup_stats(_pool: &sqlx::PgPool, _system_id: &str) -> Vec<TalkgroupStats> {
    // This would query the database for talkgroup stats
    // Placeholder implementation
    Vec::new()
}

/// Get upload source statistics for a system
fn get_upload_source_stats(_pool: &sqlx::PgPool, _system_id: &str) -> Vec<UploadSourceStats> {
    // This would query the database for upload source stats
    // Placeholder implementation
    Vec::new()
}

/// Get hourly call distribution for a system
fn get_hourly_stats(_pool: &sqlx::PgPool, _system_id: &str) -> Vec<HourlyStats> {
    // This would query the database for hourly distribution
    // Placeholder implementation
    let mut hourly_stats = Vec::new();
    for hour in 0..24 {
        hourly_stats.push(HourlyStats {
            hour,
            call_count: 0,
            avg_duration: None,
        });
    }
    hourly_stats
}

/// Calculate storage statistics
fn calculate_storage_stats(storage_path: &std::path::Path) -> StorageStats {
    // This is a simplified implementation
    // In production, you might want to use more sophisticated file system analysis
    StorageStats {
        total_files: 0,
        total_size_bytes: 0,
        avg_file_size: 0,
        storage_path: storage_path.display().to_string(),
    }
}
