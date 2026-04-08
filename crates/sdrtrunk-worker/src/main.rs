//! Standalone transcription worker binary for `SDRTrunk`.
//!
//! Designed to run as a K8s pod, this binary polls the `PostgreSQL` job queue for
//! pending transcription jobs, processes them via whisper.cpp (whisper-rs), and
//! writes results back to the database. Supports graceful `SIGTERM` shutdown,
//! heartbeat liveness probes, and automatic stale-job reclamation.

#![forbid(unsafe_code)]

mod whisper;

use anyhow::{Result, anyhow};
use sdrtrunk_protocol::Config;
use sdrtrunk_storage::jobs::{JobQueue, JobResult, TranscriptionJob};
use sdrtrunk_storage::queries::{RadioCallQueries, TranscriptionUpdate};
use sdrtrunk_storage::{Database, PgPool};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::sync::Notify;
use tracing::{error, info, warn};
use uuid::Uuid;
use whisper::WhisperEngine;

/// Load configuration from environment variables and config files.
///
/// # Errors
///
/// Returns an error if configuration cannot be loaded or parsed.
fn load_config() -> Result<Config> {
    let cfg = config::Config::builder()
        .add_source(config::File::with_name("config").required(false))
        .add_source(config::Environment::with_prefix("SDRTRUNK").separator("__"))
        .build()
        .map_err(|e| anyhow!("Configuration load error: {e}"))?;
    cfg.try_deserialize()
        .map_err(|e| anyhow!("Configuration deserialization error: {e}"))
}

/// Determine the worker identifier.
///
/// Uses the configured `worker_id` if present, then the `HOSTNAME` environment
/// variable (set automatically by K8s), falling back to a random UUID.
fn resolve_worker_id(config: &Config) -> String {
    if let Some(id) = config
        .transcription
        .as_ref()
        .and_then(|t| t.worker_id.clone())
    {
        return id;
    }
    std::env::var("HOSTNAME").unwrap_or_else(|_| Uuid::new_v4().to_string())
}

/// Process a single transcription job to completion or failure.
///
/// Spawns a heartbeat task, runs the Whisper engine, and writes the
/// result (success or failure) back to the database. Updates the corresponding
/// `radio_calls` row as well.
///
/// # Errors
///
/// Returns an error if database operations fail in an unrecoverable way.
async fn process_job(
    pool: &PgPool,
    engine: &WhisperEngine,
    job: &TranscriptionJob,
    worker_id: &str,
    heartbeat_interval: u64,
) -> Result<()> {
    let job_id = job.id;
    let call_id = job.call_id;

    info!(job_id = %job_id, call_id = %call_id, "Processing transcription job");

    // --- Heartbeat task ---
    let hb_pool = pool.clone();
    let hb_worker = worker_id.to_string();
    let hb_cancel = Arc::new(Notify::new());
    let hb_cancel_rx = hb_cancel.clone();

    let heartbeat_handle = tokio::spawn(async move {
        let dur = tokio::time::Duration::from_secs(heartbeat_interval);
        loop {
            tokio::select! {
                () = tokio::time::sleep(dur) => {
                    if let Err(e) = JobQueue::heartbeat(&hb_pool, job_id, &hb_worker).await {
                        warn!(job_id = %job_id, error = %e, "Heartbeat failed");
                        break;
                    }
                }
                () = hb_cancel_rx.notified() => {
                    break;
                }
            }
        }
    });

    // --- Resolve audio to a file path ---
    // If audio_data is present, write it to a temp file; otherwise use audio_path directly.
    let temp_file: Option<tempfile::NamedTempFile>;
    let audio_path: std::path::PathBuf;

    if let Some(ref bytes) = job.audio_data {
        let tmp = tempfile::Builder::new()
            .suffix(".mp3")
            .tempfile()
            .map_err(|e| anyhow!("Failed to create temp file for audio_data: {e}"))?;
        std::fs::write(tmp.path(), bytes)
            .map_err(|e| anyhow!("Failed to write audio_data to temp file: {e}"))?;
        audio_path = tmp.path().to_path_buf();
        temp_file = Some(tmp);
    } else if let Some(ref path) = job.audio_path {
        audio_path = std::path::PathBuf::from(path);
        temp_file = None;
    } else {
        // Neither audio_data nor audio_path — fail the job
        hb_cancel.notify_one();
        let _join = heartbeat_handle.await;
        handle_failure(pool, job, "Job has neither audio_data nor audio_path").await?;
        return Ok(());
    }

    // --- Transcription ---
    let start = Instant::now();
    let result = engine.transcribe(&audio_path);
    let elapsed_ms = i64::try_from(start.elapsed().as_millis()).unwrap_or(i64::MAX);

    // Drop temp file (cleaned up on drop, but be explicit)
    drop(temp_file);

    // Stop heartbeat
    hb_cancel.notify_one();
    let _join = heartbeat_handle.await;

    match result {
        Ok(transcription) => {
            let job_result = JobResult {
                text: Some(transcription.text),
                confidence: None,
                language: Some("en".to_string()),
                speaker_segments: None,
                speaker_count: None,
                error: None,
                processing_time_ms: elapsed_ms,
            };
            handle_success(pool, job_id, call_id, &job_result).await?;
        }
        Err(e) => {
            handle_failure(pool, job, &e.to_string()).await?;
        }
    }

    Ok(())
}

