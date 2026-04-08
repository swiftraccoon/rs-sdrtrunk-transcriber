//! PostgreSQL-based transcription job queue operations.
//!
//! Provides a durable, distributed job queue backed by `PostgreSQL` with
//! `SELECT ... FOR UPDATE SKIP LOCKED` semantics for safe concurrent worker
//! claiming. Supports priority ordering, heartbeat-based liveness detection,
//! automatic retry with back-off, and stale-job reclamation.

use crate::error::StorageError;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Row};
use uuid::Uuid;

/// Result type alias for job queue operations.
type Result<T> = std::result::Result<T, StorageError>;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Parameters for enqueuing a new transcription job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnqueueParams {
    /// The radio call to transcribe.
    pub call_id: Uuid,
    /// Filesystem path to the audio file (optional if `audio_data` is provided).
    pub audio_path: Option<String>,
    /// Raw audio bytes (MP3). Stored in DB so workers don't need shared filesystem.
    pub audio_data: Option<Vec<u8>>,
    /// Job priority (higher values are claimed first).
    pub priority: i32,
    /// Arbitrary transcription options forwarded to the worker.
    pub options: serde_json::Value,
    /// Maximum seconds the job may run before it is considered stale.
    pub timeout_seconds: i32,
}

/// A row from the `transcription_jobs` table.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TranscriptionJob {
    /// Unique job identifier.
    pub id: Uuid,
    /// Associated radio call.
    pub call_id: Uuid,
    /// Current status (`pending`, `processing`, `completed`, `failed`).
    pub status: String,
    /// Identifier of the worker that claimed the job.
    pub worker_id: Option<String>,
    /// Timestamp when the job was claimed.
    pub claimed_at: Option<DateTime<Utc>>,
    /// Last heartbeat from the owning worker.
    pub heartbeat_at: Option<DateTime<Utc>>,
    /// Filesystem path to the audio file.
    pub audio_path: Option<String>,
    /// Raw audio bytes (MP3).
    pub audio_data: Option<Vec<u8>>,
    /// Job priority (higher values claimed first).
    pub priority: i32,
    /// How many times the job has been retried.
    pub retry_count: i32,
    /// Maximum allowed retries before permanent failure.
    pub max_retries: i32,
    /// Arbitrary transcription options.
    pub options: serde_json::Value,
    /// Transcription result text.
    pub result_text: Option<String>,
    /// Transcription confidence score.
    pub result_confidence: Option<Decimal>,
    /// Detected language.
    pub result_language: Option<String>,
    /// Speaker diarization segments.
    pub result_speaker_segments: Option<serde_json::Value>,
    /// Number of distinct speakers detected.
    pub result_speaker_count: Option<i32>,
    /// Error message if the job failed.
    pub result_error: Option<String>,
    /// Wall-clock processing time in milliseconds.
    pub processing_time_ms: Option<i64>,
    /// Row creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Timestamp when processing started.
    pub started_at: Option<DateTime<Utc>>,
    /// Timestamp when the job finished (success or permanent failure).
    pub completed_at: Option<DateTime<Utc>>,
    /// Maximum seconds the job may run.
    pub timeout_seconds: i32,
}

/// Outcome of a transcription run, used when completing a job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    /// Transcription text.
    pub text: Option<String>,
    /// Confidence score.
    pub confidence: Option<f32>,
    /// Detected language code.
    pub language: Option<String>,
    /// Speaker diarization segments.
    pub speaker_segments: Option<serde_json::Value>,
    /// Number of distinct speakers.
    pub speaker_count: Option<i32>,
    /// Error message (when the attempt partially succeeded but had issues).
    pub error: Option<String>,
    /// Wall-clock processing time in milliseconds.
    pub processing_time_ms: i64,
}

/// Aggregate statistics for the job queue.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct QueueStats {
    /// Number of jobs waiting to be claimed.
    pub pending: i64,
    /// Number of jobs currently being processed.
    pub processing: i64,
    /// Number of successfully completed jobs.
    pub completed: i64,
    /// Number of permanently failed jobs.
    pub failed: i64,
    /// Total number of jobs across all statuses.
    pub total: i64,
}

// ---------------------------------------------------------------------------
// Job queue operations
// ---------------------------------------------------------------------------

