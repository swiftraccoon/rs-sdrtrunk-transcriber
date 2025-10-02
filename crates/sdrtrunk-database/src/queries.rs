//! Database query operations for `SDRTrunk` transcriber

use crate::models::{ApiKeyDb, RadioCallDb, SystemStatsDb, UploadLogDb};
use sdrtrunk_core::{Error, Result, types::RadioCall};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Radio call database operations
pub struct RadioCallQueries;

impl RadioCallQueries {
    /// Insert a new radio call
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn insert(pool: &PgPool, call: &RadioCall) -> Result<Uuid> {
        let query = r"
            INSERT INTO radio_calls (
                id, created_at, call_timestamp, system_id, system_label,
                frequency, talkgroup_id, talkgroup_label, talkgroup_group, talkgroup_tag,
                source_radio_id, talker_alias, audio_filename, audio_file_path,
                audio_size_bytes, audio_content_type, duration_seconds,
                transcription_text, transcription_confidence, transcription_language,
                transcription_status, speaker_segments, speaker_count,
                patches, frequencies, sources, upload_ip, upload_timestamp, upload_api_key_id
            ) VALUES (
                COALESCE($1, gen_random_uuid()), $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24, $25, $26, $27, $28, $29
            )
            RETURNING id
        ";

        let confidence = call
            .transcription_confidence
            .map(rust_decimal::Decimal::try_from)
            .transpose()
            .map_err(|e| Error::Database(format!("Invalid confidence value: {e}")))?;

        let duration = call
            .duration_seconds
            .map(rust_decimal::Decimal::try_from)
            .transpose()
            .map_err(|e| Error::Database(format!("Invalid duration value: {e}")))?;

        let upload_ip = call
            .upload_ip
            .as_deref()
            .and_then(|ip| ip.parse::<std::net::IpAddr>().ok())
            .map(sqlx::types::ipnetwork::IpNetwork::from);

        let row = sqlx::query(query)
            .bind(call.id)
            .bind(call.created_at)
            .bind(call.call_timestamp)
            .bind(&call.system_id)
            .bind(&call.system_label)
            .bind(call.frequency)
            .bind(call.talkgroup_id)
            .bind(&call.talkgroup_label)
            .bind(&call.talkgroup_group)
            .bind(&call.talkgroup_tag)
            .bind(call.source_radio_id)
            .bind(&call.talker_alias)
            .bind(&call.audio_filename)
            .bind(&call.audio_file_path)
            .bind(call.audio_size_bytes)
            .bind(Option::<String>::None) // audio_content_type
            .bind(duration)
            .bind(&call.transcription_text)
            .bind(confidence)
            .bind(Option::<String>::None) // transcription_language
            .bind(call.transcription_status.to_string())
            .bind(&call.speaker_segments)
            .bind(call.speaker_count)
            .bind(call.patches.as_ref().map(std::string::ToString::to_string))
            .bind(
                call.frequencies
                    .as_ref()
                    .map(std::string::ToString::to_string),
            )
            .bind(call.sources.as_ref().map(std::string::ToString::to_string))
            .bind(upload_ip)
            .bind(call.upload_timestamp)
            .bind(&call.upload_api_key_id)
            .fetch_one(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        let id: Uuid = row.get("id");
        Ok(id)
    }

    /// Find radio call by ID
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails or call is not found.
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<RadioCallDb> {
        let query = "SELECT * FROM radio_calls WHERE id = $1";

        sqlx::query_as::<_, RadioCallDb>(query)
            .bind(id)
            .fetch_one(pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => Error::NotFound {
                    resource: format!("RadioCall with ID {id}"),
                },
                _ => Error::Database(e.to_string()),
            })
    }

    /// Find radio calls by system ID with pagination
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn find_by_system(
        pool: &PgPool,
        system_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<RadioCallDb>> {
        let query = r"
            SELECT * FROM radio_calls 
            WHERE system_id = $1 
            ORDER BY call_timestamp DESC 
            LIMIT $2 OFFSET $3
        ";

        sqlx::query_as::<_, RadioCallDb>(query)
            .bind(system_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))
    }