/// Record a successful transcription in both the job queue and the radio call.
///
/// # Errors
///
/// Returns an error if database writes fail.
async fn handle_success(
    pool: &PgPool,
    job_id: Uuid,
    call_id: Uuid,
    job_result: &JobResult,
) -> Result<()> {
    JobQueue::complete(pool, job_id, job_result)
        .await
        .map_err(|e| {
            error!(job_id = %job_id, error = %e, "Failed to complete job in queue");
            anyhow!("Failed to complete job: {e}")
        })?;

    RadioCallQueries::update_transcription_status(
        pool,
        TranscriptionUpdate {
            id: call_id,
            status: "completed",
            text: job_result.text.as_deref(),
            confidence: None,
            error: None,
            speaker_segments: None,
            speaker_count: None,
        },
    )
    .await
    .map_err(|e| {
        error!(call_id = %call_id, error = %e, "Failed to update call transcription status");
        anyhow!("Failed to update call status: {e}")
    })?;

    let elapsed_ms = job_result.processing_time_ms;
    info!(job_id = %job_id, call_id = %call_id, elapsed_ms, "Transcription completed");
    Ok(())
}

/// Record a transcription failure and update the radio call if retries are
/// exhausted.
///
/// # Errors
///
/// Returns an error if database writes fail.
async fn handle_failure(pool: &PgPool, job: &TranscriptionJob, error_msg: &str) -> Result<()> {
    let job_id = job.id;
    let call_id = job.call_id;

    warn!(job_id = %job_id, call_id = %call_id, error = %error_msg, "Transcription failed");

    JobQueue::fail(pool, job_id, error_msg).await.map_err(|e| {
        error!(job_id = %job_id, error = %e, "Failed to record job failure");
        anyhow!("Failed to record job failure: {e}")
    })?;

    // If retries are exhausted, mark the radio call as permanently failed
    let retries_exhausted = job.retry_count + 1 >= job.max_retries;
    if retries_exhausted {
        RadioCallQueries::update_transcription_status(
            pool,
            TranscriptionUpdate {
                id: call_id,
                status: "failed",
                text: None,
                confidence: None,
                error: Some(error_msg),
                speaker_segments: None,
                speaker_count: None,
            },
        )
        .await
        .map_err(|e| {
            error!(call_id = %call_id, error = %e, "Failed to update call status");
            anyhow!("Failed to update call status: {e}")
        })?;
    }

    Ok(())
}