/// PostgreSQL-backed transcription job queue.
///
/// All methods are stateless — they accept a [`PgPool`] reference and operate
/// directly against the `transcription_jobs` table.
#[derive(Debug)]
pub struct JobQueue;

impl JobQueue {
    /// Enqueue a new transcription job.
    ///
    /// Inserts a row into `transcription_jobs` with status `pending` and
    /// returns the generated job id.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if the INSERT fails (e.g. foreign-key
    /// violation on `call_id`).
    pub async fn enqueue(pool: &PgPool, params: &EnqueueParams) -> Result<Uuid> {
        let row = sqlx::query(
            r"
            INSERT INTO transcription_jobs (call_id, audio_path, audio_data, priority, options, timeout_seconds)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id
            ",
        )
        .bind(params.call_id)
        .bind(&params.audio_path)
        .bind(params.audio_data.as_deref())
        .bind(params.priority)
        .bind(&params.options)
        .bind(params.timeout_seconds)
        .fetch_one(pool)
        .await?;

        let id: Uuid = row.get("id");
        Ok(id)
    }

    /// Atomically claim the highest-priority pending job for a worker.
    ///
    /// Uses `SELECT ... FOR UPDATE SKIP LOCKED` so that multiple workers can
    /// safely race for jobs without blocking each other.
    ///
    /// Returns `None` when no claimable jobs exist.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if the query fails.
    pub async fn claim(pool: &PgPool, worker_id: &str) -> Result<Option<TranscriptionJob>> {
        let job = sqlx::query_as::<_, TranscriptionJob>(
            r"
            UPDATE transcription_jobs
            SET status       = 'processing',
                worker_id    = $1,
                claimed_at   = NOW(),
                heartbeat_at = NOW(),
                started_at   = NOW()
            WHERE id = (
                SELECT id
                FROM transcription_jobs
                WHERE status = 'pending'
                ORDER BY priority DESC, created_at ASC
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING *
            ",
        )
        .bind(worker_id)
        .fetch_optional(pool)
        .await?;

        Ok(job)
    }

    /// Update the heartbeat timestamp for an in-progress job.
    ///
    /// Only succeeds when the job is still in `processing` status and owned by
    /// the given `worker_id`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if no matching row is updated, or
    /// another [`StorageError`] on query failure.
    pub async fn heartbeat(pool: &PgPool, job_id: Uuid, worker_id: &str) -> Result<()> {
        let result = sqlx::query(
            r"
            UPDATE transcription_jobs
            SET heartbeat_at = NOW()
            WHERE id = $1
              AND worker_id = $2
              AND status = 'processing'
            ",
        )
        .bind(job_id)
        .bind(worker_id)
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound {
                entity: "transcription_job".into(),
                id: job_id.to_string(),
            });
        }

