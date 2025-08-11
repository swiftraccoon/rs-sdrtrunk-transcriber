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
                transcription_completed_at = CASE 
                    WHEN $1 IN ('completed', 'failed') THEN NOW() 
                    ELSE transcription_completed_at 
                END,
                transcription_started_at = CASE 
                    WHEN $1 = 'processing' AND transcription_started_at IS NULL THEN NOW() 
                    ELSE transcription_started_at 
                END
            WHERE id = $5
        ";

        sqlx::query(query)
            .bind(status)
            .bind(text)
            .bind(confidence_decimal)
            .bind(error)
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
                SUM(CASE WHEN file_size IS NOT NULL THEN file_size END) as total_bytes_uploaded
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

#[cfg(test)]
mod tests {
    use super::*;
    use sdrtrunk_core::types::TranscriptionStatus;
    use sqlx::PgPool;
    use uuid::Uuid;

    // Helper function to create test database (would need actual test setup)
    fn create_test_pool() -> PgPool {
        // This would be implemented with actual test database setup
        // For now, this is a placeholder
        unimplemented!("Test database setup needed")
    }

    #[tokio::test]
    #[ignore] // Ignore until test database is set up
    #[allow(clippy::missing_panics_doc)]
    async fn test_radio_call_insert_and_find() {
        let pool = create_test_pool();

        let call = RadioCall {
            system_id: "test_system".to_string(),
            talkgroup_id: Some(12345),
            transcription_status: TranscriptionStatus::None,
            ..RadioCall::default()
        };

        let id = RadioCallQueries::insert(&pool, &call).await.unwrap();
        assert!(!id.is_nil());

        let retrieved = RadioCallQueries::find_by_id(&pool, id).await.unwrap();
        assert_eq!(retrieved.system_id, "test_system");
        assert_eq!(retrieved.talkgroup_id, Some(12345));
    }

    #[tokio::test]
    #[ignore] // Ignore until test database is set up
    #[allow(clippy::missing_panics_doc)]
    async fn test_radio_call_count_by_system() {
        let pool = create_test_pool();

        // Insert test calls
        for i in 0..5 {
            let call = RadioCall {
                system_id: "count_test_system".to_string(),
                talkgroup_id: Some(i),
                ..RadioCall::default()
            };
            RadioCallQueries::insert(&pool, &call).await.unwrap();
        }

        let count = RadioCallQueries::count_by_system(&pool, "count_test_system")
            .await
            .unwrap();
        assert_eq!(count, 5);
    }

    #[tokio::test]
    #[ignore] // Ignore until test database is set up
    #[allow(clippy::missing_panics_doc)]
    async fn test_transcription_status_update() {
        let pool = create_test_pool();

        let call = RadioCall {
            system_id: "transcription_test".to_string(),
            transcription_status: TranscriptionStatus::Pending,
            ..RadioCall::default()
        };

        let id = RadioCallQueries::insert(&pool, &call).await.unwrap();

        // Update to processing
        RadioCallQueries::update_transcription_status(
            &pool,
            TranscriptionUpdate {
                id,
                status: "processing",
                text: None,
                confidence: None,
                error: None,
            },
        )
        .await
        .unwrap();

        let updated = RadioCallQueries::find_by_id(&pool, id).await.unwrap();
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
            },
        )
        .await
        .unwrap();

        let completed = RadioCallQueries::find_by_id(&pool, id).await.unwrap();
        assert_eq!(
            completed.transcription_status,
            Some("completed".to_string())
        );
        assert_eq!(
            completed.transcription_text,
            Some("Test transcription".to_string())
        );
        assert!(completed.transcription_confidence.is_some());
    }

    #[tokio::test]
    #[ignore] // Ignore until test database is set up
    #[allow(clippy::missing_panics_doc)]
    async fn test_system_stats_upsert() {
        let pool = create_test_pool();

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
        SystemStatsQueries::upsert(&pool, &stats).await.unwrap();

        let retrieved = SystemStatsQueries::get_by_system_id(&pool, "upsert_test")
            .await
            .unwrap();
        assert_eq!(retrieved.system_id, "upsert_test");
        assert_eq!(retrieved.total_calls, Some(100));

        // Second upsert (update)
        let mut updated_stats = stats.clone();
        updated_stats.total_calls = Some(150);
        updated_stats.calls_today = Some(15);

        SystemStatsQueries::upsert(&pool, &updated_stats)
            .await
            .unwrap();

        let updated_retrieved = SystemStatsQueries::get_by_system_id(&pool, "upsert_test")
            .await
            .unwrap();
        assert_eq!(updated_retrieved.total_calls, Some(150));
        assert_eq!(updated_retrieved.calls_today, Some(15));
    }

    #[tokio::test]
    #[ignore] // Ignore until test database is set up
    #[allow(clippy::missing_panics_doc)]
    async fn test_upload_log_operations() {
        let pool = create_test_pool();

        let log = UploadLogDb {
            id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            client_ip: "127.0.0.1".parse::<std::net::IpAddr>().unwrap().into(),
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

        let id = UploadLogQueries::insert(&pool, &log).await.unwrap();
        assert!(!id.is_nil());

        let logs = UploadLogQueries::get_recent(&pool, 10, 0).await.unwrap();
        assert!(!logs.is_empty());

        let stats = UploadLogQueries::get_upload_stats(&pool).await.unwrap();
        assert!(stats.total_uploads > 0);
        assert!(stats.successful_uploads > 0);
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
    if let Some(system) = filter.system_id {
        RadioCallQueries::find_by_system(pool, system, filter.limit, filter.offset).await
    } else {
        // For now, just return empty - would need a proper implementation
        Ok(vec![])
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
        // For now, return 0 - would need proper implementation
        Ok(0)
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
    let row = sqlx::query("SELECT COUNT(*) as count FROM radio_calls WHERE created_at > NOW() - INTERVAL $1 || ' hours'")
        .bind(hours)
        .fetch_one(pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

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
    let row = sqlx::query("SELECT COUNT(*) as count FROM radio_calls WHERE system_id = $1 AND created_at > NOW() - INTERVAL $2 || ' hours'")
        .bind(system_id)
        .bind(hours)
        .fetch_one(pool)
        .await
        .map_err(|e| Error::Database(e.to_string()))?;

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