/// Application entry point.
#[tokio::main]
#[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
async fn main() -> Result<()> {
    init_logging();

    info!("SDRTrunk Worker starting");

    let config = load_config().unwrap_or_else(|err| {
        info!("Failed to load config ({}), using defaults", err);
        Config::default()
    });

    let transcription_config = config.transcription.clone().unwrap_or_default();
    let worker_id = resolve_worker_id(&config);
    let poll_interval =
        tokio::time::Duration::from_secs(transcription_config.poll_interval_seconds);
    let heartbeat_interval = transcription_config.heartbeat_interval_seconds;

    info!(worker_id = %worker_id, "Worker identity resolved");

    // --- Database ---
    let database = Database::new(&config).await.map_err(|e| {
        error!("Failed to connect to database: {}", e);
        anyhow!("Database connection failed: {e}")
    })?;
    let pool = database.pool().clone();
    info!("Database connection established");

    // --- Whisper engine ---
    let model_path = std::env::var("WHISPER_MODEL_PATH")
        .unwrap_or_else(|_| "/models/ggml-large-v3.bin".to_string());
    let engine = WhisperEngine::load(std::path::Path::new(&model_path))?;
    info!("Whisper engine loaded");

    // --- Graceful shutdown ---
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_flag = shutdown.clone();
    let _shutdown_task = tokio::spawn(async move {
        wait_for_shutdown().await;
        shutdown_flag.store(true, Ordering::SeqCst);
    });

    info!("Entering poll loop");
    let ctx = WorkerContext {
        pool: &pool,
        engine: &engine,
        shutdown: &shutdown,
        worker_id: &worker_id,
        poll_interval,
        heartbeat_interval,
    };
    run_poll_loop(&ctx).await;

    // Cleanup
    info!("Shutdown signal received, cleaning up");
    info!("Worker shutdown complete");

    Ok(())
}

/// Bundles references needed by the poll loop to avoid excessive parameters.
struct WorkerContext<'a> {
    /// Database connection pool.
    pool: &'a PgPool,
    /// Whisper transcription engine.
    engine: &'a WhisperEngine,
    /// Flag set when the process should stop.
    shutdown: &'a AtomicBool,
    /// Unique worker identifier.
    worker_id: &'a str,
    /// How long to sleep when no jobs are available.
    poll_interval: tokio::time::Duration,
    /// Seconds between heartbeat pings.
    heartbeat_interval: u64,
}

/// Main poll loop: reclaim stale jobs, claim new ones, and process them.
#[allow(clippy::cognitive_complexity)]
async fn run_poll_loop(ctx: &WorkerContext<'_>) {
    while !ctx.shutdown.load(Ordering::SeqCst) {
        // Reclaim stale jobs from dead workers
        match JobQueue::reclaim_stale(ctx.pool).await {
            Ok(count) if count > 0 => {
                info!(reclaimed = count, "Reclaimed stale jobs");
            }
            Ok(_) => {}
            Err(e) => {
                warn!(error = %e, "Failed to reclaim stale jobs");
            }
        }

        // Try to claim a job
        let job = match JobQueue::claim(ctx.pool, ctx.worker_id).await {
            Ok(Some(job)) => job,
            Ok(None) => {
                wait_or_shutdown(ctx.poll_interval, ctx.shutdown).await;
                continue;
            }
            Err(e) => {
                error!(error = %e, "Failed to claim job");
                wait_or_shutdown(ctx.poll_interval, ctx.shutdown).await;
                continue;
            }
        };

        if let Err(e) = process_job(
            ctx.pool,
            ctx.engine,
            &job,
            ctx.worker_id,
            ctx.heartbeat_interval,
        )
        .await
        {
            error!(job_id = %job.id, error = %e, "Unrecoverable error processing job");
        }
    }
}

/// Sleep for `duration` but wake early if the shutdown flag is set.
async fn wait_or_shutdown(duration: tokio::time::Duration, shutdown: &AtomicBool) {
    let check = tokio::time::Duration::from_millis(100);
    let deadline = tokio::time::Instant::now() + duration;
    while tokio::time::Instant::now() < deadline {
        if shutdown.load(Ordering::SeqCst) {
            return;
        }
        tokio::time::sleep(check).await;
    }
}

/// Initialize structured logging via `tracing-subscriber`.
fn init_logging() {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(
            fmt::layer()
                .with_target(false)
                .with_thread_ids(false)
                .with_thread_names(false)
                .with_file(false)
                .with_line_number(false)
                .with_level(true)
                .compact(),
        )
        .init();
}

/// Wait for a shutdown signal (`SIGTERM` or `Ctrl+C`).
///
/// # Panics
///
/// Panics if signal handlers cannot be installed.
#[allow(clippy::expect_used)]
async fn wait_for_shutdown() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        let _ = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            info!("Received Ctrl+C, initiating graceful shutdown");
        }
        () = terminate => {
            info!("Received SIGTERM, initiating graceful shutdown");
        }
    }
}