        Ok(())
    }

    /// Mark a job as successfully completed and store the results.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if no matching row exists, or
    /// another [`StorageError`] on query failure.
    pub async fn complete(pool: &PgPool, job_id: Uuid, result: &JobResult) -> Result<()> {
        let confidence = result
            .confidence
            .map(|c| Decimal::try_from(f64::from(c)))
            .transpose()
            .map_err(|e| StorageError::Serialization(format!("invalid confidence value: {e}")))?;

        let res = sqlx::query(
            r"
            UPDATE transcription_jobs
            SET status                 = 'completed',
                result_text            = $1,
                result_confidence      = $2,
                result_language        = $3,
                result_speaker_segments = $4,
                result_speaker_count   = $5,
                result_error           = $6,
                processing_time_ms     = $7,
                completed_at           = NOW()
            WHERE id = $8
              AND status = 'processing'
            ",
        )
        .bind(&result.text)
        .bind(confidence)
        .bind(&result.language)
        .bind(&result.speaker_segments)
        .bind(result.speaker_count)
        .bind(&result.error)
        .bind(result.processing_time_ms)
        .bind(job_id)
        .execute(pool)
        .await?;

        if res.rows_affected() == 0 {
            return Err(StorageError::NotFound {
                entity: "transcription_job".into(),
                id: job_id.to_string(),
            });
        }

        Ok(())
    }

    /// Record a job failure.
    ///
    /// If the job still has retries remaining (`retry_count < max_retries`),
    /// it is reset to `pending` with an incremented retry count so it can be
    /// picked up again. Otherwise it is marked `failed` permanently.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] if no matching row exists, or
    /// another [`StorageError`] on query failure.
    pub async fn fail(pool: &PgPool, job_id: Uuid, error: &str) -> Result<()> {
        let res = sqlx::query(
            r"
            UPDATE transcription_jobs
            SET status       = CASE
                                   WHEN retry_count < max_retries - 1 THEN 'pending'
                                   ELSE 'failed'
                               END,
                retry_count  = retry_count + 1,
                result_error = $1,
                worker_id    = CASE
                                   WHEN retry_count < max_retries - 1 THEN NULL
                                   ELSE worker_id
                               END,
                claimed_at   = CASE
                                   WHEN retry_count < max_retries - 1 THEN NULL
                                   ELSE claimed_at
                               END,
                heartbeat_at = CASE
                                   WHEN retry_count < max_retries - 1 THEN NULL
                                   ELSE heartbeat_at
                               END,
                completed_at = CASE
                                   WHEN retry_count < max_retries - 1 THEN NULL
                                   ELSE NOW()
                               END
            WHERE id = $2
              AND status = 'processing'
            ",
        )
        .bind(error)
        .bind(job_id)
        .execute(pool)
        .await?;

        if res.rows_affected() == 0 {
            return Err(StorageError::NotFound {
                entity: "transcription_job".into(),
                id: job_id.to_string(),
            });
        }

        Ok(())
    }

    /// Reclaim stale jobs whose heartbeat has exceeded their timeout.
    ///
    /// Processing jobs are reset to `pending` when
    /// `NOW() - heartbeat_at > timeout_seconds`. Returns the number of jobs
    /// reclaimed.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if the query fails.
    pub async fn reclaim_stale(pool: &PgPool) -> Result<i64> {
        let result = sqlx::query(
            r"
            UPDATE transcription_jobs
            SET status       = 'pending',
                worker_id    = NULL,
                claimed_at   = NULL,
                heartbeat_at = NULL,
                started_at   = NULL
            WHERE status = 'processing'
              AND heartbeat_at < NOW() - (timeout_seconds || ' seconds')::INTERVAL
            ",
        )
        .execute(pool)
        .await?;

        Ok(i64::try_from(result.rows_affected()).unwrap_or(0))
    }

    /// Return aggregate counts for each job status.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if the query fails.
    pub async fn stats(pool: &PgPool) -> Result<QueueStats> {
        let stats = sqlx::query_as::<_, QueueStats>(
            r"
            SELECT
                COUNT(*) FILTER (WHERE status = 'pending')    AS pending,
                COUNT(*) FILTER (WHERE status = 'processing') AS processing,
                COUNT(*) FILTER (WHERE status = 'completed')  AS completed,
                COUNT(*) FILTER (WHERE status = 'failed')     AS failed,
                COUNT(*)                                      AS total
            FROM transcription_jobs
            ",
        )
        .fetch_one(pool)
        .await?;

        Ok(stats)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::missing_panics_doc,
    clippy::no_effect_underscore_binding,
    clippy::used_underscore_binding,
    unused_results
)]
mod tests {
    use super::*;

    #[test]
    fn test_enqueue_params_creation() {
        let params = EnqueueParams {
            call_id: Uuid::new_v4(),
            audio_path: Some("/tmp/audio/test.mp3".to_string()),
            audio_data: None,
            priority: 5,
            options: serde_json::json!({"model": "base", "language": "en"}),
            timeout_seconds: 300,
        };

        assert_eq!(params.priority, 5);
        assert_eq!(params.timeout_seconds, 300);
        assert_eq!(params.audio_path.as_deref(), Some("/tmp/audio/test.mp3"));
    }

    #[test]
    fn test_enqueue_params_serialize_roundtrip() {
        let params = EnqueueParams {
            call_id: Uuid::new_v4(),
            audio_path: Some("/tmp/test.mp3".to_string()),
            audio_data: None,
            priority: 0,
            options: serde_json::json!({}),
            timeout_seconds: 60,
        };

        let json = serde_json::to_string(&params).unwrap();
        let restored: EnqueueParams = serde_json::from_str(&json).unwrap();

        assert_eq!(params.call_id, restored.call_id);
        assert_eq!(params.priority, restored.priority);
    }