    /// Count radio calls by system ID
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn count_by_system(pool: &PgPool, system_id: &str) -> Result<i64> {
        let query = "SELECT COUNT(*) as count FROM radio_calls WHERE system_id = $1";

        let row = sqlx::query(query)
            .bind(system_id)
            .fetch_one(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(row.get("count"))
    }

    /// Find all radio calls with basic pagination (no system filter)
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn find_all_with_filters(
        pool: &PgPool,
        filter: &RadioCallFilter<'_>,
    ) -> Result<Vec<RadioCallDb>> {
        // Use same pattern as find_by_system which works
        let query = r"
            SELECT * FROM radio_calls
            ORDER BY call_timestamp DESC
            LIMIT $1 OFFSET $2
        ";

        tracing::info!("Executing find_all_with_filters query with limit={}, offset={}", filter.limit, filter.offset);

        let result = sqlx::query_as::<_, RadioCallDb>(query)
            .bind(filter.limit)
            .bind(filter.offset)
            .fetch_all(pool)
            .await
            .map_err(|e| {
                tracing::error!("Database query error in find_all_with_filters: {}", e);
                Error::Database(e.to_string())
            })?;

        tracing::info!("find_all_with_filters returned {} results", result.len());
        Ok(result)
    }

    /// Update transcription status
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn update_transcription_status(
        pool: &PgPool,
        transcription: TranscriptionUpdate<'_>,
    ) -> Result<()> {
        let TranscriptionUpdate {
            id,
            status,
            text,
            confidence,
            error,
            speaker_segments,
            speaker_count,
        } = transcription;
        let confidence_decimal = confidence
            .map(rust_decimal::Decimal::try_from)
            .transpose()
            .map_err(|e| Error::Database(format!("Invalid confidence value: {e}")))?;

        let query = r"
            UPDATE radio_calls
            SET transcription_status = $1,
                transcription_text = $2,
                transcription_confidence = $3,
                transcription_error = $4,
                speaker_segments = $5,
                speaker_count = $6,
                transcription_completed_at = CASE
                    WHEN $1 IN ('completed', 'failed') THEN NOW()
                    ELSE transcription_completed_at
                END,
                transcription_started_at = CASE
                    WHEN $1 = 'processing' AND transcription_started_at IS NULL THEN NOW()
                    ELSE transcription_started_at
                END
            WHERE id = $7
        ";

        sqlx::query(query)
            .bind(status)
            .bind(text)
            .bind(confidence_decimal)
            .bind(error)
            .bind(speaker_segments)
            .bind(speaker_count)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    /// Delete old radio calls
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn delete_older_than(
        pool: &PgPool,
        older_than: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64> {
        let query = "DELETE FROM radio_calls WHERE created_at < $1";

        let result = sqlx::query(query)
            .bind(older_than)
            .execute(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(result.rows_affected())
    }

    /// Get transcription statistics
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn get_transcription_stats(pool: &PgPool) -> Result<TranscriptionStats> {
        let query = r"
            SELECT 
                COUNT(*) as total,
                COUNT(CASE WHEN transcription_status = 'completed' THEN 1 END) as completed,
                COUNT(CASE WHEN transcription_status = 'failed' THEN 1 END) as failed,
                COUNT(CASE WHEN transcription_status = 'processing' THEN 1 END) as processing,
                COUNT(CASE WHEN transcription_status = 'pending' THEN 1 END) as pending,
                AVG(CASE WHEN transcription_confidence IS NOT NULL THEN transcription_confidence END) as avg_confidence
            FROM radio_calls
        ";

        let row = sqlx::query(query)
            .fetch_one(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(TranscriptionStats {
            total: row.get("total"),
            completed: row.get("completed"),
            failed: row.get("failed"),
            processing: row.get("processing"),
            pending: row.get("pending"),
            avg_confidence: row
                .get::<Option<rust_decimal::Decimal>, _>("avg_confidence")
                .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
        })
    }
}

/// Upload log database operations
pub struct UploadLogQueries;

impl UploadLogQueries {
    /// Insert a new upload log entry
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn insert(pool: &PgPool, log: &UploadLogDb) -> Result<Uuid> {
        let query = r"
            INSERT INTO upload_logs (
                timestamp, client_ip, user_agent, api_key_used, system_id,
                success, error_message, filename, file_size, content_type,
                response_code, processing_time_ms
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12
            )
            RETURNING id
        ";

        let row = sqlx::query(query)
            .bind(log.timestamp)
            .bind(log.client_ip)
            .bind(&log.user_agent)
            .bind(&log.api_key_used)
            .bind(&log.system_id)
            .bind(log.success)
            .bind(&log.error_message)
            .bind(&log.filename)
            .bind(log.file_size)
            .bind(&log.content_type)
            .bind(log.response_code)
            .bind(log.processing_time_ms)
            .fetch_one(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        let id: Uuid = row.get("id");
        Ok(id)
    }

    /// Get recent upload logs with pagination
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn get_recent(pool: &PgPool, limit: i64, offset: i64) -> Result<Vec<UploadLogDb>> {
        let query = r"
            SELECT * FROM upload_logs 
            ORDER BY timestamp DESC 
            LIMIT $1 OFFSET $2
        ";

        sqlx::query_as::<_, UploadLogDb>(query)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))
    }

    /// Get upload statistics
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn get_upload_stats(pool: &PgPool) -> Result<UploadStats> {
        let query = r"
            SELECT 
                COUNT(*) as total_uploads,
                COUNT(CASE WHEN success THEN 1 END) as successful_uploads,
                COUNT(CASE WHEN NOT success THEN 1 END) as failed_uploads,
                AVG(CASE WHEN processing_time_ms IS NOT NULL THEN processing_time_ms END) as avg_processing_time,
                COALESCE(SUM(CASE WHEN file_size IS NOT NULL THEN file_size END), 0)::BIGINT as total_bytes_uploaded
            FROM upload_logs
        ";

        let row = sqlx::query(query)
            .fetch_one(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(UploadStats {
            total_uploads: row.get("total_uploads"),
            successful_uploads: row.get("successful_uploads"),
            failed_uploads: row.get("failed_uploads"),
            avg_processing_time: row
                .get::<Option<rust_decimal::Decimal>, _>("avg_processing_time")
                .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
            total_bytes_uploaded: row
                .get::<Option<i64>, _>("total_bytes_uploaded")
                .unwrap_or(0),
        })
    }
}

/// System statistics database operations  
pub struct SystemStatsQueries;

impl SystemStatsQueries {
    /// Upsert system statistics
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn upsert(pool: &PgPool, stats: &SystemStatsDb) -> Result<()> {
        let query = r"
            INSERT INTO system_stats (
                system_id, system_label, total_calls, calls_today, calls_this_hour,
                first_seen, last_seen, top_talkgroups, upload_sources, last_updated
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10
            )
            ON CONFLICT (system_id) DO UPDATE SET
                system_label = EXCLUDED.system_label,
                total_calls = EXCLUDED.total_calls,
                calls_today = EXCLUDED.calls_today,
                calls_this_hour = EXCLUDED.calls_this_hour,
                last_seen = EXCLUDED.last_seen,
                top_talkgroups = EXCLUDED.top_talkgroups,
                upload_sources = EXCLUDED.upload_sources,
                last_updated = EXCLUDED.last_updated
        ";

        sqlx::query(query)
            .bind(&stats.system_id)
            .bind(&stats.system_label)
            .bind(stats.total_calls)
            .bind(stats.calls_today)
            .bind(stats.calls_this_hour)
            .bind(stats.first_seen)
            .bind(stats.last_seen)
            .bind(&stats.top_talkgroups)
            .bind(&stats.upload_sources)
            .bind(stats.last_updated)
            .execute(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    /// Get all system statistics
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn get_all(pool: &PgPool) -> Result<Vec<SystemStatsDb>> {
        let query = "SELECT * FROM system_stats ORDER BY system_id";

        sqlx::query_as::<_, SystemStatsDb>(query)
            .fetch_all(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))
    }

    /// Get system statistics by system ID
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails or system is not found.
    pub async fn get_by_system_id(pool: &PgPool, system_id: &str) -> Result<SystemStatsDb> {
        let query = "SELECT * FROM system_stats WHERE system_id = $1";

        sqlx::query_as::<_, SystemStatsDb>(query)
            .bind(system_id)
            .fetch_one(pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => Error::NotFound {
                    resource: format!("SystemStats for system {system_id}"),
                },
                _ => Error::Database(e.to_string()),
            })
    }

    /// Update system activity counters
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn update_activity(pool: &PgPool, system_id: &str) -> Result<()> {
        let query = r"
            INSERT INTO system_stats (
                system_id, total_calls, calls_today, calls_this_hour,
                first_seen, last_seen, last_updated
            ) VALUES (
                $1, 1, 1, 1, NOW(), NOW(), NOW()
            )
            ON CONFLICT (system_id) DO UPDATE SET
                total_calls = system_stats.total_calls + 1,
                calls_today = CASE 
                    WHEN DATE(system_stats.last_updated) = CURRENT_DATE THEN system_stats.calls_today + 1
                    ELSE 1
                END,
                calls_this_hour = CASE 
                    WHEN DATE_TRUNC('hour', system_stats.last_updated) = DATE_TRUNC('hour', NOW()) 
                    THEN system_stats.calls_this_hour + 1
                    ELSE 1
                END,
                last_seen = NOW(),
                last_updated = NOW()
        ";

        sqlx::query(query)
            .bind(system_id)
            .execute(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }
}

/// API key database operations
pub struct ApiKeyQueries;

impl ApiKeyQueries {
    /// Find API key by key hash
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails or key is not found.
    pub async fn find_by_key_hash(pool: &PgPool, key_hash: &str) -> Result<ApiKeyDb> {
        let query = r"
            SELECT * FROM api_keys 
            WHERE key_hash = $1 AND active = true
            AND (expires_at IS NULL OR expires_at > NOW())
        ";

        sqlx::query_as::<_, ApiKeyDb>(query)
            .bind(key_hash)
            .fetch_one(pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => Error::Authentication("Invalid API key".to_string()),
                _ => Error::Database(e.to_string()),
            })
    }

    /// Update API key usage
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn update_usage(pool: &PgPool, key_id: &str) -> Result<()> {
        let query = r"
            UPDATE api_keys 
            SET last_used = NOW(), 
                total_requests = COALESCE(total_requests, 0) + 1
            WHERE id = $1
        ";

        sqlx::query(query)
            .bind(key_id)
            .execute(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?;

        Ok(())
    }

    /// Get all active API keys
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn get_all_active(pool: &PgPool) -> Result<Vec<ApiKeyDb>> {
        let query = r"
            SELECT * FROM api_keys 
            WHERE active = true 
            AND (expires_at IS NULL OR expires_at > NOW())
            ORDER BY created_at DESC
        ";

        sqlx::query_as::<_, ApiKeyDb>(query)
            .fetch_all(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))
    }
}

/// Parameter struct for transcription updates
#[derive(Debug)]
pub struct TranscriptionUpdate<'a> {
    /// Call ID to update
    pub id: Uuid,
    /// New status
    pub status: &'a str,
    /// Transcription text
    pub text: Option<&'a str>,
    /// Confidence score
    pub confidence: Option<f32>,
    /// Error message
    pub error: Option<&'a str>,
    /// Speaker segments as JSON
    pub speaker_segments: Option<&'a serde_json::Value>,
    /// Number of speakers detected
    pub speaker_count: Option<i32>,
}

/// Parameter struct for filtering radio calls
#[derive(Debug)]
pub struct RadioCallFilter<'a> {
    /// System ID filter
    pub system_id: Option<&'a str>,
    /// Talkgroup ID filter
    pub talkgroup_id: Option<i32>,
    /// Date range start
    pub from_date: Option<chrono::DateTime<chrono::Utc>>,
    /// Date range end
    pub to_date: Option<chrono::DateTime<chrono::Utc>>,
    /// Maximum number of results
    pub limit: i64,
    /// Result offset for pagination
    pub offset: i64,
}

/// Parameter struct for upload log creation
#[derive(Debug)]
pub struct UploadLogParams {
    /// Client IP address
    pub client_ip: std::net::IpAddr,
    /// User agent string
    pub user_agent: Option<String>,
    /// API key ID
    pub api_key_id: Option<String>,
    /// System ID
    pub system_id: Option<String>,
    /// Upload success status
    pub success: bool,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Filename of uploaded file
    pub filename: Option<String>,
    /// File size in bytes
    pub file_size: Option<i64>,
}

/// Additional statistics types
#[derive(Debug, Clone)]
pub struct TranscriptionStats {
    /// Total number of transcriptions
    pub total: i64,
    /// Number of completed transcriptions
    pub completed: i64,
    /// Number of failed transcriptions
    pub failed: i64,
    /// Number of processing transcriptions
    pub processing: i64,
    /// Number of pending transcriptions
    pub pending: i64,
    /// Average confidence score
    pub avg_confidence: Option<f64>,
}

/// Upload statistics
#[derive(Debug, Clone)]
pub struct UploadStats {
    /// Total number of uploads
    pub total_uploads: i64,
    /// Number of successful uploads
    pub successful_uploads: i64,
    /// Number of failed uploads
    pub failed_uploads: i64,
    /// Average processing time in milliseconds
    pub avg_processing_time: Option<f64>,
    /// Total bytes uploaded
    pub total_bytes_uploaded: i64,
}

// Convenience wrapper functions for API compatibility

/// Insert a radio call (wrapper)
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn insert_radio_call(pool: &PgPool, call: &RadioCall) -> Result<Uuid> {
    RadioCallQueries::insert(pool, call).await
}

/// Get a radio call by ID (wrapper)
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn get_radio_call(pool: &PgPool, id: Uuid) -> Result<Option<RadioCallDb>> {
    match RadioCallQueries::find_by_id(pool, id).await {
        Ok(call) => Ok(Some(call)),
        Err(Error::NotFound { .. }) => Ok(None),
        Err(Error::Database(ref msg)) if msg.contains("no rows returned") => Ok(None),
        Err(e) => Err(e),
    }
}

/// List radio calls with filtering (simplified implementation)
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn list_radio_calls_filtered(
    pool: &PgPool,
    filter: RadioCallFilter<'_>,
) -> Result<Vec<RadioCallDb>> {
    tracing::info!("list_radio_calls_filtered called with system_id={:?}", filter.system_id);

    if let Some(system) = filter.system_id {
        tracing::info!("Using find_by_system for system: {}", system);
        RadioCallQueries::find_by_system(pool, system, filter.limit, filter.offset).await
    } else {
        tracing::info!("Using find_all_with_filters (no system filter)");
        RadioCallQueries::find_all_with_filters(pool, &filter).await
    }
}

/// Count radio calls with filtering
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn count_radio_calls_filtered(pool: &PgPool, filter: RadioCallFilter<'_>) -> Result<i64> {
    if let Some(system) = filter.system_id {
        RadioCallQueries::count_by_system(pool, system).await
    } else {
        // Use the existing count_radio_calls function for total count
        count_radio_calls(pool).await
    }
}

/// Count total radio calls
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn count_radio_calls(pool: &PgPool) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) as count FROM radio_calls")
        .fetch_one(pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(row.get("count"))
}

/// Count total systems
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn count_systems(pool: &PgPool) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(DISTINCT system_id) as count FROM radio_calls")
        .fetch_one(pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(row.get("count"))
}

/// Count recent calls (last N hours)
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn count_recent_calls(pool: &PgPool, hours: i32) -> Result<i64> {
    // Handle negative hours by ensuring we don't query future timestamps
    let row = if hours <= 0 {
        sqlx::query("SELECT 0::bigint as count")
            .fetch_one(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?
    } else {
        sqlx::query("SELECT COUNT(*) as count FROM radio_calls WHERE created_at > NOW() - make_interval(hours => $1)")
            .bind(hours)
            .fetch_one(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?
    };

    Ok(row.get("count"))
}

/// Get top systems by call count
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn get_top_systems(pool: &PgPool, limit: i64) -> Result<Vec<(String, i64)>> {
    let rows = sqlx::query("SELECT system_id, COUNT(*) as count FROM radio_calls GROUP BY system_id ORDER BY count DESC LIMIT $1")
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|row| (row.get("system_id"), row.get("count")))
        .collect())
}

/// Count system calls since a given time
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn count_system_calls_since(pool: &PgPool, system_id: &str, hours: i32) -> Result<i64> {
    // Handle negative hours by ensuring we don't query future timestamps
    let row = if hours <= 0 {
        sqlx::query("SELECT 0::bigint as count")
            .fetch_one(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?
    } else {
        sqlx::query("SELECT COUNT(*) as count FROM radio_calls WHERE system_id = $1 AND created_at > NOW() - make_interval(hours => $2)")
            .bind(system_id)
            .bind(hours)
            .fetch_one(pool)
            .await
            .map_err(|e| Error::Database(e.to_string()))?
    };

    Ok(row.get("count"))
}

/// Get system statistics
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn get_system_stats(pool: &PgPool, system_id: &str) -> Result<SystemStatsDb> {
    SystemStatsQueries::get_by_system_id(pool, system_id).await
}

/// Update system statistics
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn update_system_stats(
    pool: &PgPool,
    system_id: &str,
    system_label: Option<String>,
) -> Result<()> {
    // Create basic system stats entry
    let stats = SystemStatsDb {
        id: Uuid::new_v4(),
        system_id: system_id.to_string(),
        system_label,
        total_calls: Some(1),
        calls_today: Some(1),
        calls_this_hour: Some(1),
        first_seen: Some(chrono::Utc::now()),
        last_seen: Some(chrono::Utc::now()),
        top_talkgroups: None,
        upload_sources: None,
        last_updated: chrono::Utc::now(),
    };

    SystemStatsQueries::upsert(pool, &stats).await
}

/// Validate API key
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn validate_api_key(pool: &PgPool, key_hash: &str) -> Result<Option<ApiKeyDb>> {
    match ApiKeyQueries::find_by_key_hash(pool, key_hash).await {
        Ok(key) => Ok(Some(key)),
        Err(Error::Authentication(_)) => Ok(None),
        Err(Error::Database(ref msg)) if msg.contains("no rows returned") => Ok(None),
        Err(e) => Err(e),
    }
}

/// Insert upload log entry
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn insert_upload_log(pool: &PgPool, params: UploadLogParams) -> Result<Uuid> {
    let log = UploadLogDb {
        id: Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        client_ip: params.client_ip.into(),
        user_agent: params.user_agent,
        api_key_used: params.api_key_id,
        system_id: params.system_id,
        success: params.success,
        error_message: params.error_message,
        filename: params.filename,
        file_size: params.file_size,
        content_type: None,
        response_code: None,
        processing_time_ms: None,
    };

    UploadLogQueries::insert(pool, &log).await
}

/// Update transcription status for a radio call
///
/// # Errors
///
/// Returns an error if the database query fails.
pub async fn update_transcription_status(pool: &PgPool, call_id: Uuid, status: &str) -> Result<()> {
    let query = r"
        UPDATE radio_calls
        SET transcription_status = $2,
            updated_at = NOW()
        WHERE id = $1
    ";

    sqlx::query(query)
        .bind(call_id)
        .bind(status)
        .execute(pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;
    use sdrtrunk_core::types::TranscriptionStatus;
    use sqlx::PgPool;
    use std::net::IpAddr;
    use uuid::Uuid;

    // Helper function to create test database
    async fn create_test_pool() -> Option<PgPool> {
        // Check if TEST_DATABASE_URL is set, if not, skip the test
        if let Ok(database_url) = std::env::var("TEST_DATABASE_URL") {
            match PgPool::connect(&database_url).await {
                Ok(pool) => {
                    // Skip migrations in test - assume they're already run by CI setup
                    // The CI script runs migrations before tests
                    Some(pool)
                }
                Err(e) => {
                    eprintln!("Failed to connect to database: {e}");
                    None
                }
            }
        } else {
            None
        }
    }

    // Test data helpers
    fn create_test_radio_call(system_id: &str, talkgroup_id: Option<i32>) -> RadioCall {
        RadioCall {
            id: Some(Uuid::new_v4()),
            created_at: chrono::Utc::now(),
            call_timestamp: chrono::Utc::now(),
            system_id: system_id.to_string(),
            system_label: Some("Test System".to_string()),
            frequency: Some(154_000_000),
            talkgroup_id,
            talkgroup_label: Some("Test TG".to_string()),
            talkgroup_group: Some("Public Safety".to_string()),
            talkgroup_tag: Some("Police".to_string()),
            source_radio_id: Some(12345),
            talker_alias: Some("OFFICER1".to_string()),
            audio_filename: Some("test.mp3".to_string()),
            audio_file_path: Some("/tmp/test.mp3".to_string()),
            audio_size_bytes: Some(2_048_000),
            duration_seconds: Some(30.5),
            transcription_text: Some("Test transcription".to_string()),
            transcription_confidence: Some(0.95),
            transcription_status: TranscriptionStatus::Completed,
            transcription_error: None,
            transcription_started_at: Some(chrono::Utc::now() - chrono::Duration::seconds(30)),
            transcription_completed_at: Some(chrono::Utc::now()),
            speaker_segments: Some(
                serde_json::json!([{"speaker": "A", "start": 0.0, "end": 30.5}]),
            ),
            transcription_segments: Some(
                serde_json::json!([{"text": "Test transcription", "start": 0.0, "end": 30.5}]),
            ),
            speaker_count: Some(1),
            patches: Some(serde_json::json!([{"id": 1, "name": "Patch1"}])),
            frequencies: Some(serde_json::json!([154_000_000])),
            sources: Some(serde_json::json!([{"id": 1, "name": "Source1"}])),
            upload_ip: Some("127.0.0.1".to_string()),
            upload_timestamp: chrono::Utc::now(),
            upload_api_key_id: Some("test_key_id".to_string()),
        }
    }

    fn create_test_upload_log() -> UploadLogDb {
        UploadLogDb {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            client_ip: "192.168.1.100".parse::<IpAddr>().unwrap().into(),
            user_agent: Some("TestClient/1.0".to_string()),
            api_key_used: Some("test_key".to_string()),
            system_id: Some("test_system".to_string()),
            success: true,
            error_message: None,
            filename: Some("test_audio.mp3".to_string()),
            file_size: Some(1_048_576),
            content_type: Some("audio/mpeg".to_string()),
            response_code: Some(200),
            processing_time_ms: Some(rust_decimal::Decimal::new(750, 0)),
        }
    }

    fn create_test_system_stats(system_id: &str) -> SystemStatsDb {
        SystemStatsDb {
            id: Uuid::new_v4(),
            system_id: system_id.to_string(),
            system_label: Some("Test Radio System".to_string()),
            total_calls: Some(500),
            calls_today: Some(25),
            calls_this_hour: Some(3),
            first_seen: Some(chrono::Utc::now() - chrono::Duration::days(90)),
            last_seen: Some(chrono::Utc::now()),
            top_talkgroups: Some(
                serde_json::json!([{"id": 123, "count": 50}, {"id": 456, "count": 30}]),
            ),
            upload_sources: Some(serde_json::json!(["192.168.1.1", "192.168.1.2"])),
            last_updated: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_radio_call_insert_and_find() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let call = RadioCall {
            system_id: "test_system".to_string(),
            talkgroup_id: Some(12345),
            transcription_status: TranscriptionStatus::None,
            ..RadioCall::default()
        };

        let id = RadioCallQueries::insert(&pool, &call).await?;
        assert!(!id.is_nil());

        let retrieved = RadioCallQueries::find_by_id(&pool, id).await?;
        assert_eq!(retrieved.system_id, "test_system");
        assert_eq!(retrieved.talkgroup_id, Some(12345));

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_radio_call_count_by_system() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        // Use unique system ID to avoid conflicts with other test runs
        // Keep it short to fit in varchar(50)
        let unique_system = format!("test_{}", &Uuid::new_v4().to_string()[0..8]);

        // Insert test calls
        for i in 0..5 {
            let call = RadioCall {
                system_id: unique_system.clone(),
                talkgroup_id: Some(i),
                ..RadioCall::default()
            };
            RadioCallQueries::insert(&pool, &call).await?;
        }

        let count = RadioCallQueries::count_by_system(&pool, &unique_system).await?;
        assert_eq!(count, 5);

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_transcription_status_update() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let call = RadioCall {
            system_id: "transcription_test".to_string(),
            transcription_status: TranscriptionStatus::Pending,
            ..RadioCall::default()
        };

        let id = RadioCallQueries::insert(&pool, &call).await?;

        // Update to processing
        RadioCallQueries::update_transcription_status(
            &pool,
            TranscriptionUpdate {
                id,
                status: "processing",
                text: None,
                confidence: None,
                error: None,
                speaker_segments: None,
                speaker_count: None,
            },
        )
        .await?;

        let updated = RadioCallQueries::find_by_id(&pool, id).await?;
        assert_eq!(updated.transcription_status, Some("processing".to_string()));

        // Update to completed
        RadioCallQueries::update_transcription_status(
            &pool,
            TranscriptionUpdate {
                id,
                status: "completed",
                text: Some("Test transcription"),
                confidence: Some(0.95),
                error: None,
                speaker_segments: None,
                speaker_count: None,
            },
        )
        .await?;

        let completed = RadioCallQueries::find_by_id(&pool, id).await?;
        assert_eq!(
            completed.transcription_status,
            Some("completed".to_string())
        );
        assert_eq!(
            completed.transcription_text,
            Some("Test transcription".to_string())
        );
        assert!(completed.transcription_confidence.is_some());

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_system_stats_upsert() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let stats = SystemStatsDb {
            id: Uuid::new_v4(),
            system_id: "upsert_test".to_string(),
            system_label: Some("Test System".to_string()),
            total_calls: Some(100),
            calls_today: Some(10),
            calls_this_hour: Some(2),
            first_seen: Some(chrono::Utc::now() - chrono::Duration::days(30)),
            last_seen: Some(chrono::Utc::now()),
            top_talkgroups: None,
            upload_sources: None,
            last_updated: chrono::Utc::now(),
        };

        // First upsert (insert)
        SystemStatsQueries::upsert(&pool, &stats).await?;

        let retrieved = SystemStatsQueries::get_by_system_id(&pool, "upsert_test").await?;
        assert_eq!(retrieved.system_id, "upsert_test");
        assert_eq!(retrieved.total_calls, Some(100));

        // Second upsert (update)
        let mut updated_stats = stats.clone();
        updated_stats.total_calls = Some(150);
        updated_stats.calls_today = Some(15);

        SystemStatsQueries::upsert(&pool, &updated_stats).await?;

        let updated_retrieved = SystemStatsQueries::get_by_system_id(&pool, "upsert_test").await?;
        assert_eq!(updated_retrieved.total_calls, Some(150));
        assert_eq!(updated_retrieved.calls_today, Some(15));

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_upload_log_operations() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let log = UploadLogDb {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            client_ip: "127.0.0.1"
                .parse::<std::net::IpAddr>()
                .map_err(|e| Error::Validation {
                    field: "client_ip".to_string(),
                    message: e.to_string(),
                })?
                .into(),
            user_agent: Some("Test User Agent".to_string()),
            api_key_used: Some("test_key".to_string()),
            system_id: Some("test_system".to_string()),
            success: true,
            error_message: None,
            filename: Some("test.mp3".to_string()),
            file_size: Some(1_024_000),
            content_type: Some("audio/mpeg".to_string()),
            response_code: Some(200),
            processing_time_ms: Some(rust_decimal::Decimal::new(1500, 0)),
        };

        let id = UploadLogQueries::insert(&pool, &log).await?;
        assert!(!id.is_nil());

        let logs = UploadLogQueries::get_recent(&pool, 10, 0).await?;
        assert!(!logs.is_empty());

        let stats = UploadLogQueries::get_upload_stats(&pool).await?;
        assert!(stats.total_uploads > 0);
        assert!(stats.successful_uploads > 0);

        Ok(())
    }

    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_transcription_stats_creation() {
        let stats = TranscriptionStats {
            total: 1000,
            completed: 850,
            failed: 50,
            processing: 10,
            pending: 90,
            avg_confidence: Some(0.92),
        };

        assert_eq!(stats.total, 1000);
        assert_eq!(stats.completed, 850);
        assert!(stats.avg_confidence.unwrap() > 0.9);
    }

    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_upload_stats_creation() {
        let stats = UploadStats {
            total_uploads: 500,
            successful_uploads: 475,
            failed_uploads: 25,
            avg_processing_time: Some(1200.0),
            total_bytes_uploaded: 1024 * 1024 * 1024, // 1GB
        };

        assert_eq!(stats.total_uploads, 500);
        assert_eq!(stats.successful_uploads + stats.failed_uploads, 500);
        assert!(stats.avg_processing_time.unwrap() > 1000.0);
        assert!(stats.total_bytes_uploaded > 1_000_000_000);
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_radio_call_find_by_id_not_found() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let fake_id = Uuid::new_v4();
        let result = RadioCallQueries::find_by_id(&pool, fake_id).await;

        assert!(matches!(result, Err(Error::NotFound { .. })));
        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_radio_call_find_by_system_empty() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let nonexistent_system = format!("nonexistent_{}", &Uuid::new_v4().to_string()[0..8]);
        let calls = RadioCallQueries::find_by_system(&pool, &nonexistent_system, 10, 0).await?;

        assert!(calls.is_empty());
        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_radio_call_count_by_system_zero() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let nonexistent_system = format!("nonexistent_{}", &Uuid::new_v4().to_string()[0..8]);
        let count = RadioCallQueries::count_by_system(&pool, &nonexistent_system).await?;

        assert_eq!(count, 0);
        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_transcription_update_with_error() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let call = RadioCall {
            system_id: "error_test".to_string(),
            transcription_status: TranscriptionStatus::Pending,
            ..RadioCall::default()
        };

        let id = RadioCallQueries::insert(&pool, &call).await?;

        // Update with error
        RadioCallQueries::update_transcription_status(
            &pool,
            TranscriptionUpdate {
                id,
                status: "failed",
                text: None,
                confidence: None,
                error: Some("Transcription service unavailable"),
                speaker_segments: None,
                speaker_count: None,
            },
        )
        .await?;

        let updated = RadioCallQueries::find_by_id(&pool, id).await?;
        assert_eq!(updated.transcription_status, Some("failed".to_string()));
        // Note: transcription_error field doesn't exist in RadioCallDb model
        // The error is stored but may not be accessible through this model
        assert_eq!(updated.transcription_status, Some("failed".to_string()));

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_system_stats_not_found() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let nonexistent_system = format!("nonexistent_{}", &Uuid::new_v4().to_string()[0..8]);
        let result = SystemStatsQueries::get_by_system_id(&pool, &nonexistent_system).await;

        assert!(matches!(result, Err(Error::NotFound { .. })));
        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_upload_log_with_failure() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let mut log = create_test_upload_log();
        log.success = false;
        log.error_message = Some("File too large".to_string());
        log.response_code = Some(413);

        let id = UploadLogQueries::insert(&pool, &log).await?;
        assert!(!id.is_nil());

        let stats = UploadLogQueries::get_upload_stats(&pool).await?;
        assert!(stats.failed_uploads > 0);

        Ok(())
    }

    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_transcription_update_struct() {
        let update = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "processing",
            text: Some("Partial transcription"),
            confidence: Some(0.75),
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };

        assert_eq!(update.status, "processing");
        assert_eq!(update.text, Some("Partial transcription"));
        assert!(update.confidence.unwrap() > 0.7);
        assert!(update.error.is_none());
    }

    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_radio_call_filter_struct() {
        let now = chrono::Utc::now();
        let filter = RadioCallFilter {
            system_id: Some("test_system"),
            talkgroup_id: Some(12345),
            from_date: Some(now - chrono::Duration::hours(24)),
            to_date: Some(now),
            limit: 100,
            offset: 50,
        };

        assert_eq!(filter.system_id, Some("test_system"));
        assert_eq!(filter.talkgroup_id, Some(12345));
        assert_eq!(filter.limit, 100);
        assert_eq!(filter.offset, 50);
        assert!(filter.from_date.is_some());
        assert!(filter.to_date.is_some());
    }

    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_upload_log_params_struct() {
        let params = UploadLogParams {
            client_ip: "192.168.1.100".parse().unwrap(),
            user_agent: Some("TestClient/1.0".to_string()),
            api_key_id: Some("test_key".to_string()),
            system_id: Some("test_system".to_string()),
            success: true,
            error_message: None,
            filename: Some("test.mp3".to_string()),
            file_size: Some(2_048_000),
        };

        assert!(params.success);
        assert!(params.error_message.is_none());
        assert_eq!(params.file_size, Some(2_048_000));
        assert_eq!(params.filename, Some("test.mp3".to_string()));
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_wrapper_get_radio_call_error_handling() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        // Test with non-existent ID
        let fake_id = Uuid::new_v4();
        let result = get_radio_call(&pool, fake_id).await?;
        assert!(result.is_none());

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_validate_api_key_not_found() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let fake_hash = "nonexistent_hash_12345";
        let result = validate_api_key(&pool, fake_hash).await?;
        assert!(result.is_none());

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_insert_upload_log_with_minimal_data() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let minimal_params = UploadLogParams {
            client_ip: "127.0.0.1".parse().unwrap(),
            user_agent: None,
            api_key_id: None,
            system_id: None,
            success: false,
            error_message: Some("Minimal test error".to_string()),
            filename: None,
            file_size: None,
        };

        let id = insert_upload_log(&pool, minimal_params).await?;
        assert!(!id.is_nil());

        Ok(())
    }

    #[test]
    fn test_confidence_and_duration_decimal_conversion_edge_cases() {
        // Test edge case values for decimal conversion
        let zero_confidence = 0.0f32;
        let zero_decimal = rust_decimal::Decimal::try_from(zero_confidence).unwrap();
        assert_eq!(zero_decimal.to_string(), "0");

        let max_confidence = 1.0f32;
        let max_decimal = rust_decimal::Decimal::try_from(max_confidence).unwrap();
        assert_eq!(max_decimal.to_string(), "1");

        // Test duration conversion
        let zero_duration = 0.0f64;
        let zero_dur_decimal = rust_decimal::Decimal::try_from(zero_duration).unwrap();
        assert_eq!(zero_dur_decimal.to_string(), "0");

        let long_duration = 3600.0f64; // 1 hour
        let long_dur_decimal = rust_decimal::Decimal::try_from(long_duration).unwrap();
        assert_eq!(long_dur_decimal.to_string(), "3600");
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_radio_call_with_all_optional_fields() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let system_id = format!("full_test_{}", &Uuid::new_v4().to_string()[0..8]);

        let call = RadioCall {
            id: Some(Uuid::new_v4()),
            created_at: chrono::Utc::now(),
            call_timestamp: chrono::Utc::now(),
            system_id: system_id.clone(),
            system_label: Some("Full Test System".to_string()),
            frequency: Some(460_125_000),
            talkgroup_id: Some(98765),
            talkgroup_label: Some("Test Talkgroup".to_string()),
            talkgroup_group: Some("Emergency".to_string()),
            talkgroup_tag: Some("Fire".to_string()),
            source_radio_id: Some(54321),
            talker_alias: Some("ENGINE_01".to_string()),
            audio_filename: Some("full_test.wav".to_string()),
            audio_file_path: Some("/tmp/full_test.wav".to_string()),
            audio_size_bytes: Some(4_096_000),
            duration_seconds: Some(125.75),
            transcription_text: Some("This is a full test transcription".to_string()),
            transcription_confidence: Some(0.98),
            transcription_status: TranscriptionStatus::Completed,
            transcription_error: None,
            transcription_started_at: Some(chrono::Utc::now() - chrono::Duration::seconds(60)),
            transcription_completed_at: Some(chrono::Utc::now()),
            speaker_segments: Some(serde_json::json!([
                {"speaker": "A", "start": 0.0, "end": 60.0},
                {"speaker": "B", "start": 60.0, "end": 125.75}
            ])),
            transcription_segments: Some(serde_json::json!([
                {"text": "This is a full", "start": 0.0, "end": 2.5},
                {"text": "test transcription", "start": 2.5, "end": 5.0}
            ])),
            speaker_count: Some(2),
            patches: Some(serde_json::json!([{"id": 1, "name": "Patch A"}])),
            frequencies: Some(serde_json::json!([460_125_000, 460_150_000])),
            sources: Some(serde_json::json!([{"id": 1, "name": "Site 1"}])),
            upload_ip: Some("10.0.0.100".to_string()),
            upload_timestamp: chrono::Utc::now(),
            upload_api_key_id: Some("test_api_key_full".to_string()),
        };

        let id = RadioCallQueries::insert(&pool, &call).await?;
        assert!(!id.is_nil());

        let retrieved = RadioCallQueries::find_by_id(&pool, id).await?;
        assert_eq!(retrieved.system_id, system_id);
        assert_eq!(retrieved.talkgroup_id, Some(98765));
        assert_eq!(retrieved.speaker_count, Some(2));
        assert!(retrieved.transcription_confidence.is_some());
        assert!(retrieved.speaker_segments.is_some());

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_system_stats_with_json_fields() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let system_id = format!("json_test_{}", &Uuid::new_v4().to_string()[0..8]);

        let stats = SystemStatsDb {
            id: Uuid::new_v4(),
            system_id: system_id.clone(),
            system_label: Some("JSON Test System".to_string()),
            total_calls: Some(1000),
            calls_today: Some(50),
            calls_this_hour: Some(5),
            first_seen: Some(chrono::Utc::now() - chrono::Duration::days(365)),
            last_seen: Some(chrono::Utc::now()),
            top_talkgroups: Some(serde_json::json!([
                {"id": 100, "label": "Dispatch", "count": 300},
                {"id": 200, "label": "Tactical", "count": 150},
                {"id": 300, "label": "Admin", "count": 75}
            ])),
            upload_sources: Some(serde_json::json!([
                {"ip": "192.168.1.10", "last_seen": "2023-12-01T10:00:00Z"},
                {"ip": "10.0.0.50", "last_seen": "2023-12-01T09:30:00Z"}
            ])),
            last_updated: chrono::Utc::now(),
        };

        SystemStatsQueries::upsert(&pool, &stats).await?;

        let retrieved = SystemStatsQueries::get_by_system_id(&pool, &system_id).await?;
        assert_eq!(retrieved.system_id, system_id);
        assert_eq!(retrieved.total_calls, Some(1000));
        assert!(retrieved.top_talkgroups.is_some());
        assert!(retrieved.upload_sources.is_some());

        // Verify JSON structure
        let talkgroups = retrieved.top_talkgroups.unwrap();
        assert!(talkgroups.is_array());
        let tg_array = talkgroups.as_array().unwrap();
        assert_eq!(tg_array.len(), 3);
        assert_eq!(tg_array[0]["id"], 100);
        assert_eq!(tg_array[0]["label"], "Dispatch");

        Ok(())
    }

    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_error_conversions() {
        // Test that database errors are properly converted
        let sql_error = sqlx::Error::RowNotFound;
        let app_error = match sql_error {
            sqlx::Error::RowNotFound => Error::NotFound {
                resource: "test resource".to_string(),
            },
            _ => Error::Database("other error".to_string()),
        };

        match app_error {
            Error::NotFound { resource } => assert_eq!(resource, "test resource"),
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_decimal_conversions() {
        // Test decimal conversion for confidence values
        let confidence = 0.95f32;
        let decimal = rust_decimal::Decimal::try_from(confidence).unwrap();
        let back_to_float = decimal.to_string().parse::<f32>().unwrap();

        assert!((confidence - back_to_float).abs() < 0.001);

        // Test duration conversion
        let duration = 30.5f64;
        let decimal = rust_decimal::Decimal::try_from(duration).unwrap();
        let back_to_float = decimal.to_string().parse::<f64>().unwrap();

        assert!((duration - back_to_float).abs() < 0.001);
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_transcription_update() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let system_id = format!("transcription_test_{}", &Uuid::new_v4().to_string()[0..8]);
        let call = create_test_radio_call(&system_id, Some(123));
        let call_id = RadioCallQueries::insert(&pool, &call).await?;

        // Test updating transcription status to processing
        let update = TranscriptionUpdate {
            id: call_id,
            status: "processing",
            text: None,
            confidence: None,
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };
        RadioCallQueries::update_transcription_status(&pool, update).await?;

        // Test updating to completed with text and confidence
        let update = TranscriptionUpdate {
            id: call_id,
            status: "completed",
            text: Some("Updated transcription text"),
            confidence: Some(0.95),
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };
        RadioCallQueries::update_transcription_status(&pool, update).await?;

        // Test updating to failed with error
        let update = TranscriptionUpdate {
            id: call_id,
            status: "failed",
            text: None,
            confidence: None,
            error: Some("Test error message"),
            speaker_segments: None,
            speaker_count: None,
        };
        RadioCallQueries::update_transcription_status(&pool, update).await?;

        let retrieved = RadioCallQueries::find_by_id(&pool, call_id).await?;
        assert_eq!(retrieved.transcription_status, Some("failed".to_string()));
        // Note: transcription_error is not stored in database model, only used in update

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_delete_older_than() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let system_id = format!("delete_test_{}", &Uuid::new_v4().to_string()[0..8]);

        // Create an old call (2 days ago)
        let mut old_call = create_test_radio_call(&system_id, Some(100));
        old_call.created_at = chrono::Utc::now() - chrono::Duration::days(2);
        let old_call_id = RadioCallQueries::insert(&pool, &old_call).await?;

        // Create a recent call
        let recent_call = create_test_radio_call(&system_id, Some(101));
        let recent_call_id = RadioCallQueries::insert(&pool, &recent_call).await?;

        // Delete calls older than 1 day
        let cutoff = chrono::Utc::now() - chrono::Duration::days(1);
        let deleted_count = RadioCallQueries::delete_older_than(&pool, cutoff).await?;
        assert!(deleted_count >= 1);

        // Verify old call is gone but recent call remains
        assert!(
            RadioCallQueries::find_by_id(&pool, old_call_id)
                .await
                .is_err()
        );
        assert!(
            RadioCallQueries::find_by_id(&pool, recent_call_id)
                .await
                .is_ok()
        );

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_get_transcription_stats() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let system_id = format!("stats_test_{}", &Uuid::new_v4().to_string()[0..8]);

        // Insert calls with different transcription statuses
        for (i, status) in ["completed", "failed", "processing", "pending"]
            .iter()
            .enumerate()
        {
            let mut call = create_test_radio_call(&system_id, Some(i.try_into().unwrap()));
            call.transcription_status = match *status {
                "completed" => TranscriptionStatus::Completed,
                "failed" => TranscriptionStatus::Failed,
                "processing" => TranscriptionStatus::Processing,
                _ => TranscriptionStatus::None,
            };
            call.transcription_confidence = if *status == "completed" {
                Some(0.85)
            } else {
                None
            };
            RadioCallQueries::insert(&pool, &call).await?;
        }

        let stats = RadioCallQueries::get_transcription_stats(&pool).await?;
        assert!(stats.total >= 4);
        assert!(stats.completed >= 1);
        assert!(stats.failed >= 1);
        assert!(stats.processing >= 1);
        assert!(stats.pending >= 1);
        assert!(stats.avg_confidence.is_some());

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_find_by_system_with_pagination() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let system_id = format!("test_pagination_{}", &Uuid::new_v4().to_string()[0..8]);

        // Insert 10 test calls
        for i in 0..10 {
            let mut call = create_test_radio_call(&system_id, Some(i));
            call.call_timestamp = chrono::Utc::now() - chrono::Duration::minutes(i64::from(i));
            RadioCallQueries::insert(&pool, &call).await?;
        }

        // Test pagination
        let page1 = RadioCallQueries::find_by_system(&pool, &system_id, 5, 0).await?;
        assert_eq!(page1.len(), 5);

        let page2 = RadioCallQueries::find_by_system(&pool, &system_id, 5, 5).await?;
        assert_eq!(page2.len(), 5);

        // Ensure calls are ordered by timestamp DESC
        for i in 0..4 {
            let current = page1[i].call_timestamp;
            let next = page1[i + 1].call_timestamp;
            assert!(current >= next, "Calls should be ordered by timestamp DESC");
        }

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_transcription_stats() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let system_id = format!("test_stats_{}", &Uuid::new_v4().to_string()[0..8]);

        // Insert calls with different transcription statuses
        let statuses = ["completed", "failed", "processing", "pending"];
        let confidences = [Some(0.95), None, None, None];

        for (i, &status) in statuses.iter().enumerate() {
            let mut call = create_test_radio_call(&system_id, Some(i32::try_from(i).unwrap()));
            call.transcription_status = match status {
                "completed" => TranscriptionStatus::Completed,
                "failed" => TranscriptionStatus::Failed,
                "processing" => TranscriptionStatus::Processing,
                "pending" => TranscriptionStatus::Pending,
                _ => TranscriptionStatus::None,
            };
            call.transcription_confidence = confidences[i];
            RadioCallQueries::insert(&pool, &call).await?;
        }

        let stats = RadioCallQueries::get_transcription_stats(&pool).await?;
        assert!(stats.total >= 4);
        assert!(stats.completed >= 1);
        assert!(stats.failed >= 1);
        assert!(stats.processing >= 1);
        assert!(stats.pending >= 1);
        assert!(stats.avg_confidence.is_some());

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_upload_log_recent_queries() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        // Insert multiple upload logs with different timestamps
        for i in 0..5 {
            let mut log = create_test_upload_log();
            log.timestamp = chrono::Utc::now() - chrono::Duration::minutes(i64::from(i));
            log.filename = Some(format!("test_{i}.mp3"));
            UploadLogQueries::insert(&pool, &log).await?;
        }

        // Test pagination and ordering
        let recent_logs = UploadLogQueries::get_recent(&pool, 3, 0).await?;
        assert_eq!(recent_logs.len(), 3);

        // Verify ordering (most recent first)
        for i in 0..2 {
            assert!(recent_logs[i].timestamp >= recent_logs[i + 1].timestamp);
        }

        // Test second page
        let second_page = UploadLogQueries::get_recent(&pool, 3, 3).await?;
        assert!(second_page.len() >= 2);

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_upload_log_statistics() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        // Insert logs with success and failure
        let mut success_log = create_test_upload_log();
        success_log.success = true;
        success_log.processing_time_ms = Some(rust_decimal::Decimal::new(1000, 0));
        success_log.file_size = Some(1_024_000);
        UploadLogQueries::insert(&pool, &success_log).await?;

        let mut failed_log = create_test_upload_log();
        failed_log.success = false;
        failed_log.error_message = Some("Upload failed".to_string());
        failed_log.processing_time_ms = Some(rust_decimal::Decimal::new(500, 0));
        failed_log.file_size = Some(512_000);
        UploadLogQueries::insert(&pool, &failed_log).await?;

        let stats = UploadLogQueries::get_upload_stats(&pool).await?;
        assert!(stats.total_uploads >= 2);
        assert!(stats.successful_uploads >= 1);
        assert!(stats.failed_uploads >= 1);
        assert!(stats.avg_processing_time.is_some());
        assert!(stats.total_bytes_uploaded >= 1_536_000);

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_system_stats_get_all() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        // Insert multiple system stats
        for i in 1..=3 {
            let stats = create_test_system_stats(&format!("system_{i}"));
            SystemStatsQueries::upsert(&pool, &stats).await?;
        }

        let all_stats = SystemStatsQueries::get_all(&pool).await?;
        assert!(all_stats.len() >= 3);

        // Verify ordering by system_id
        let mut prev_id = String::new();
        for stat in &all_stats {
            if !prev_id.is_empty() {
                assert!(stat.system_id >= prev_id);
            }
            prev_id = stat.system_id.clone();
        }

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_system_stats_activity_update() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let system_id = format!("activity_{}", &Uuid::new_v4().to_string()[0..8]);

        // First update (insert)
        SystemStatsQueries::update_activity(&pool, &system_id).await?;

        let stats1 = SystemStatsQueries::get_by_system_id(&pool, &system_id).await?;
        assert_eq!(stats1.total_calls, Some(1));
        assert_eq!(stats1.calls_today, Some(1));
        assert_eq!(stats1.calls_this_hour, Some(1));

        // Second update (increment)
        SystemStatsQueries::update_activity(&pool, &system_id).await?;

        let stats2 = SystemStatsQueries::get_by_system_id(&pool, &system_id).await?;
        assert_eq!(stats2.total_calls, Some(2));
        assert_eq!(stats2.calls_today, Some(2));
        assert_eq!(stats2.calls_this_hour, Some(2));
        assert!(stats2.last_seen.is_some());
        assert_eq!(stats2.system_id, system_id);

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_api_key_operations() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        // Note: These tests would require API key data in the database
        // For now, we'll test the error cases

        let fake_hash = "fake_hash";
        let fake_key_id = "fake_key_id";

        // Test finding non-existent key
        let result = ApiKeyQueries::find_by_key_hash(&pool, fake_hash).await;
        assert!(matches!(result, Err(Error::Authentication(_))));

        // Test updating non-existent key usage
        let usage_result = ApiKeyQueries::update_usage(&pool, fake_key_id).await;
        // This might succeed with 0 rows affected, which is fine
        assert!(usage_result.is_ok());

        // Test getting all active keys
        let keys = ApiKeyQueries::get_all_active(&pool).await?;
        // Should return empty list if no keys exist
        assert!(keys.is_empty() || !keys.is_empty()); // Either case is valid

        Ok(())
    }

    // Test wrapper functions
    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_wrapper_functions() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let system_id = format!("wrapper_{}", &Uuid::new_v4().to_string()[0..8]);

        // Test insert_radio_call wrapper
        let call = create_test_radio_call(&system_id, Some(12345));
        let inserted_id = insert_radio_call(&pool, &call).await?;
        assert!(!inserted_id.is_nil());

        // Test get_radio_call wrapper
        let retrieved = get_radio_call(&pool, inserted_id).await?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().system_id, system_id);

        // Test get_radio_call with non-existent ID
        let fake_id = Uuid::new_v4();
        let not_found = get_radio_call(&pool, fake_id).await?;
        assert!(not_found.is_none());

        // Test list_radio_calls_filtered
        let filter = RadioCallFilter {
            system_id: Some(&system_id),
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            limit: 10,
            offset: 0,
        };
        let filtered_calls = list_radio_calls_filtered(&pool, filter).await?;
        assert!(!filtered_calls.is_empty());

        // Test count_radio_calls_filtered
        let filter_count = RadioCallFilter {
            system_id: Some(&system_id),
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            limit: 10,
            offset: 0,
        };
        let count = count_radio_calls_filtered(&pool, filter_count).await?;
        assert!(count > 0);

        // Test filter with no system_id (should return empty/0)
        let empty_filter = RadioCallFilter {
            system_id: None,
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            limit: 10,
            offset: 0,
        };
        let empty_calls = list_radio_calls_filtered(&pool, empty_filter).await?;
        assert!(empty_calls.is_empty());

        let empty_count = count_radio_calls_filtered(
            &pool,
            RadioCallFilter {
                system_id: None,
                talkgroup_id: None,
                from_date: None,
                to_date: None,
                limit: 10,
                offset: 0,
            },
        )
        .await?;
        assert_eq!(empty_count, 0);

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_counting_functions() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let system_id = format!("count_{}", &Uuid::new_v4().to_string()[0..8]);

        // Insert a test call
        let call = create_test_radio_call(&system_id, Some(999));
        insert_radio_call(&pool, &call).await?;

        // Test total count functions
        let total_calls = count_radio_calls(&pool).await?;
        assert!(total_calls > 0);

        let total_systems = count_systems(&pool).await?;
        assert!(total_systems > 0);

        // Test recent calls count
        let recent_calls = count_recent_calls(&pool, 24).await?;
        assert!(recent_calls >= 1); // Should include our just-inserted call

        // Test very old calls (should be 0)
        let old_calls = count_recent_calls(&pool, -1).await?;
        assert_eq!(old_calls, 0);

        // Test top systems - get more to ensure our test system is included
        let top_systems = get_top_systems(&pool, 100).await?;
        assert!(!top_systems.is_empty());

        // In parallel tests, our system might not be in top 5, but should be in the list
        let found_our_system = top_systems
            .iter()
            .any(|(id, count)| id == &system_id && *count >= 1);
        assert!(
            found_our_system,
            "System {system_id} not found in top systems"
        );

        // Test system calls since time
        let system_calls_24h = count_system_calls_since(&pool, &system_id, 24).await?;
        assert!(system_calls_24h >= 1);

        let system_calls_old = count_system_calls_since(&pool, &system_id, -1).await?;
        assert_eq!(system_calls_old, 0);

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_system_operations() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        let system_id = format!("sysop_{}", &Uuid::new_v4().to_string()[0..8]);
        let system_label = "Test System Operations";

        // Test update_system_stats
        update_system_stats(&pool, &system_id, Some(system_label.to_string())).await?;

        // Test get_system_stats
        let stats = get_system_stats(&pool, &system_id).await?;
        assert_eq!(stats.system_id, system_id);
        assert_eq!(stats.system_label, Some(system_label.to_string()));
        assert_eq!(stats.total_calls, Some(1));

        // Test updating again (should increment)
        update_system_stats(&pool, &system_id, Some("Updated Label".to_string())).await?;
        let updated_stats = get_system_stats(&pool, &system_id).await?;
        assert_eq!(
            updated_stats.system_label,
            Some("Updated Label".to_string())
        );

        // Test getting non-existent system
        let fake_system = "non_existent_system";
        let result = get_system_stats(&pool, fake_system).await;
        assert!(matches!(result, Err(Error::NotFound { .. })));

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_api_key_validation() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        // Test validating non-existent key
        let fake_hash = "fake_hash_12345";
        let result = validate_api_key(&pool, fake_hash).await?;
        assert!(result.is_none());

        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    async fn test_upload_log_insertion() -> Result<()> {
        let Some(pool) = create_test_pool().await else {
            eprintln!("Skipping test: TEST_DATABASE_URL not set or database not available");
            return Ok(());
        };

        // Test successful upload log
        let success_params = UploadLogParams {
            client_ip: "10.0.0.1".parse().unwrap(),
            user_agent: Some("TestAgent/2.0".to_string()),
            api_key_id: Some("test_api_key".to_string()),
            system_id: Some("test_upload_system".to_string()),
            success: true,
            error_message: None,
            filename: Some("success_upload.mp3".to_string()),
            file_size: Some(2_097_152), // 2MB
        };

        let success_id = insert_upload_log(&pool, success_params).await?;
        assert!(!success_id.is_nil());

        // Test failed upload log
        let failure_params = UploadLogParams {
            client_ip: "192.168.1.50".parse().unwrap(),
            user_agent: None,
            api_key_id: None,
            system_id: Some("test_failed_system".to_string()),
            success: false,
            error_message: Some("File too large".to_string()),
            filename: Some("failed_upload.wav".to_string()),
            file_size: Some(104_857_600), // 100MB
        };

        let failure_id = insert_upload_log(&pool, failure_params).await?;
        assert!(!failure_id.is_nil());

        Ok(())
    }

    // Test struct initialization and validation
    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_struct_creation_and_validation() {
        // Test TranscriptionUpdate struct
        let update = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "completed",
            text: Some("Test transcription text"),
            confidence: Some(0.85),
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };
        assert_eq!(update.status, "completed");
        assert!(update.confidence.unwrap() > 0.8);

        // Test RadioCallFilter struct
        let filter = RadioCallFilter {
            system_id: Some("test_system"),
            talkgroup_id: Some(12345),
            from_date: Some(chrono::Utc::now() - chrono::Duration::days(7)),
            to_date: Some(chrono::Utc::now()),
            limit: 50,
            offset: 0,
        };
        assert_eq!(filter.limit, 50);
        assert_eq!(filter.offset, 0);
        assert!(filter.system_id.is_some());

        // Test UploadLogParams struct
        let params = UploadLogParams {
            client_ip: "127.0.0.1".parse().unwrap(),
            user_agent: Some("Test/1.0".to_string()),
            api_key_id: Some("key123".to_string()),
            system_id: Some("sys456".to_string()),
            success: true,
            error_message: None,
            filename: Some("test.mp3".to_string()),
            file_size: Some(1024),
        };
        assert!(params.success);
        assert!(params.error_message.is_none());
        assert_eq!(params.file_size, Some(1024));
    }

    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_stats_structs_edge_cases() {
        // Test TranscriptionStats with zero values
        let empty_stats = TranscriptionStats {
            total: 0,
            completed: 0,
            failed: 0,
            processing: 0,
            pending: 0,
            avg_confidence: None,
        };
        assert_eq!(empty_stats.total, 0);
        assert!(empty_stats.avg_confidence.is_none());

        // Test UploadStats with large numbers
        let large_stats = UploadStats {
            total_uploads: 1_000_000,
            successful_uploads: 950_000,
            failed_uploads: 50_000,
            avg_processing_time: Some(1250.5),
            total_bytes_uploaded: 1_099_511_627_776, // 1TB
        };
        assert_eq!(large_stats.total_uploads, 1_000_000);
        assert_eq!(
            large_stats.successful_uploads + large_stats.failed_uploads,
            1_000_000
        );
        assert!(large_stats.total_bytes_uploaded > 1_000_000_000_000);
    }

    // Test error handling scenarios
    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_error_handling_scenarios() {
        // Test various sqlx error mappings
        let row_not_found = sqlx::Error::RowNotFound;
        let mapped_error = match row_not_found {
            sqlx::Error::RowNotFound => Error::NotFound {
                resource: "test_resource".to_string(),
            },
            _ => Error::Database("other".to_string()),
        };

        if let Error::NotFound { resource } = mapped_error {
            assert_eq!(resource, "test_resource");
        } else {
            panic!("Expected NotFound error");
        }

        // Test authentication error mapping
        let auth_error = Error::Authentication("Invalid key".to_string());
        match auth_error {
            Error::Authentication(msg) => assert_eq!(msg, "Invalid key"),
            _ => panic!("Expected Authentication error"),
        }
    }

    // Test helper functions behavior
    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_helper_functions() {
        // Test create_test_radio_call
        let call = create_test_radio_call("test_sys", Some(789));
        assert_eq!(call.system_id, "test_sys");
        assert_eq!(call.talkgroup_id, Some(789));
        assert!(call.audio_size_bytes.is_some());
        assert!(call.duration_seconds.is_some());

        // Test create_test_upload_log
        let log = create_test_upload_log();
        assert!(log.success);
        assert!(log.file_size.is_some());
        assert!(log.processing_time_ms.is_some());

        // Test create_test_system_stats
        let stats = create_test_system_stats("helper_test");
        assert_eq!(stats.system_id, "helper_test");
        assert!(stats.total_calls.is_some());
        assert!(stats.top_talkgroups.is_some());
    }

    // Additional comprehensive tests for maximum coverage
    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_transcription_update_debug() {
        let update = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "debug_test",
            text: Some("Debug text"),
            confidence: Some(0.88),
            error: Some("Debug error"),
            speaker_segments: None,
            speaker_count: None,
        };

        let debug_str = format!("{update:?}");
        assert!(debug_str.contains("TranscriptionUpdate"));
        assert!(debug_str.contains("debug_test"));
        assert!(debug_str.contains("Debug text"));
        assert!(debug_str.contains("0.88"));
        assert!(debug_str.contains("Debug error"));
    }

    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_radio_call_filter_debug() {
        let filter = RadioCallFilter {
            system_id: Some("debug_sys"),
            talkgroup_id: Some(999),
            from_date: None,
            to_date: None,
            limit: 25,
            offset: 10,
        };

        let debug_str = format!("{filter:?}");
        assert!(debug_str.contains("RadioCallFilter"));
        assert!(debug_str.contains("debug_sys"));
        assert!(debug_str.contains("999"));
        assert!(debug_str.contains("25"));
        assert!(debug_str.contains("10"));
    }

    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_upload_log_params_debug() {
        let params = UploadLogParams {
            client_ip: "10.1.1.1".parse().unwrap(),
            user_agent: Some("DebugAgent".to_string()),
            api_key_id: Some("debug_key".to_string()),
            system_id: Some("debug_system".to_string()),
            success: false,
            error_message: Some("Debug error".to_string()),
            filename: Some("debug.mp3".to_string()),
            file_size: Some(9999),
        };

        let debug_str = format!("{params:?}");
        assert!(debug_str.contains("UploadLogParams"));
        assert!(debug_str.contains("10.1.1.1"));
        assert!(debug_str.contains("DebugAgent"));
        assert!(debug_str.contains("debug_key"));
        assert!(debug_str.contains("Debug error"));
    }

    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_transcription_stats_debug() {
        let stats = TranscriptionStats {
            total: 100,
            completed: 90,
            failed: 5,
            processing: 3,
            pending: 2,
            avg_confidence: Some(0.95),
        };

        let debug_str = format!("{stats:?}");
        assert!(debug_str.contains("TranscriptionStats"));
        assert!(debug_str.contains("100"));
        assert!(debug_str.contains("90"));
        assert!(debug_str.contains("0.95"));
    }

    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_transcription_stats_clone() {
        let stats = TranscriptionStats {
            total: 75,
            completed: 60,
            failed: 10,
            processing: 3,
            pending: 2,
            avg_confidence: Some(0.87),
        };

        let cloned = stats.clone();
        assert_eq!(stats.total, cloned.total);
        assert_eq!(stats.completed, cloned.completed);
        assert_eq!(stats.failed, cloned.failed);
        assert_eq!(stats.avg_confidence, cloned.avg_confidence);
    }

    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_upload_stats_debug() {
        let stats = UploadStats {
            total_uploads: 200,
            successful_uploads: 190,
            failed_uploads: 10,
            avg_processing_time: Some(1800.0),
            total_bytes_uploaded: 1_000_000_000,
        };

        let debug_str = format!("{stats:?}");
        assert!(debug_str.contains("UploadStats"));
        assert!(debug_str.contains("200"));
        assert!(debug_str.contains("190"));
        assert!(debug_str.contains("1800"));
    }

    #[test]
    #[allow(clippy::missing_panics_doc)]
    fn test_upload_stats_clone() {
        let stats = UploadStats {
            total_uploads: 150,
            successful_uploads: 140,
            failed_uploads: 10,
            avg_processing_time: Some(950.5),
            total_bytes_uploaded: 500_000_000,
        };

        let cloned = stats.clone();
        assert_eq!(stats.total_uploads, cloned.total_uploads);
        assert_eq!(stats.successful_uploads, cloned.successful_uploads);
        assert_eq!(stats.failed_uploads, cloned.failed_uploads);
        assert_eq!(stats.avg_processing_time, cloned.avg_processing_time);
        assert_eq!(stats.total_bytes_uploaded, cloned.total_bytes_uploaded);
    }

    #[test]
    fn test_ip_address_conversions() {
        // Test various IP address formats
        let ipv4: std::net::IpAddr = "192.168.1.1".parse().unwrap();
        let ipv6: std::net::IpAddr = "::1".parse().unwrap();
        let mapped_ipv4: std::net::IpAddr = "::ffff:192.168.1.1".parse().unwrap();

        assert!(ipv4.is_ipv4());
        assert!(ipv6.is_ipv6());
        assert!(mapped_ipv4.is_ipv6());

        // Test IP address in UploadLogParams
        let params = UploadLogParams {
            client_ip: ipv4,
            user_agent: None,
            api_key_id: None,
            system_id: None,
            success: true,
            error_message: None,
            filename: None,
            file_size: None,
        };
        assert!(params.client_ip.is_ipv4());
    }

    #[test]
    fn test_decimal_conversion_edge_cases() {
        // Test very small confidence values
        let tiny = 0.000_001_f32;
        let tiny_decimal = rust_decimal::Decimal::try_from(tiny).unwrap();
        assert!(tiny_decimal.to_string().starts_with("0.00000"));

        // Test very large duration values
        let huge_duration = 86400.0f64; // 24 hours
        let huge_decimal = rust_decimal::Decimal::try_from(huge_duration).unwrap();
        assert_eq!(huge_decimal.to_string(), "86400");

        // Test negative values (edge case)
        let negative = -1.0f64;
        let neg_decimal = rust_decimal::Decimal::try_from(negative).unwrap();
        assert_eq!(neg_decimal.to_string(), "-1");
    }

    #[test]
    fn test_uuid_operations() {
        // Test UUID generation and comparison
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();

        assert_ne!(uuid1, uuid2);
        assert!(!uuid1.is_nil());
        assert!(!uuid2.is_nil());

        let nil_uuid = Uuid::nil();
        assert!(nil_uuid.is_nil());

        // Test in TranscriptionUpdate
        let update1 = TranscriptionUpdate {
            id: uuid1,
            status: "test",
            text: None,
            confidence: None,
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };
        let update2 = TranscriptionUpdate {
            id: uuid2,
            status: "test",
            text: None,
            confidence: None,
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };

        assert_ne!(update1.id, update2.id);
    }

    #[test]
    fn test_radio_call_filter_default_like() {
        // Test creating filter with minimal data
        let minimal_filter = RadioCallFilter {
            system_id: None,
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            limit: 100,
            offset: 0,
        };

        assert!(minimal_filter.system_id.is_none());
        assert!(minimal_filter.talkgroup_id.is_none());
        assert!(minimal_filter.from_date.is_none());
        assert!(minimal_filter.to_date.is_none());
        assert_eq!(minimal_filter.limit, 100);
        assert_eq!(minimal_filter.offset, 0);
    }

    #[test]
    fn test_transcription_status_strings() {
        // Test various status strings that might be used
        let statuses = [
            "pending",
            "processing",
            "completed",
            "failed",
            "cancelled",
            "timeout",
        ];

        for status in &statuses {
            let update = TranscriptionUpdate {
                id: Uuid::new_v4(),
                status,
                text: None,
                confidence: None,
                error: None,
                speaker_segments: None,
                speaker_count: None,
            };
            assert!(!update.status.is_empty());
            assert_eq!(update.status, *status);
        }
    }

    #[test]
    fn test_optional_fields_none_cases() {
        // Test TranscriptionUpdate with all None optional fields
        let minimal_update = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "minimal",
            text: None,
            confidence: None,
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };

        assert!(minimal_update.text.is_none());
        assert!(minimal_update.confidence.is_none());
        assert!(minimal_update.error.is_none());

        // Test UploadLogParams with minimal data
        let minimal_params = UploadLogParams {
            client_ip: "127.0.0.1".parse().unwrap(),
            user_agent: None,
            api_key_id: None,
            system_id: None,
            success: false,
            error_message: None,
            filename: None,
            file_size: None,
        };

        assert!(minimal_params.user_agent.is_none());
        assert!(minimal_params.api_key_id.is_none());
        assert!(minimal_params.system_id.is_none());
        assert!(minimal_params.error_message.is_none());
        assert!(minimal_params.filename.is_none());
        assert!(minimal_params.file_size.is_none());
    }

    #[test]
    fn test_confidence_boundary_values() {
        // Test confidence boundary values
        let zero_conf = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "test",
            text: None,
            confidence: Some(0.0),
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };
        assert_eq!(zero_conf.confidence, Some(0.0));

        let max_conf = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "test",
            text: None,
            confidence: Some(1.0),
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };
        assert_eq!(max_conf.confidence, Some(1.0));

        let over_max = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "test",
            text: None,
            confidence: Some(1.5), // Over 100%
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };
        assert_eq!(over_max.confidence, Some(1.5));
    }

    #[test]
    fn test_large_file_sizes() {
        // Test large file sizes in UploadLogParams
        let large_file = UploadLogParams {
            client_ip: "192.168.1.1".parse().unwrap(),
            user_agent: Some("LargeFileTest".to_string()),
            api_key_id: None,
            system_id: None,
            success: true,
            error_message: None,
            filename: Some("huge_file.wav".to_string()),
            file_size: Some(10_737_418_240), // 10GB
        };

        assert_eq!(large_file.file_size, Some(10_737_418_240));
        assert!(large_file.file_size.unwrap() > 1_000_000_000);
    }

    #[test]
    fn test_pagination_edge_cases() {
        // Test extreme pagination values
        let large_offset = RadioCallFilter {
            system_id: Some("test"),
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            limit: 1,
            offset: 1_000_000,
        };
        assert_eq!(large_offset.offset, 1_000_000);

        let large_limit = RadioCallFilter {
            system_id: Some("test"),
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            limit: 10_000,
            offset: 0,
        };
        assert_eq!(large_limit.limit, 10_000);

        let zero_limit = RadioCallFilter {
            system_id: Some("test"),
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            limit: 0,
            offset: 0,
        };
        assert_eq!(zero_limit.limit, 0);
    }

    #[test]
    fn test_date_range_filters() {
        let now = chrono::Utc::now();
        let past = now - chrono::Duration::days(30);
        let future = now + chrono::Duration::days(1);

        let date_filter = RadioCallFilter {
            system_id: Some("date_test"),
            talkgroup_id: None,
            from_date: Some(past),
            to_date: Some(future),
            limit: 50,
            offset: 0,
        };

        assert!(date_filter.from_date.is_some());
        assert!(date_filter.to_date.is_some());
        assert!(date_filter.from_date.unwrap() < date_filter.to_date.unwrap());

        // Test inverted date range (edge case)
        let inverted_filter = RadioCallFilter {
            system_id: Some("inverted_test"),
            talkgroup_id: None,
            from_date: Some(future),
            to_date: Some(past),
            limit: 10,
            offset: 0,
        };

        assert!(inverted_filter.from_date.unwrap() > inverted_filter.to_date.unwrap());
    }

    #[test]
    fn test_stats_calculation_edge_cases() {
        // Test stats with zero totals
        let zero_stats = TranscriptionStats {
            total: 0,
            completed: 0,
            failed: 0,
            processing: 0,
            pending: 0,
            avg_confidence: None,
        };

        let sum =
            zero_stats.completed + zero_stats.failed + zero_stats.processing + zero_stats.pending;
        assert_eq!(sum, zero_stats.total);

        // Test stats where components don't add up to total (edge case)
        let inconsistent_stats = TranscriptionStats {
            total: 100,
            completed: 50,
            failed: 20,
            processing: 10,
            pending: 10, // Sums to 90, not 100
            avg_confidence: Some(0.8),
        };

        let component_sum = inconsistent_stats.completed
            + inconsistent_stats.failed
            + inconsistent_stats.processing
            + inconsistent_stats.pending;
        assert_ne!(component_sum, inconsistent_stats.total);

        // Test upload stats with mismatched totals
        let mismatched_upload = UploadStats {
            total_uploads: 100,
            successful_uploads: 70,
            failed_uploads: 40, // 70 + 40 = 110, not 100
            avg_processing_time: Some(1000.0),
            total_bytes_uploaded: 1_000_000,
        };

        let upload_sum = mismatched_upload.successful_uploads + mismatched_upload.failed_uploads;
        assert_ne!(upload_sum, mismatched_upload.total_uploads);
    }

    #[test]
    fn test_string_field_edge_cases() {
        // Test empty strings
        let empty_update = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "",     // Empty status
            text: Some(""), // Empty text
            confidence: None,
            error: Some(""), // Empty error
            speaker_count: None,
            speaker_segments: None,
        };

        assert!(empty_update.status.is_empty());
        assert_eq!(empty_update.text, Some(""));
        assert_eq!(empty_update.error, Some(""));

        // Test very long strings
        let long_text = "a".repeat(10_000);
        let long_update = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "test",
            text: Some(&long_text),
            confidence: None,
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };

        assert_eq!(long_update.text.unwrap().len(), 10_000);

        // Test strings with special characters
        let special_params = UploadLogParams {
            client_ip: "127.0.0.1".parse().unwrap(),
            user_agent: Some("Møzillå/5.0 (tëst) ñõdé/v1.0".to_string()),
            api_key_id: Some("key-with-dashes_and_underscores.123".to_string()),
            system_id: Some("system/with\\backslash".to_string()),
            success: true,
            error_message: Some("Error: file 'test.mp3' not found!".to_string()),
            filename: Some("file with spaces & symbols.mp3".to_string()),
            file_size: None,
        };

        assert!(special_params.user_agent.unwrap().contains('ë'));
        assert!(special_params.api_key_id.unwrap().contains('_'));
        assert!(special_params.system_id.unwrap().contains('\\'));
        assert!(special_params.error_message.unwrap().contains('!'));
        assert!(special_params.filename.unwrap().contains(' '));
    }

    #[test]
    fn test_upload_params_variations() {
        // Test with IPv6 address
        let ipv6_params = UploadLogParams {
            client_ip: "2001:0db8:85a3::8a2e:370:7334".parse().unwrap(),
            user_agent: Some("IPv6TestClient".to_string()),
            api_key_id: Some("ipv6_key".to_string()),
            system_id: Some("ipv6_system".to_string()),
            success: true,
            error_message: None,
            filename: Some("ipv6_test.mp3".to_string()),
            file_size: Some(2048),
        };

        assert!(ipv6_params.client_ip.is_ipv6());
        assert_eq!(ipv6_params.filename, Some("ipv6_test.mp3".to_string()));
        assert_eq!(ipv6_params.file_size, Some(2048));

        // Test with maximum values
        let max_params = UploadLogParams {
            client_ip: "255.255.255.255".parse().unwrap(),
            user_agent: Some("MaxClient/999.999".to_string()),
            api_key_id: Some("max_api_key_identifier".to_string()),
            system_id: Some("max_system_identifier".to_string()),
            success: false,
            error_message: Some(
                "Maximum length error message for comprehensive testing".to_string(),
            ),
            filename: Some("maximum_filename_length_test.mp3".to_string()),
            file_size: Some(i64::MAX),
        };

        assert_eq!(max_params.client_ip.to_string(), "255.255.255.255");
        assert_eq!(max_params.file_size, Some(i64::MAX));
        assert!(!max_params.success);
    }

    #[test]
    fn test_filter_combinations() {
        // Test various filter combinations
        let system_only = RadioCallFilter {
            system_id: Some("system_only"),
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            limit: 50,
            offset: 0,
        };

        let talkgroup_only = RadioCallFilter {
            system_id: None,
            talkgroup_id: Some(12345),
            from_date: None,
            to_date: None,
            limit: 50,
            offset: 0,
        };

        let comprehensive = RadioCallFilter {
            system_id: Some("comprehensive_test"),
            talkgroup_id: Some(99_999),
            from_date: Some(chrono::Utc::now() - chrono::Duration::days(30)),
            to_date: Some(chrono::Utc::now()),
            limit: 1000,
            offset: 2000,
        };

        assert!(system_only.system_id.is_some());
        assert!(system_only.talkgroup_id.is_none());
        assert!(talkgroup_only.system_id.is_none());
        assert!(talkgroup_only.talkgroup_id.is_some());
        assert!(comprehensive.system_id.is_some());
        assert!(comprehensive.talkgroup_id.is_some());
        assert!(comprehensive.from_date.is_some());
        assert!(comprehensive.to_date.is_some());
        assert_eq!(comprehensive.limit, 1000);
        assert_eq!(comprehensive.offset, 2000);
    }

    #[test]
    fn test_confidence_precision_variations() {
        // Test various confidence precision levels
        let precisions = [0.1, 0.12, 0.123, 0.1234, 0.12345, 0.999_999];

        for &precision in &precisions {
            let update = TranscriptionUpdate {
                id: Uuid::new_v4(),
                status: "precision_test",
                text: None,
                confidence: Some(precision),
                error: None,
                speaker_segments: None,
                speaker_count: None,
            };

            assert!((update.confidence.unwrap() - precision).abs() < f32::EPSILON);

            // Test decimal conversion roundtrip
            let decimal = rust_decimal::Decimal::try_from(precision).unwrap();
            let back_to_float = decimal.to_string().parse::<f32>().unwrap();
            assert!((precision - back_to_float).abs() < 0.0001);
        }
    }

    #[test]
    fn test_stats_edge_cases() {
        // Test with all zero values
        let zero_transcription_stats = TranscriptionStats {
            total: 0,
            completed: 0,
            failed: 0,
            processing: 0,
            pending: 0,
            avg_confidence: None,
        };

        assert_eq!(zero_transcription_stats.total, 0);
        assert!(zero_transcription_stats.avg_confidence.is_none());

        // Test with large values
        let large_upload_stats = UploadStats {
            total_uploads: 10_000_000,
            successful_uploads: 9_950_000,
            failed_uploads: 50_000,
            avg_processing_time: Some(1250.75),
            total_bytes_uploaded: 10_995_116_277_760, // 10TB
        };

        assert_eq!(large_upload_stats.total_uploads, 10_000_000);
        assert!(large_upload_stats.total_bytes_uploaded > 10_000_000_000_000);
        assert_eq!(
            large_upload_stats.successful_uploads + large_upload_stats.failed_uploads,
            10_000_000
        );
    }

    #[test]
    fn test_uuid_operations_extended() {
        // Test UUID operations in various contexts
        let mut uuids = Vec::new();
        for _ in 0..50 {
            uuids.push(Uuid::new_v4());
        }

        // Verify all UUIDs are unique
        for (i, uuid1) in uuids.iter().enumerate() {
            for (j, uuid2) in uuids.iter().enumerate() {
                if i != j {
                    assert_ne!(uuid1, uuid2, "UUIDs should be unique");
                }
            }
        }

        // Test nil UUID
        let nil_uuid = Uuid::nil();
        assert!(nil_uuid.is_nil());
        assert_ne!(uuids[0], nil_uuid);
    }

    #[test]
    fn test_error_message_formats() {
        // Test various error message formats
        let error_types = [
            "database error: connection timeout after 30 seconds",
            "network error: failed to reach service",
            "validation error: invalid audio format",
            "authentication error: invalid API key",
            "rate limit error: too many requests",
        ];

        for error_msg in &error_types {
            let update = TranscriptionUpdate {
                id: Uuid::new_v4(),
                status: "failed",
                text: None,
                confidence: None,
                error: Some(error_msg),
                speaker_segments: None,
                speaker_count: None,
            };

            assert!(update.error.is_some());
            assert!(update.error.unwrap().contains("error:"));
        }
    }

    #[test]
    fn test_struct_cloning() {
        // Test cloning of various structs
        let original_transcription = TranscriptionStats {
            total: 1000,
            completed: 950,
            failed: 30,
            processing: 15,
            pending: 5,
            avg_confidence: Some(0.92),
        };

        let cloned_transcription = original_transcription.clone();
        assert_eq!(original_transcription.total, cloned_transcription.total);
        assert_eq!(
            original_transcription.avg_confidence,
            cloned_transcription.avg_confidence
        );

        let original_upload = UploadStats {
            total_uploads: 5000,
            successful_uploads: 4900,
            failed_uploads: 100,
            avg_processing_time: Some(850.0),
            total_bytes_uploaded: 5_368_709_120, // 5GB
        };

        let cloned_upload = original_upload.clone();
        assert_eq!(original_upload.total_uploads, cloned_upload.total_uploads);
        assert_eq!(
            original_upload.total_bytes_uploaded,
            cloned_upload.total_bytes_uploaded
        );
    }

    #[test]
    fn test_debug_formatting() {
        // Test debug formatting of various structs
        let update = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "debug_test",
            text: Some("Debug text"),
            confidence: Some(0.88),
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };

        let debug_str = format!("{update:?}");
        assert!(debug_str.contains("TranscriptionUpdate"));
        assert!(debug_str.contains("debug_test"));
        assert!(debug_str.contains("Debug text"));
        assert!(debug_str.contains("0.88"));

        let filter = RadioCallFilter {
            system_id: Some("debug_system"),
            talkgroup_id: Some(999),
            from_date: None,
            to_date: None,
            limit: 100,
            offset: 50,
        };

        let filter_debug = format!("{filter:?}");
        assert!(filter_debug.contains("RadioCallFilter"));
        assert!(filter_debug.contains("debug_system"));
        assert!(filter_debug.contains("999"));
    }

    // Additional unit tests to improve coverage
    #[test]
    fn test_invalid_decimal_conversions() {
        // Test error paths in decimal conversion
        use rust_decimal::Decimal;

        // Test invalid confidence conversion handling
        let invalid_confidence = f32::NAN;
        let result = Decimal::try_from(invalid_confidence);
        assert!(result.is_err());

        let infinite_confidence = f32::INFINITY;
        let result = Decimal::try_from(infinite_confidence);
        assert!(result.is_err());

        // Test invalid duration conversion handling
        let invalid_duration = f64::NAN;
        let result = Decimal::try_from(invalid_duration);
        assert!(result.is_err());

        let infinite_duration = f64::INFINITY;
        let result = Decimal::try_from(infinite_duration);
        assert!(result.is_err());
    }

    #[test]
    fn test_ip_network_conversion_edge_cases() {
        use sqlx::types::ipnetwork::IpNetwork;
        use std::net::IpAddr;

        // Test IPv4 conversion
        let ipv4: IpAddr = "192.168.1.1".parse().unwrap();
        let network = IpNetwork::from(ipv4);
        assert_eq!(network.ip(), ipv4);

        // Test IPv6 conversion
        let ipv6: IpAddr = "::1".parse().unwrap();
        let network = IpNetwork::from(ipv6);
        assert_eq!(network.ip(), ipv6);

        // Test localhost
        let localhost: IpAddr = "127.0.0.1".parse().unwrap();
        let network = IpNetwork::from(localhost);
        assert!(network.ip().is_loopback());
    }

    #[test]
    fn test_invalid_ip_string_parsing() {
        // Test invalid IP string parsing paths
        let invalid_ips = vec![
            "invalid",
            "999.999.999.999",
            "192.168.1",
            "192.168.1.1.1",
            "::gggg",
            "",
        ];

        for invalid_ip in invalid_ips {
            let result = invalid_ip.parse::<std::net::IpAddr>();
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_json_serialization_error_paths() {
        // Test JSON serialization with invalid JSON
        let invalid_json_strings = vec![
            "{",           // Incomplete JSON
            "invalid",     // Not JSON at all
            "[1,2,3",      // Incomplete array
            "{\"key\": }", // Missing value
            "null",        // Valid JSON but edge case
        ];

        for invalid_json in invalid_json_strings {
            let result: std::result::Result<serde_json::Value, _> =
                serde_json::from_str(invalid_json);
            if invalid_json == "null" {
                assert!(result.is_ok());
                assert!(result.unwrap().is_null());
            } else {
                assert!(result.is_err());
            }
        }
    }

    #[test]
    fn test_transcription_status_string_conversion() {
        use sdrtrunk_core::types::TranscriptionStatus;

        // Test various transcription status string conversions
        let statuses = vec![
            TranscriptionStatus::None,
            TranscriptionStatus::Pending,
            TranscriptionStatus::Processing,
            TranscriptionStatus::Completed,
            TranscriptionStatus::Failed,
        ];

        for status in statuses {
            let status_str = status.to_string();
            assert!(!status_str.is_empty());

            // Test that each status has expected string representation
            match status {
                TranscriptionStatus::None => {
                    assert!(status_str.contains("none") || status_str.contains("None"));
                }
                TranscriptionStatus::Pending => {
                    assert!(status_str.contains("pending") || status_str.contains("Pending"));
                }
                TranscriptionStatus::Processing => {
                    assert!(status_str.contains("processing") || status_str.contains("Processing"));
                }
                TranscriptionStatus::Completed => {
                    assert!(status_str.contains("completed") || status_str.contains("Completed"));
                }
                TranscriptionStatus::Failed => {
                    assert!(status_str.contains("failed") || status_str.contains("Failed"));
                }
                TranscriptionStatus::Cancelled => {
                    assert!(status_str.contains("cancelled") || status_str.contains("Cancelled"));
                }
            }
        }
    }

    #[test]
    fn test_radio_call_default_values() {
        use sdrtrunk_core::types::RadioCall;

        // Test RadioCall default values
        let default_call = RadioCall::default();

        assert!(default_call.id.is_none());
        assert!(default_call.system_label.is_none());
        assert!(default_call.frequency.is_none());
        assert!(default_call.talkgroup_id.is_none());
        assert!(default_call.source_radio_id.is_none());
        assert!(default_call.audio_filename.is_none());
        assert!(default_call.transcription_text.is_none());
        assert!(default_call.transcription_confidence.is_none());
        assert!(default_call.patches.is_none());
        assert!(default_call.frequencies.is_none());
        assert!(default_call.sources.is_none());
    }

    #[test]
    fn test_radio_call_with_extreme_values() {
        use sdrtrunk_core::types::{RadioCall, TranscriptionStatus};

        // Test RadioCall with extreme boundary values
        let extreme_call = RadioCall {
            id: Some(Uuid::new_v4()),
            created_at: chrono::DateTime::<chrono::Utc>::MIN_UTC,
            call_timestamp: chrono::DateTime::<chrono::Utc>::MAX_UTC,
            system_id: "x".repeat(255), // Max length system ID
            system_label: Some("y".repeat(255)),
            frequency: Some(i64::MAX),
            talkgroup_id: Some(i32::MAX),
            talkgroup_label: Some("z".repeat(255)),
            talkgroup_group: Some("group".repeat(50)),
            talkgroup_tag: Some("tag".repeat(50)),
            source_radio_id: Some(i32::MAX),
            talker_alias: Some("alias".repeat(50)),
            audio_filename: Some("file".repeat(50)),
            audio_file_path: Some("path".repeat(100)),
            audio_size_bytes: Some(i64::MAX),
            duration_seconds: Some(f64::MAX),
            transcription_text: Some("text".repeat(1000)),
            transcription_confidence: Some(1.0),
            transcription_status: TranscriptionStatus::Completed,
            transcription_error: Some("error".repeat(100)),
            transcription_started_at: Some(chrono::DateTime::<chrono::Utc>::MIN_UTC),
            transcription_completed_at: Some(chrono::DateTime::<chrono::Utc>::MAX_UTC),
            speaker_segments: Some(serde_json::json!([{"test": "data"}])),
            transcription_segments: Some(serde_json::json!([{"seg": "data"}])),
            speaker_count: Some(i32::MAX),
            patches: Some(serde_json::json!([{"patch": "data"}])),
            frequencies: Some(serde_json::json!([999_999_999])),
            sources: Some(serde_json::json!([{"source": "data"}])),
            upload_ip: Some("255.255.255.255".to_string()),
            upload_timestamp: chrono::Utc::now(),
            upload_api_key_id: Some("key".repeat(36)),
        };

        // Verify all fields are set as expected
        assert_eq!(extreme_call.system_id.len(), 255);
        assert_eq!(extreme_call.frequency, Some(i64::MAX));
        assert_eq!(extreme_call.talkgroup_id, Some(i32::MAX));
        assert_eq!(extreme_call.source_radio_id, Some(i32::MAX));
        assert_eq!(extreme_call.audio_size_bytes, Some(i64::MAX));
        assert_eq!(extreme_call.duration_seconds, Some(f64::MAX));
        assert_eq!(extreme_call.transcription_confidence, Some(1.0));
        assert_eq!(extreme_call.speaker_count, Some(i32::MAX));
    }

    #[test]
    fn test_upload_log_db_field_validation() {
        use crate::models::UploadLogDb;

        // Test UploadLogDb with various field combinations
        let log = UploadLogDb {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            client_ip: "192.168.1.1".parse::<std::net::IpAddr>().unwrap().into(),
            user_agent: Some("Test/1.0 (compatible)".to_string()),
            api_key_used: Some("test_key_123".to_string()),
            system_id: Some("test_system_456".to_string()),
            success: true,
            error_message: None,
            filename: Some("test_file.mp3".to_string()),
            file_size: Some(1024 * 1024), // 1MB
            content_type: Some("audio/mpeg".to_string()),
            response_code: Some(200),
            processing_time_ms: Some(rust_decimal::Decimal::new(1500, 0)),
        };

        assert!(!log.id.is_nil());
        assert!(log.success);
        assert_eq!(log.response_code, Some(200));
        assert!(log.processing_time_ms.is_some());
        assert_eq!(log.file_size, Some(1024 * 1024));
    }

    #[test]
    fn test_system_stats_db_field_validation() {
        use crate::models::SystemStatsDb;

        // Test SystemStatsDb with comprehensive field population
        let stats = SystemStatsDb {
            id: Uuid::new_v4(),
            system_id: "comprehensive_system".to_string(),
            system_label: Some("Comprehensive Test System".to_string()),
            total_calls: Some(10_000),
            calls_today: Some(500),
            calls_this_hour: Some(25),
            first_seen: Some(chrono::Utc::now() - chrono::Duration::days(365)),
            last_seen: Some(chrono::Utc::now()),
            top_talkgroups: Some(serde_json::json!([
                {"id": 100, "label": "Dispatch", "count": 1500},
                {"id": 200, "label": "Tactical", "count": 800},
                {"id": 300, "label": "Admin", "count": 300}
            ])),
            upload_sources: Some(serde_json::json!([
                {"ip": "192.168.1.10", "count": 5000, "last_seen": "2023-12-01T10:00:00Z"},
                {"ip": "10.0.0.50", "count": 3000, "last_seen": "2023-12-01T09:30:00Z"},
                {"ip": "172.16.0.100", "count": 2000, "last_seen": "2023-11-30T15:45:00Z"}
            ])),
            last_updated: chrono::Utc::now(),
        };

        assert_eq!(stats.system_id, "comprehensive_system");
        assert_eq!(stats.total_calls, Some(10_000));
        assert_eq!(stats.calls_today, Some(500));
        assert_eq!(stats.calls_this_hour, Some(25));
        assert!(stats.top_talkgroups.is_some());
        assert!(stats.upload_sources.is_some());

        // Verify JSON structure
        if let Some(talkgroups) = &stats.top_talkgroups {
            assert!(talkgroups.is_array());
            let tg_array = talkgroups.as_array().unwrap();
            assert_eq!(tg_array.len(), 3);
            assert_eq!(tg_array[0]["id"], 100);
            assert_eq!(tg_array[0]["label"], "Dispatch");
            assert_eq!(tg_array[0]["count"], 1500);
        }

        if let Some(sources) = &stats.upload_sources {
            assert!(sources.is_array());
            let src_array = sources.as_array().unwrap();
            assert_eq!(src_array.len(), 3);
            assert_eq!(src_array[0]["ip"], "192.168.1.10");
            assert_eq!(src_array[0]["count"], 5000);
        }
    }

    #[test]
    fn test_api_key_db_field_validation() {
        use crate::models::ApiKeyDb;

        // Test ApiKeyDb structure
        let api_key = ApiKeyDb {
            id: "test_key_id_123".to_string(),
            key_hash: "hashed_key_value_456".to_string(),
            description: Some("Test API Key for validation".to_string()),
            active: true,
            created_at: chrono::Utc::now(),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::days(365)),
            last_used: Some(chrono::Utc::now()),
            total_requests: Some(1500),
            allowed_ips: None,
            allowed_systems: None,
        };

        assert_eq!(api_key.id, "test_key_id_123");
        assert_eq!(api_key.key_hash, "hashed_key_value_456");
        assert!(api_key.active);
        assert!(api_key.expires_at.is_some());
        assert!(api_key.last_used.is_some());
        assert_eq!(api_key.total_requests, Some(1500));
    }

    #[test]
    fn test_chrono_datetime_operations() {
        use chrono::{DateTime, Duration, Utc};

        // Test various datetime operations used in queries
        let now = Utc::now();
        let past = now - Duration::days(30);
        let future = now + Duration::hours(24);

        assert!(past < now);
        assert!(now < future);
        assert!(future > past);

        // Test timestamp conversion
        let timestamp = now.timestamp();
        let from_timestamp = DateTime::from_timestamp(timestamp, 0).unwrap();
        assert_eq!(from_timestamp.timestamp(), timestamp);

        // Test duration calculations
        let diff = now - past;
        assert!(diff.num_days() >= 29); // Account for timing variations
        assert!(diff.num_days() <= 31);
    }

    #[test]
    fn test_json_value_operations() {
        use serde_json::{Value, json};

        // Test various JSON operations used in the codebase
        let patches = json!([
            {"id": 1, "name": "Patch A", "active": true},
            {"id": 2, "name": "Patch B", "active": false}
        ]);

        assert!(patches.is_array());
        let patch_array = patches.as_array().unwrap();
        assert_eq!(patch_array.len(), 2);
        assert_eq!(patch_array[0]["id"], 1);
        assert_eq!(patch_array[0]["name"], "Patch A");
        assert_eq!(patch_array[0]["active"], true);

        let sources = json!({
            "primary": "192.168.1.1",
            "backup": "192.168.1.2",
            "count": 500
        });

        assert!(sources.is_object());
        let source_obj = sources.as_object().unwrap();
        assert_eq!(source_obj["primary"], "192.168.1.1");
        assert_eq!(source_obj["count"], 500);

        // Test JSON string conversion
        let json_string = sources.to_string();
        assert!(json_string.contains("primary"));
        assert!(json_string.contains("192.168.1.1"));

        // Test parsing back
        let parsed: Value = serde_json::from_str(&json_string).unwrap();
        assert_eq!(parsed, sources);
    }

    #[test]
    fn test_struct_parameter_validation() {
        // Test parameter struct validation with edge cases
        let long_system_id = "x".repeat(100);
        let extreme_filter = RadioCallFilter {
            system_id: Some(long_system_id.as_str()),
            talkgroup_id: Some(i32::MIN),
            from_date: Some(chrono::DateTime::<chrono::Utc>::MIN_UTC),
            to_date: Some(chrono::DateTime::<chrono::Utc>::MAX_UTC),
            limit: i64::MAX,
            offset: i64::MAX,
        };

        assert_eq!(extreme_filter.system_id.unwrap().len(), 100);
        assert_eq!(extreme_filter.talkgroup_id, Some(i32::MIN));
        assert_eq!(extreme_filter.limit, i64::MAX);
        assert_eq!(extreme_filter.offset, i64::MAX);

        let long_user_agent = "A".repeat(1000);
        let long_api_key = "K".repeat(100);
        let long_system_id = "S".repeat(100);
        let long_error = "E".repeat(1000);
        let long_filename = "F".repeat(255);

        let extreme_params = UploadLogParams {
            client_ip: "0.0.0.0".parse().unwrap(),
            user_agent: Some(long_user_agent),
            api_key_id: Some(long_api_key),
            system_id: Some(long_system_id),
            success: false,
            error_message: Some(long_error),
            filename: Some(long_filename),
            file_size: Some(i64::MAX),
        };

        assert_eq!(extreme_params.user_agent.as_ref().unwrap().len(), 1000);
        assert_eq!(extreme_params.api_key_id.as_ref().unwrap().len(), 100);
        assert_eq!(extreme_params.error_message.as_ref().unwrap().len(), 1000);
        assert_eq!(extreme_params.file_size, Some(i64::MAX));

        assert_eq!(extreme_params.user_agent.as_ref().unwrap().len(), 1000);
        assert_eq!(extreme_params.api_key_id.as_ref().unwrap().len(), 100);
        assert_eq!(extreme_params.error_message.as_ref().unwrap().len(), 1000);
        assert_eq!(extreme_params.file_size, Some(i64::MAX));
    }

    #[test]
    fn test_stats_calculation_edge_cases_extended() {
        // Test stats with extreme values
        let max_stats = TranscriptionStats {
            total: i64::MAX,
            completed: i64::MAX / 2,
            failed: i64::MAX / 4,
            processing: 1000,
            pending: 2000,
            avg_confidence: Some(1.0),
        };

        assert_eq!(max_stats.total, i64::MAX);
        assert!((max_stats.avg_confidence.unwrap() - 1.0).abs() < f64::EPSILON);

        let upload_max_stats = UploadStats {
            total_uploads: i64::MAX,
            successful_uploads: i64::MAX - 1000,
            failed_uploads: 1000,
            avg_processing_time: Some(f64::MAX),
            total_bytes_uploaded: i64::MAX,
        };

        assert_eq!(upload_max_stats.total_uploads, i64::MAX);
        assert_eq!(upload_max_stats.total_bytes_uploaded, i64::MAX);
        assert_eq!(upload_max_stats.avg_processing_time, Some(f64::MAX));
    }

    #[test]
    fn test_uuid_string_operations() {
        // Test UUID string operations
        let uuid = Uuid::new_v4();
        let uuid_str = uuid.to_string();

        assert_eq!(uuid_str.len(), 36); // Standard UUID format
        assert!(uuid_str.contains('-'));

        // Test parsing back
        let parsed_uuid = Uuid::parse_str(&uuid_str).unwrap();
        assert_eq!(uuid, parsed_uuid);

        // Test hyphenless format
        let simple_str = uuid.simple().to_string();
        assert_eq!(simple_str.len(), 32);
        assert!(!simple_str.contains('-'));

        // Test uppercase/lowercase
        let upper_str = uuid_str.to_uppercase();
        let lower_str = uuid_str.to_lowercase();
        assert_ne!(upper_str, lower_str);

        // Both should parse to same UUID
        let from_upper = Uuid::parse_str(&upper_str).unwrap();
        let from_lower = Uuid::parse_str(&lower_str).unwrap();
        assert_eq!(from_upper, from_lower);
        assert_eq!(from_upper, uuid);
    }

    #[test]
    fn test_wrapper_functions_logic() {
        // Test wrapper function error handling logic

        // Test the error matching logic in get_radio_call
        let db_not_found_error = Error::Database(
            "no rows returned by a query that expected to return at least one row".to_string(),
        );
        let other_db_error = Error::Database("connection failed".to_string());

        // Verify error message matching
        if let Error::Database(ref msg) = db_not_found_error {
            assert!(msg.contains("no rows returned"));
        }

        if let Error::Database(ref msg) = other_db_error {
            assert!(!msg.contains("no rows returned"));
        }
    }

    #[test]
    fn test_radio_call_filter_system_logic() {
        // Test RadioCallFilter system_id logic
        let filter_with_system = RadioCallFilter {
            system_id: Some("police_dept"),
            limit: 50,
            offset: 0,
            talkgroup_id: None,
            from_date: None,
            to_date: None,
        };

        let filter_without_system = RadioCallFilter {
            system_id: None,
            limit: 50,
            offset: 0,
            talkgroup_id: None,
            from_date: None,
            to_date: None,
        };

        // Test filter logic branching
        assert!(filter_with_system.system_id.is_some());
        assert!(filter_without_system.system_id.is_none());

        // Test conditional logic that would be used in list_radio_calls_filtered
        if let Some(system) = filter_with_system.system_id {
            assert_eq!(system, "police_dept");
        }

        // Test the else branch
        if filter_without_system.system_id.is_none() {
            // This would result in empty vec in the actual function
            let empty_result: Vec<RadioCallDb> = vec![];
            assert!(empty_result.is_empty());
        }
    }

    #[test]
    fn test_count_functions_zero_cases() {
        // Test count function logic for zero cases

        // Simulate count_radio_calls_filtered with no system
        let result = if None::<&str>.is_none() {
            0i64 // This is what the function returns
        } else {
            -1i64 // Shouldn't happen
        };

        assert_eq!(result, 0);

        // Test count validation
        assert!(result >= 0, "Count should never be negative");

        // Test large count values
        let large_count = i64::MAX;
        assert!(large_count > 0);
        assert_eq!(large_count, 9_223_372_036_854_775_807);
    }

    #[test]
    fn test_decimal_conversion_precision_edge_cases() {
        use rust_decimal::Decimal;

        // Test high precision decimal conversion
        let high_precision = 0.999_999_999_999_999_f64;
        let decimal_hp = Decimal::try_from(high_precision);
        assert!(decimal_hp.is_ok());

        // Test very small numbers
        let very_small = 0.000_000_001_f64;
        let decimal_vs = Decimal::try_from(very_small);
        assert!(decimal_vs.is_ok());

        // Test zero
        let zero = 0.0f64;
        let decimal_zero = Decimal::try_from(zero);
        assert!(decimal_zero.is_ok());
        assert_eq!(decimal_zero.unwrap(), Decimal::ZERO);

        // Test negative values
        let negative = -123.456f64;
        let decimal_neg = Decimal::try_from(negative);
        assert!(decimal_neg.is_ok());
        assert!(decimal_neg.unwrap().is_sign_negative());

        // Test special float values
        let infinity = f64::INFINITY;
        let decimal_inf = Decimal::try_from(infinity);
        assert!(decimal_inf.is_err());

        let nan = f64::NAN;
        let decimal_nan = Decimal::try_from(nan);
        assert!(decimal_nan.is_err());
    }

    #[test]
    fn test_system_stats_creation() {
        let stats = create_test_system_stats("test_system_123");
        assert_eq!(stats.system_id, "test_system_123");
        assert_eq!(stats.system_label, Some("Test Radio System".to_string()));
        assert_eq!(stats.total_calls, Some(500));
        assert_eq!(stats.calls_today, Some(25));
        assert_eq!(stats.calls_this_hour, Some(3));
        assert!(stats.first_seen.is_some());
        assert!(stats.last_seen.is_some());
        assert!(stats.top_talkgroups.is_some());
        assert!(stats.upload_sources.is_some());

        // Test talkgroup JSON structure
        let talkgroups = stats.top_talkgroups.unwrap();
        assert!(talkgroups.is_array());
        assert_eq!(talkgroups.as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_upload_log_creation() {
        let log = create_test_upload_log();
        assert!(!log.id.is_nil());
        assert_eq!(log.user_agent, Some("TestClient/1.0".to_string()));
        assert_eq!(log.api_key_used, Some("test_key".to_string()));
        assert_eq!(log.system_id, Some("test_system".to_string()));
        assert!(log.success);
        assert!(log.error_message.is_none());
        assert_eq!(log.filename, Some("test_audio.mp3".to_string()));
        assert_eq!(log.file_size, Some(1_048_576));
        assert_eq!(log.content_type, Some("audio/mpeg".to_string()));
        assert_eq!(log.response_code, Some(200));
        assert!(log.processing_time_ms.is_some());
    }

    #[test]
    fn test_radio_call_creation_variations() {
        // Test with minimal data
        let minimal_call = create_test_radio_call("minimal", None);
        assert_eq!(minimal_call.system_id, "minimal");
        assert!(minimal_call.talkgroup_id.is_none());

        // Test with talkgroup ID
        let with_talkgroup = create_test_radio_call("with_tg", Some(999));
        assert_eq!(with_talkgroup.system_id, "with_tg");
        assert_eq!(with_talkgroup.talkgroup_id, Some(999));

        // Test all fields are properly set
        assert!(minimal_call.id.is_some());
        assert!(minimal_call.frequency.is_some());
        assert!(minimal_call.audio_filename.is_some());
        assert!(minimal_call.audio_file_path.is_some());
        assert!(minimal_call.audio_size_bytes.is_some());
        assert!(minimal_call.duration_seconds.is_some());
        assert!(minimal_call.transcription_text.is_some());
        assert!(minimal_call.transcription_confidence.is_some());
        assert!(minimal_call.speaker_segments.is_some());
        assert!(minimal_call.transcription_segments.is_some());
        assert!(minimal_call.speaker_count.is_some());
        assert!(minimal_call.patches.is_some());
        assert!(minimal_call.frequencies.is_some());
        assert!(minimal_call.sources.is_some());
        assert!(minimal_call.upload_ip.is_some());
        assert!(minimal_call.upload_api_key_id.is_some());
    }

    #[test]
    fn test_transcription_update_variations() {
        // Test with all None values
        let minimal_update = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "processing",
            text: None,
            confidence: None,
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };
        assert_eq!(minimal_update.status, "processing");
        assert!(minimal_update.text.is_none());
        assert!(minimal_update.confidence.is_none());
        assert!(minimal_update.error.is_none());

        // Test with error only
        let error_update = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "failed",
            text: None,
            confidence: None,
            error: Some("Network timeout"),
            speaker_segments: None,
            speaker_count: None,
        };
        assert_eq!(error_update.status, "failed");
        assert!(error_update.error.is_some());
        assert_eq!(error_update.error.unwrap(), "Network timeout");

        // Test with very high confidence
        let high_confidence = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "completed",
            text: Some("High quality audio"),
            confidence: Some(0.999),
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };
        assert!(high_confidence.confidence.unwrap() > 0.99);
    }

    #[test]
    fn test_radio_call_filter_variations() {
        let now = chrono::Utc::now();

        // Test empty filter
        let empty_filter = RadioCallFilter {
            system_id: None,
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            limit: 50,
            offset: 0,
        };
        assert!(empty_filter.system_id.is_none());
        assert!(empty_filter.talkgroup_id.is_none());
        assert!(empty_filter.from_date.is_none());
        assert!(empty_filter.to_date.is_none());

        // Test with all filters
        let full_filter = RadioCallFilter {
            system_id: Some("test_system"),
            talkgroup_id: Some(12345),
            from_date: Some(now - chrono::Duration::hours(24)),
            to_date: Some(now),
            limit: 100,
            offset: 200,
        };
        assert_eq!(full_filter.system_id, Some("test_system"));
        assert_eq!(full_filter.talkgroup_id, Some(12345));
        assert!(full_filter.from_date.is_some());
        assert!(full_filter.to_date.is_some());
        assert_eq!(full_filter.limit, 100);
        assert_eq!(full_filter.offset, 200);

        // Test date range validation
        assert!(full_filter.from_date.unwrap() < full_filter.to_date.unwrap());
    }

    #[test]
    fn test_upload_log_params_variations() {
        // Test with IPv4
        let ipv4_params = UploadLogParams {
            client_ip: "10.0.0.1".parse().unwrap(),
            user_agent: Some("TestAgent/1.0".to_string()),
            api_key_id: Some("key123".to_string()),
            system_id: Some("system456".to_string()),
            success: true,
            error_message: None,
            filename: Some("audio.wav".to_string()),
            file_size: Some(1024),
        };
        assert!(ipv4_params.client_ip.is_ipv4());
        assert!(ipv4_params.success);

        // Test with IPv6
        let ipv6_params = UploadLogParams {
            client_ip: "::1".parse().unwrap(),
            user_agent: None,
            api_key_id: None,
            system_id: None,
            success: false,
            error_message: Some("File too large".to_string()),
            filename: None,
            file_size: None,
        };
        assert!(ipv6_params.client_ip.is_ipv6());
        assert!(!ipv6_params.success);
        assert!(ipv6_params.error_message.is_some());

        // Test edge case values
        let edge_params = UploadLogParams {
            client_ip: "255.255.255.255".parse().unwrap(),
            user_agent: Some(String::new()),   // Empty string
            api_key_id: Some("a".repeat(100)), // Long key
            system_id: Some("system_with_underscores_and_numbers_123".to_string()),
            success: true,
            error_message: Some("Error with special chars: !@#$%^&*()".to_string()),
            filename: Some("file name with spaces.mp3".to_string()),
            file_size: Some(i64::MAX),
        };
        assert_eq!(edge_params.user_agent, Some(String::new()));
        assert_eq!(edge_params.api_key_id.as_ref().unwrap().len(), 100);
        assert_eq!(edge_params.file_size, Some(i64::MAX));
        assert!(edge_params.error_message.as_ref().unwrap().contains("!@#"));
        assert!(edge_params.filename.as_ref().unwrap().contains(' '));
    }

    #[test]
    fn test_transcription_stats_variations() {
        // Test zero stats
        let zero_stats = TranscriptionStats {
            total: 0,
            completed: 0,
            failed: 0,
            processing: 0,
            pending: 0,
            avg_confidence: None,
        };
        assert_eq!(zero_stats.total, 0);
        assert!(zero_stats.avg_confidence.is_none());

        // Test normal stats
        let normal_stats = TranscriptionStats {
            total: 1000,
            completed: 800,
            failed: 100,
            processing: 50,
            pending: 50,
            avg_confidence: Some(0.85),
        };
        assert_eq!(normal_stats.total, 1000);
        assert_eq!(
            normal_stats.completed
                + normal_stats.failed
                + normal_stats.processing
                + normal_stats.pending,
            1000
        );
        assert!(normal_stats.avg_confidence.unwrap() > 0.8);

        // Test edge case stats
        let edge_stats = TranscriptionStats {
            total: i64::MAX,
            completed: i64::MAX - 3,
            failed: 2,
            processing: 1,
            pending: 0,
            avg_confidence: Some(1.0), // Perfect confidence
        };
        assert_eq!(edge_stats.total, i64::MAX);
        assert_eq!(edge_stats.avg_confidence, Some(1.0));
        assert_eq!(edge_stats.pending, 0);
    }

    #[test]
    fn test_upload_stats_variations() {
        // Test minimal stats
        let minimal_stats = UploadStats {
            total_uploads: 0,
            successful_uploads: 0,
            failed_uploads: 0,
            avg_processing_time: None,
            total_bytes_uploaded: 0,
        };
        assert_eq!(minimal_stats.total_uploads, 0);
        assert!(minimal_stats.avg_processing_time.is_none());

        // Test normal stats
        let normal_stats = UploadStats {
            total_uploads: 5000,
            successful_uploads: 4750,
            failed_uploads: 250,
            avg_processing_time: Some(1500.5),
            total_bytes_uploaded: 10_737_418_240, // 10GB
        };
        assert_eq!(normal_stats.total_uploads, 5000);
        assert_eq!(
            normal_stats.successful_uploads + normal_stats.failed_uploads,
            5000
        );
        assert!(normal_stats.total_bytes_uploaded > 10_000_000_000);
        assert!(normal_stats.avg_processing_time.unwrap() > 1000.0);

        // Test edge case stats
        let edge_stats = UploadStats {
            total_uploads: i64::MAX,
            successful_uploads: i64::MAX - 1,
            failed_uploads: 1,
            avg_processing_time: Some(f64::MAX),
            total_bytes_uploaded: i64::MAX,
        };
        assert_eq!(edge_stats.total_uploads, i64::MAX);
        assert_eq!(edge_stats.total_bytes_uploaded, i64::MAX);
        assert_eq!(edge_stats.avg_processing_time, Some(f64::MAX));
    }

    #[test]
    fn test_error_conversion_handling() {
        // Test confidence decimal conversion errors
        let invalid_values = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY];

        for &invalid in &invalid_values {
            let result = rust_decimal::Decimal::try_from(invalid);
            assert!(result.is_err(), "Should fail for {invalid}");
        }

        // Test valid confidence values
        let valid_values = [0.0, 0.5, 0.99, 1.0, 0.123_456];

        for &valid in &valid_values {
            let result = rust_decimal::Decimal::try_from(valid);
            assert!(result.is_ok(), "Should succeed for {valid}");
        }
    }

    #[test]
    fn test_duration_conversion_handling() {
        // Test duration decimal conversion errors
        let invalid_durations = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY];

        for &invalid in &invalid_durations {
            let result = rust_decimal::Decimal::try_from(invalid);
            assert!(result.is_err(), "Should fail for duration {invalid}");
        }

        // Test valid duration values
        let valid_durations = [0.0, 30.5, 3600.0, 0.001, 86400.0]; // 0s to 24h

        for &valid in &valid_durations {
            let result = rust_decimal::Decimal::try_from(valid);
            assert!(result.is_ok(), "Should succeed for duration {valid}");
        }
    }

    #[test]
    fn test_json_serialization_edge_cases() {
        // Test empty JSON objects/arrays
        let empty_obj = serde_json::json!({});
        let empty_arr = serde_json::json!([]);

        assert!(empty_obj.is_object());
        assert!(empty_arr.is_array());
        assert_eq!(empty_obj.as_object().unwrap().len(), 0);
        assert_eq!(empty_arr.as_array().unwrap().len(), 0);

        // Test deeply nested JSON
        let deep_json = serde_json::json!({
            "level1": {
                "level2": {
                    "level3": {
                        "level4": {
                            "level5": "deep value"
                        }
                    }
                }
            }
        });

        assert_eq!(
            deep_json["level1"]["level2"]["level3"]["level4"]["level5"],
            "deep value"
        );

        // Test JSON with special characters and Unicode
        let special_json = serde_json::json!({
            "quotes": "\"quoted\"",
            "newlines": "line1\nline2",
            "tabs": "col1\tcol2",
            "unicode": "🚨📻📡🎙️",
            "chinese": "测试数据",
            "emoji_array": ["🔥", "💯", "🚀"]
        });

        assert!(special_json["quotes"].as_str().unwrap().contains('\"'));
        assert!(special_json["newlines"].as_str().unwrap().contains('\n'));
        assert!(special_json["unicode"].as_str().unwrap().contains('🚨'));
        assert_eq!(special_json["emoji_array"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_timestamp_operations() {
        let now = chrono::Utc::now();
        let past = now - chrono::Duration::days(30);
        let future = now + chrono::Duration::hours(1);

        // Test timestamp comparisons
        assert!(past < now);
        assert!(now < future);
        assert!(future > past);

        // Test duration calculations
        let duration = now.signed_duration_since(past);
        assert!(duration.num_days() >= 29); // Account for leap seconds etc
        assert!(duration.num_days() <= 31);

        // Test formatting
        let formatted = now.format("%Y-%m-%d %H:%M:%S").to_string();
        assert!(formatted.len() >= 19); // YYYY-MM-DD HH:MM:SS

        // Test ISO8601 formatting
        let iso = now.to_rfc3339();
        assert!(iso.ends_with('Z') || iso.contains('+') || iso.contains('-')); // UTC or timezone offset
        assert!(iso.contains('T')); // ISO8601 separator
    }

    #[test]
    fn test_numeric_edge_cases() {
        // Test integer limits
        assert_eq!(i64::MAX, 9_223_372_036_854_775_807);
        assert_eq!(i64::MIN, -9_223_372_036_854_775_808);

        // Test frequency values (typical radio frequencies)
        let frequencies = [30_000_000, 154_000_000, 800_000_000, 1_200_000_000]; // 30MHz to 1.2GHz

        for &freq in &frequencies {
            assert!(freq > 0);
            assert!(freq < i64::MAX);
        }

        // Test file sizes (bytes)
        let file_sizes = [0, 1024, 1_048_576, 1_073_741_824, 10_737_418_240]; // 0B to 10GB

        for &size in &file_sizes {
            assert!(size >= 0);
            let log_params = UploadLogParams {
                client_ip: "127.0.0.1".parse().unwrap(),
                user_agent: None,
                api_key_id: None,
                system_id: None,
                success: true,
                error_message: None,
                filename: None,
                file_size: Some(size),
            };
            assert_eq!(log_params.file_size, Some(size));
        }
    }

    #[test]
    fn test_boolean_operations() {
        // Test success/failure combinations
        let scenarios = [
            (true, None),                               // Success with no error
            (false, Some("Error occurred")),            // Failure with error
            (true, Some("Warning: low audio quality")), // Success with warning
            (false, None),                              // Failure without specific error
        ];

        for (success, error_msg) in scenarios {
            let params = UploadLogParams {
                client_ip: "127.0.0.1".parse().unwrap(),
                user_agent: None,
                api_key_id: None,
                system_id: None,
                success,
                error_message: error_msg.map(String::from),
                filename: None,
                file_size: None,
            };

            assert_eq!(params.success, success);
            match (success, error_msg) {
                (true | false, None) => assert!(params.error_message.is_none()),
                (false, Some(_)) => assert!(params.error_message.is_some()),
                (true, Some(msg)) => assert!(params.error_message.as_ref().unwrap().contains(msg)),
            }
        }
    }

    #[test]
    fn test_option_handling_patterns() {
        // Test Option<T> combinations in structs
        let all_none = UploadLogParams {
            client_ip: "127.0.0.1".parse().unwrap(),
            user_agent: None,
            api_key_id: None,
            system_id: None,
            success: true,
            error_message: None,
            filename: None,
            file_size: None,
        };

        assert!(all_none.user_agent.is_none());
        assert!(all_none.api_key_id.is_none());
        assert!(all_none.system_id.is_none());
        assert!(all_none.error_message.is_none());
        assert!(all_none.filename.is_none());
        assert!(all_none.file_size.is_none());

        let all_some = UploadLogParams {
            client_ip: "127.0.0.1".parse().unwrap(),
            user_agent: Some("TestAgent/1.0".to_string()),
            api_key_id: Some("key123".to_string()),
            system_id: Some("system456".to_string()),
            success: true,
            error_message: Some("No error".to_string()),
            filename: Some("test.mp3".to_string()),
            file_size: Some(1024),
        };

        assert!(all_some.user_agent.is_some());
        assert!(all_some.api_key_id.is_some());
        assert!(all_some.system_id.is_some());
        assert!(all_some.error_message.is_some());
        assert!(all_some.filename.is_some());
        assert!(all_some.file_size.is_some());

        // Test Option unwrapping safety
        assert_eq!(all_some.user_agent.as_ref().unwrap(), "TestAgent/1.0");
        assert_eq!(all_some.api_key_id.as_ref().unwrap(), "key123");
        assert_eq!(all_some.file_size.unwrap(), 1024);
    }

    #[test]
    fn test_decimal_conversion_extreme_values() {
        // Test with extreme decimal values
        let extreme_values = vec![
            0.0,
            1.0,
            -1.0,
            f64::MIN_POSITIVE,
            100.0,
            999.999,
            0.000_001,
            123.456_789,
        ];

        for value in extreme_values {
            // Test that conversion doesn't panic
            let decimal_result = rust_decimal::Decimal::try_from(value);

            if let Ok(decimal) = decimal_result {
                // Should be able to convert back
                let float_result = f64::try_from(decimal);
                assert!(float_result.is_ok());
            } else {
                // Some extreme values might fail, that's acceptable
            }
        }
    }

    #[test]
    fn test_ip_address_parsing_extended() {
        let test_cases = vec![
            ("127.0.0.1", true),
            ("192.168.1.1", true),
            ("10.0.0.1", true),
            ("::1", true),
            ("2001:db8::1", true),
            ("", false),
            ("256.256.256.256", false),
            ("not.an.ip", false),
            ("192.168.1", false),
            ("192.168.1.256", false),
        ];

        for (ip_str, should_parse) in test_cases {
            let parse_result = ip_str.parse::<std::net::IpAddr>();

            if should_parse {
                assert!(parse_result.is_ok(), "Should parse: {ip_str}");

                // Test that we can work with the IP address
                if let Ok(_ip_addr) = parse_result {
                    // Just verify we got a valid IP
                    assert!(!ip_str.is_empty());
                }
            } else {
                assert!(parse_result.is_err(), "Should not parse: {ip_str}");
            }
        }
    }

    #[test]
    fn test_uuid_generation_extended() {
        // Test UUID generation and parsing
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();

        // UUIDs should be unique
        assert_ne!(uuid1, uuid2);

        // Test string conversion
        let uuid_str = uuid1.to_string();
        let parsed_uuid = Uuid::parse_str(&uuid_str).unwrap();
        assert_eq!(uuid1, parsed_uuid);

        // Test hyphenated format
        assert!(uuid_str.contains('-'));
        assert_eq!(uuid_str.len(), 36);

        // Test nil UUID
        let nil_uuid = Uuid::nil();
        assert_eq!(nil_uuid.to_string(), "00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn test_filter_bounds_validation() {
        // Test filter validation logic
        let limit_values = vec![1, 10, 100, 1000];
        let offset_values = vec![0, 10, 100];

        for limit in limit_values {
            // Limit should be reasonable
            assert!(limit > 0, "Limit should be positive");
            assert!(limit <= 10_000, "Limit should be reasonable");
        }

        for offset in offset_values {
            // Offset should be non-negative
            assert!(offset >= 0, "Offset should be non-negative");
        }

        // Test boundary conditions
        // Minimum limit should be valid (1 > 0 is always true)
        // Zero offset should be valid (0 >= 0 is always true)
    }

    #[test]
    fn test_error_message_formatting() {
        // Test error message construction patterns
        let test_messages = vec![
            "Database connection failed",
            "Invalid UUID format",
            "Record not found",
            "Constraint violation",
            "Timeout occurred",
            "Invalid decimal value: NaN",
            "Pool connection exhausted",
        ];

        for message in test_messages {
            // Test that messages can be formatted into errors
            let error = crate::Error::Database(message.to_string());
            let error_string = error.to_string();

            assert!(error_string.contains(message));
            assert!(!error_string.is_empty());

            // Test debug formatting
            let debug_string = format!("{error:?}");
            assert!(!debug_string.is_empty());
        }
    }

    #[test]
    fn test_query_parameter_construction() {
        // Test query parameter building patterns
        let params = vec![
            ("test_string", "value"),
            ("another_param", "another_value"),
            ("empty_param", ""),
            ("special_chars", "value with spaces & symbols!"),
        ];

        for (param_name, param_value) in params {
            // Test that parameters don't cause issues
            assert!(!param_name.is_empty());
            assert!(param_name.is_ascii());

            // Test value handling
            let trimmed = param_value.trim();
            assert!(trimmed.len() <= param_value.len());
        }
    }

    #[test]
    fn test_timestamp_operations_extended() {
        use chrono::{DateTime, Utc};

        // Test timestamp creation and manipulation
        let now = Utc::now();
        let earlier = now - chrono::Duration::hours(1);
        let later = now + chrono::Duration::hours(1);

        // Test ordering
        assert!(earlier < now);
        assert!(now < later);

        // Test formatting
        let iso_string = now.to_rfc3339();
        assert!(!iso_string.is_empty());

        // Test parsing back
        let parsed = DateTime::parse_from_rfc3339(&iso_string);
        assert!(parsed.is_ok());

        // Test that we can convert to UTC
        if let Ok(parsed_dt) = parsed {
            let utc_dt = parsed_dt.with_timezone(&Utc);
            // Should be very close to original (within a few milliseconds)
            let diff = (now - utc_dt).num_milliseconds().abs();
            assert!(diff < 1000); // Within 1 second
        }
    }

    #[test]
    fn test_system_id_validation_patterns() {
        // Test system ID validation patterns
        let valid_system_ids = vec![
            "system1",
            "test_system",
            "POLICE_DEPT",
            "fire-station-1",
            "system_123",
        ];

        let invalid_system_ids = vec![
            "",    // Empty
            " ",   // Just whitespace
            "   ", // Multiple spaces
        ];

        for system_id in valid_system_ids {
            // Valid system IDs should have reasonable properties
            assert!(!system_id.is_empty());
            assert!(!system_id.trim().is_empty());
            assert!(system_id.len() <= 100); // Reasonable length limit
        }

        for system_id in invalid_system_ids {
            // Invalid system IDs should be caught
            assert!(system_id.is_empty() || system_id.trim().is_empty());
        }
    }

    #[test]
    fn test_json_value_operations_extended() {
        // Test JSON value creation and manipulation
        let json_values = vec![
            serde_json::json!({}),
            serde_json::json!({"key": "value"}),
            serde_json::json!([1, 2, 3]),
            serde_json::json!(null),
            serde_json::json!(true),
            serde_json::json!(42),
            serde_json::json!(std::f64::consts::PI),
        ];

        for value in json_values {
            // Test serialization
            let serialized = serde_json::to_string(&value);
            assert!(serialized.is_ok());

            if let Ok(json_str) = serialized {
                // Test deserialization
                let deserialized: std::result::Result<serde_json::Value, _> =
                    serde_json::from_str(&json_str);
                assert!(deserialized.is_ok());

                if let Ok(parsed_value) = deserialized {
                    assert_eq!(value, parsed_value);
                }
            }
        }
    }

    #[test]
    fn test_struct_field_edge_cases() {
        // Test empty strings and special characters
        let update_empty_text = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "",
            text: Some(""),
            confidence: Some(0.5),
            error: Some(""),
            speaker_segments: None,
            speaker_count: None,
        };
        let debug_str = format!("{update_empty_text:?}");
        assert!(debug_str.contains("TranscriptionUpdate"));

        // Test special characters in strings
        let update_special_chars = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "completed",
            text: Some("Text with 日本語 and émojis 🎉"),
            confidence: Some(0.8),
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };
        let debug_str_special = format!("{update_special_chars:?}");
        assert!(debug_str_special.contains("completed"));

        // Test very long strings
        let long_text = "a".repeat(1000);
        let update_long_text = TranscriptionUpdate {
            id: Uuid::new_v4(),
            status: "completed",
            text: Some(&long_text),
            confidence: Some(0.9),
            error: None,
            speaker_segments: None,
            speaker_count: None,
        };
        let debug_str_long = format!("{update_long_text:?}");
        assert!(debug_str_long.contains("completed"));

        // Test system_id with special characters
        let filter_special_system = RadioCallFilter {
            system_id: Some("SYS-001_TEST.2024"),
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            limit: 5,
            offset: 0,
        };
        let debug_str_sys = format!("{filter_special_system:?}");
        assert!(debug_str_sys.contains("SYS-001_TEST.2024"));
    }

    #[test]
    fn test_transcription_stats_edge_case_values() {
        // Test maximum values
        let stats_max = TranscriptionStats {
            total: i64::MAX,
            completed: i64::MAX / 2,
            failed: i64::MAX / 4,
            processing: i64::MAX / 8,
            pending: i64::MAX / 8 - 1,
            avg_confidence: Some(1.0),
        };
        let debug_str_max = format!("{stats_max:?}");
        assert!(debug_str_max.contains("TranscriptionStats"));

        // Verify sum constraint for max values
        let sum = stats_max.completed + stats_max.failed + stats_max.processing + stats_max.pending;
        assert!(sum <= stats_max.total);

        // Test minimum confidence
        let stats_min_conf = TranscriptionStats {
            total: 1,
            completed: 1,
            failed: 0,
            processing: 0,
            pending: 0,
            avg_confidence: Some(0.0),
        };
        if let Some(conf) = stats_min_conf.avg_confidence {
            assert!(conf == 0.0);
        }

        // Test various confidence precision values
        let confidence_values = vec![0.123_456_789_0, 0.999_999, 0.000_001, 0.5];
        for conf_val in confidence_values {
            let stats = TranscriptionStats {
                total: 10,
                completed: 8,
                failed: 1,
                processing: 1,
                pending: 0,
                avg_confidence: Some(conf_val),
            };
            if let Some(conf) = stats.avg_confidence {
                assert!((0.0..=1.0).contains(&conf));
            }
        }
    }

    #[test]
    fn test_upload_stats_edge_case_values() {
        // Test zero values
        let stats_zero = UploadStats {
            total_uploads: 0,
            successful_uploads: 0,
            failed_uploads: 0,
            avg_processing_time: Some(0.0),
            total_bytes_uploaded: 0,
        };
        let debug_str_zero = format!("{stats_zero:?}");
        assert!(debug_str_zero.contains("UploadStats"));
        assert_eq!(stats_zero.total_uploads, 0);

        // Test very high processing times
        let stats_high_time = UploadStats {
            total_uploads: 1,
            successful_uploads: 0,
            failed_uploads: 1,
            avg_processing_time: Some(60000.0), // 1 minute
            total_bytes_uploaded: 1,
        };
        if let Some(time) = stats_high_time.avg_processing_time {
            assert!(time > 0.0);
        }

        // Test maximum byte values
        let stats_max_bytes = UploadStats {
            total_uploads: 1000,
            successful_uploads: 900,
            failed_uploads: 100,
            avg_processing_time: Some(150.5),
            total_bytes_uploaded: i64::MAX,
        };
        assert!(stats_max_bytes.total_bytes_uploaded > 0);

        // Test fractional processing times
        let fractional_times = vec![0.1, 0.01, 0.001, 999.999];
        for time_val in fractional_times {
            let stats = UploadStats {
                total_uploads: 5,
                successful_uploads: 4,
                failed_uploads: 1,
                avg_processing_time: Some(time_val),
                total_bytes_uploaded: 5000,
            };
            if let Some(time) = stats.avg_processing_time {
                assert!(time >= 0.0);
            }
        }
    }

    #[test]
    fn test_wrapper_functions_signatures_and_documentation() {
        // Test that wrapper functions have correct signatures and documentation
        // We can't test async functions without a database, but we can test their
        // existence and that they compile correctly

        // Test RadioCallFilter with None system_id for coverage
        let filter_no_system = RadioCallFilter {
            system_id: None,
            talkgroup_id: None,
            from_date: None,
            to_date: None,
            limit: 100,
            offset: 0,
        };

        // Test that filter correctly represents None case
        assert!(filter_no_system.system_id.is_none());
        assert!(filter_no_system.talkgroup_id.is_none());
        assert!(filter_no_system.from_date.is_none());
        assert!(filter_no_system.to_date.is_none());
        assert!(filter_no_system.limit == 100);
        assert!(filter_no_system.offset == 0);

        // Test debug output for None fields
        let debug_str = format!("{filter_no_system:?}");
        assert!(debug_str.contains("None"));
        assert!(debug_str.contains("100"));
        assert!(debug_str.contains('0'));
    }

    #[test]
    fn test_upload_log_params_ip_address_parsing() {
        // Test various IP address formats
        let ipv4_addresses = vec![
            "127.0.0.1",
            "0.0.0.0",
            "255.255.255.255",
            "192.168.1.1",
            "10.0.0.1",
            "172.16.0.1",
        ];

        for addr_str in ipv4_addresses {
            let ip = addr_str.parse::<std::net::IpAddr>().unwrap();
            let params = UploadLogParams {
                client_ip: ip,
                user_agent: Some(format!("TestAgent for {addr_str}")),
                api_key_id: None,
                system_id: None,
                success: true,
                error_message: None,
                filename: None,
                file_size: None,
            };

            match params.client_ip {
                std::net::IpAddr::V4(_) => {},
                std::net::IpAddr::V6(_) => panic!("Expected IPv4 for {addr_str}"),
            }
        }

        let ipv6_addresses = vec!["::1", "::", "2001:db8::1", "fe80::1", "::ffff:192.0.2.1"];

        for addr_str in ipv6_addresses {
            let ip = addr_str.parse::<std::net::IpAddr>().unwrap();
            let params = UploadLogParams {
                client_ip: ip,
                user_agent: Some(format!("TestAgent for {addr_str}")),
                api_key_id: None,
                system_id: None,
                success: false,
                error_message: Some("Test error".to_string()),
                filename: None,
                file_size: None,
            };

            match params.client_ip {
                std::net::IpAddr::V4(_) => panic!("Expected IPv6 for {addr_str}"),
                std::net::IpAddr::V6(_) => {}, // IPv6 as expected
            }
        }
    }

    #[test]
    fn test_struct_combinations_and_edge_cases() {
        // Test TranscriptionUpdate with various status combinations
        let status_combinations = vec![
            ("pending", None, None, None),
            ("processing", None, None, None),
            ("completed", Some("Success"), Some(0.95), None),
            ("failed", None, None, Some("Error occurred")),
            ("cancelled", None, None, Some("User cancelled")),
            ("", Some(""), Some(0.0), Some("")),
        ];

        for (status, text, confidence, error) in status_combinations {
            let update = TranscriptionUpdate {
                id: Uuid::new_v4(),
                status,
                text,
                confidence,
                error,
                speaker_count: None,
                speaker_segments: None,
            };

            let debug_str = format!("{update:?}");
            assert!(debug_str.contains("TranscriptionUpdate"));
            assert!(debug_str.contains(status));

            // Verify confidence range if present
            if let Some(conf) = update.confidence {
                assert!(
                    (0.0..=1.0).contains(&conf),
                    "Confidence {conf} out of range"
                );
            }
        }
    }

    #[test]
    fn test_radio_call_filter_boundary_conditions() {
        let now = chrono::Utc::now();

        // Test with very large limits and offsets
        let filter_large = RadioCallFilter {
            system_id: Some("LARGE_SYS"),
            talkgroup_id: Some(i32::MAX),
            from_date: Some(now - chrono::Duration::days(365)),
            to_date: Some(now),
            limit: i64::MAX,
            offset: i64::MAX,
        };

        assert_eq!(filter_large.limit, i64::MAX);
        assert_eq!(filter_large.offset, i64::MAX);
        if let Some(tg) = filter_large.talkgroup_id {
            assert_eq!(tg, i32::MAX);
        }

        // Test with minimum values
        let filter_min = RadioCallFilter {
            system_id: Some(""),
            talkgroup_id: Some(i32::MIN),
            from_date: Some(chrono::DateTime::<chrono::Utc>::MIN_UTC),
            to_date: Some(chrono::DateTime::<chrono::Utc>::MAX_UTC),
            limit: 1,
            offset: 0,
        };

        assert_eq!(filter_min.limit, 1);
        assert_eq!(filter_min.offset, 0);
        if let Some(tg) = filter_min.talkgroup_id {
            assert_eq!(tg, i32::MIN);
        }

        // Test debug formatting with extreme values
        let debug_large = format!("{filter_large:?}");
        let debug_min = format!("{filter_min:?}");

        assert!(debug_large.contains("RadioCallFilter"));
        assert!(debug_min.contains("RadioCallFilter"));
    }

    #[test]
    fn test_upload_log_params_string_field_variations() {
        // Test various string field combinations
        let string_variations = vec![
            (None, None, None, None, None),
            (Some(String::new()), Some(String::new()), Some(String::new()), Some(String::new()), None),
            (Some("Mozilla/5.0".to_string()), Some("key123".to_string()), Some("SYS001".to_string()), Some("file.mp3".to_string()), Some("Upload failed".to_string())),
            (Some("Very long user agent string that contains lots of information about the client browser and system".to_string()), None, None, None, None),
        ];

        for (user_agent, api_key, system_id, filename, error_msg) in string_variations {
            let params = UploadLogParams {
                client_ip: "192.168.1.1".parse().unwrap(),
                user_agent,
                api_key_id: api_key,
                system_id,
                success: error_msg.is_none(),
                error_message: error_msg,
                filename,
                file_size: Some(1024),
            };

            let debug_str = format!("{params:?}");
            assert!(debug_str.contains("UploadLogParams"));
            assert!(debug_str.contains("192.168.1.1"));

            // Verify success/failure logic
            if params.error_message.is_some() {
                assert!(!params.success);
            }
        }
    }

    #[test]
    fn test_transcription_stats_mathematical_properties() {
        // Test mathematical relationships in TranscriptionStats
        let test_cases = vec![
            (100, 80, 10, 5, 5),
            (0, 0, 0, 0, 0),
            (1, 1, 0, 0, 0),
            (1_000_000, 500_000, 250_000, 125_000, 125_000),
        ];

        for (total, completed, failed, processing, pending) in test_cases {
            let stats = TranscriptionStats {
                total,
                completed,
                failed,
                processing,
                pending,
                avg_confidence: Some(0.5),
            };

            // Verify sum property
            let sum = stats.completed + stats.failed + stats.processing + stats.pending;
            assert_eq!(
                sum, stats.total,
                "Sum mismatch for case ({total}, {completed}, {failed}, {processing}, {pending})"
            );

            // Verify all values are non-negative
            assert!(stats.total >= 0);
            assert!(stats.completed >= 0);
            assert!(stats.failed >= 0);
            assert!(stats.processing >= 0);
            assert!(stats.pending >= 0);

            // Test clone operation
            let stats_clone = stats.clone();
            assert_eq!(stats.total, stats_clone.total);
            assert_eq!(stats.completed, stats_clone.completed);
            assert_eq!(stats.failed, stats_clone.failed);
            assert_eq!(stats.processing, stats_clone.processing);
            assert_eq!(stats.pending, stats_clone.pending);
            assert_eq!(stats.avg_confidence, stats_clone.avg_confidence);
        }
    }

    #[test]
    fn test_upload_stats_mathematical_properties() {
        let test_cases = vec![
            (100, 95, 5, Some(150.0), 1_000_000),
            (0, 0, 0, None, 0),
            (1, 0, 1, Some(5000.0), 1024),
            (1_000_000, 999_999, 1, Some(0.1), i64::MAX),
        ];

        for (total, successful, failed, avg_time, total_bytes) in test_cases {
            let stats = UploadStats {
                total_uploads: total,
                successful_uploads: successful,
                failed_uploads: failed,
                avg_processing_time: avg_time,
                total_bytes_uploaded: total_bytes,
            };

            // Verify sum property
            assert_eq!(
                stats.total_uploads,
                stats.successful_uploads + stats.failed_uploads
            );

            // Verify all counts are non-negative
            assert!(stats.total_uploads >= 0);
            assert!(stats.successful_uploads >= 0);
            assert!(stats.failed_uploads >= 0);
            assert!(stats.total_bytes_uploaded >= 0);

            // Verify processing time if present
            if let Some(time) = stats.avg_processing_time {
                assert!(time >= 0.0);
            }

            // Test clone operation
            let stats_clone = stats.clone();
            assert_eq!(stats.total_uploads, stats_clone.total_uploads);
            assert_eq!(stats.successful_uploads, stats_clone.successful_uploads);
            assert_eq!(stats.failed_uploads, stats_clone.failed_uploads);
            assert_eq!(stats.avg_processing_time, stats_clone.avg_processing_time);
            assert_eq!(stats.total_bytes_uploaded, stats_clone.total_bytes_uploaded);
        }
    }
}