    #[test]
    fn test_job_result_creation() {
        let result = JobResult {
            text: Some("Hello world".to_string()),
            confidence: Some(0.95),
            language: Some("en".to_string()),
            speaker_segments: Some(serde_json::json!([{"start": 0.0, "end": 1.5, "speaker": "A"}])),
            speaker_count: Some(1),
            error: None,
            processing_time_ms: 1234,
        };

        assert_eq!(result.text.as_deref(), Some("Hello world"));
        assert_eq!(result.processing_time_ms, 1234);
    }

    #[test]
    fn test_job_result_with_error() {
        let result = JobResult {
            text: None,
            confidence: None,
            language: None,
            speaker_segments: None,
            speaker_count: None,
            error: Some("timeout exceeded".to_string()),
            processing_time_ms: 5000,
        };

        assert!(result.text.is_none());
        assert_eq!(result.error.as_deref(), Some("timeout exceeded"));
    }

    #[test]
    fn test_job_result_serialize_roundtrip() {
        let result = JobResult {
            text: Some("test".to_string()),
            confidence: Some(0.9),
            language: None,
            speaker_segments: None,
            speaker_count: None,
            error: None,
            processing_time_ms: 100,
        };

        let json = serde_json::to_string(&result).unwrap();
        let restored: JobResult = serde_json::from_str(&json).unwrap();

        assert_eq!(result.text, restored.text);
        assert_eq!(result.processing_time_ms, restored.processing_time_ms);
    }

    #[test]
    fn test_queue_stats_defaults() {
        let stats = QueueStats {
            pending: 0,
            processing: 0,
            completed: 0,
            failed: 0,
            total: 0,
        };

        assert_eq!(stats.pending, 0);
        assert_eq!(stats.total, 0);
    }

    #[test]
    fn test_queue_stats_serialize() {
        let stats = QueueStats {
            pending: 10,
            processing: 3,
            completed: 100,
            failed: 2,
            total: 115,
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"pending\":10"));
        assert!(json.contains("\"total\":115"));
    }

    #[test]
    fn test_job_queue_is_debug() {
        fn assert_debug<T: std::fmt::Debug>() {}
        assert_debug::<JobQueue>();
    }

    #[test]
    fn test_transcription_job_is_debug_clone() {
        fn assert_debug_clone<T: std::fmt::Debug + Clone>() {}
        assert_debug_clone::<TranscriptionJob>();
    }

    #[test]
    fn test_enqueue_params_clone() {
        let params = EnqueueParams {
            call_id: Uuid::new_v4(),
            audio_path: Some("/tmp/test.mp3".to_string()),
            audio_data: None,
            priority: 1,
            options: serde_json::json!(null),
            timeout_seconds: 120,
        };

        let cloned = params.clone();
        assert_eq!(params.call_id, cloned.call_id);
        assert_eq!(params.audio_path, cloned.audio_path);
    }

    #[test]
    fn test_enqueue_params_defaults() {
        let call_id = Uuid::new_v4();
        let params = EnqueueParams {
            call_id,
            audio_path: Some(String::new()),
            audio_data: None,
            priority: 0,
            options: serde_json::Value::Null,
            timeout_seconds: 0,
        };

        assert_eq!(params.call_id, call_id);
        assert_eq!(params.audio_path.as_deref(), Some(""));
        assert_eq!(params.priority, 0);
        assert_eq!(params.options, serde_json::Value::Null);
        assert_eq!(params.timeout_seconds, 0);
    }

    #[test]
    fn test_job_result_all_none() {
        let result = JobResult {
            text: None,
            confidence: None,
            language: None,
            speaker_segments: None,
            speaker_count: None,
            error: None,
            processing_time_ms: 0,
        };

        assert!(result.text.is_none());
        assert!(result.confidence.is_none());
        assert!(result.language.is_none());
        assert!(result.speaker_segments.is_none());
        assert!(result.speaker_count.is_none());
        assert!(result.error.is_none());
        assert_eq!(result.processing_time_ms, 0);
    }

    #[test]
    fn test_job_result_with_values() {
        let segments = serde_json::json!([
            {"start": 0.0, "end": 2.5, "speaker": "SPEAKER_00"},
            {"start": 2.5, "end": 5.0, "speaker": "SPEAKER_01"}
        ]);

        let result = JobResult {
            text: Some("Unit one responding to dispatch".to_string()),
            confidence: Some(0.87),
            language: Some("en".to_string()),
            speaker_segments: Some(segments),
            speaker_count: Some(2),
            error: None,
            processing_time_ms: 3456,
        };

        let expected_segments = serde_json::json!([
            {"start": 0.0, "end": 2.5, "speaker": "SPEAKER_00"},
            {"start": 2.5, "end": 5.0, "speaker": "SPEAKER_01"}
        ]);

        assert_eq!(
            result.text.as_deref(),
            Some("Unit one responding to dispatch")
        );
        assert!((result.confidence.unwrap() - 0.87).abs() < f32::EPSILON);
        assert_eq!(result.language.as_deref(), Some("en"));
        assert_eq!(result.speaker_segments.unwrap(), expected_segments);
        assert_eq!(result.speaker_count, Some(2));
        assert!(result.error.is_none());
        assert_eq!(result.processing_time_ms, 3456);
    }

    #[test]
    fn test_queue_stats_serialization() {
        let stats = QueueStats {
            pending: 5,
            processing: 2,
            completed: 50,
            failed: 1,
            total: 58,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["pending"], 5);
        assert_eq!(value["processing"], 2);
        assert_eq!(value["completed"], 50);
        assert_eq!(value["failed"], 1);
        assert_eq!(value["total"], 58);
    }

    #[test]
    fn test_enqueue_params_with_options() {
        let complex_options = serde_json::json!({
            "model": "large-v3",
            "language": "en",
            "diarization": {
                "enabled": true,
                "min_speakers": 1,
                "max_speakers": 10
            },
            "vad": {
                "threshold": 0.5,
                "min_speech_duration_ms": 250
            },
            "tags": ["priority", "dispatch", "fire"]
        });

        let params = EnqueueParams {
            call_id: Uuid::new_v4(),
            audio_path: Some("/tmp/audio/complex.mp3".to_string()),
            audio_data: None,
            priority: 10,
            options: complex_options.clone(),
            timeout_seconds: 600,
        };

        assert_eq!(params.options["model"], "large-v3");
        assert_eq!(params.options["diarization"]["enabled"], true);
        assert_eq!(params.options["diarization"]["max_speakers"], 10);
        assert_eq!(params.options["vad"]["threshold"], 0.5);
        assert_eq!(params.options["tags"][2], "fire");

        // Verify roundtrip preserves complex options
        let json = serde_json::to_string(&params).unwrap();
        let restored: EnqueueParams = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.options, complex_options);
    }

    #[test]
    fn test_job_result_with_speaker_segments() {
        let segments = serde_json::json!([
            {
                "start": 0.0,
                "end": 1.2,
                "speaker": "SPEAKER_00",
                "text": "Dispatch, unit 42"
            },
            {
                "start": 1.5,
                "end": 3.8,
                "speaker": "SPEAKER_01",
                "text": "Unit 42, go ahead"
            },
            {
                "start": 4.0,
                "end": 7.2,
                "speaker": "SPEAKER_00",
                "text": "Responding to call at 123 Main Street"
            }
        ]);

        let result = JobResult {
            text: Some(
                "Dispatch, unit 42. Unit 42, go ahead. Responding to call at 123 Main Street"
                    .to_string(),
            ),
            confidence: Some(0.92),
            language: Some("en".to_string()),
            speaker_segments: Some(segments),
            speaker_count: Some(2),
            error: None,
            processing_time_ms: 2100,
        };

        // Serialize and deserialize to verify segments survive roundtrip
        let json = serde_json::to_string(&result).unwrap();
        let restored: JobResult = serde_json::from_str(&json).unwrap();

        let restored_segments = restored.speaker_segments.unwrap();
        let arr = restored_segments.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["speaker"], "SPEAKER_00");
        assert_eq!(arr[1]["speaker"], "SPEAKER_01");
        assert_eq!(arr[2]["text"], "Responding to call at 123 Main Street");
        assert_eq!(restored.speaker_count, Some(2));
    }
}
