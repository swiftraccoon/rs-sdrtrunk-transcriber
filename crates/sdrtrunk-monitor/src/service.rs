//! Main monitoring service implementation
//!
//! Orchestrates file monitoring, queuing, and processing with comprehensive
//! error handling, metrics collection, and graceful shutdown capabilities.

use crate::{
    MonitorError, Result,
    config::MonitorConfig,
    monitor::{FileEvent, FileEventType, FileMonitor},
    processor::{FileProcessor, ProcessingStatus},
    queue::{FileQueue, QueueStats},
};
use dashmap::DashMap;
use parking_lot::RwLock;
use sqlx::PgPool;
use std::sync::{Arc, atomic::AtomicU64};
use std::time::Duration;
use tokio::{
    sync::{Notify, broadcast, mpsc},
    task::JoinHandle,
    time::{Instant, interval},
};
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

/// Service metrics
#[derive(Debug, Clone, Default)]
pub struct ServiceMetrics {
    /// Total files detected
    pub files_detected: u64,

    /// Total files queued
    pub files_queued: u64,

    /// Total files processed successfully
    pub files_processed: u64,

    /// Total files failed
    pub files_failed: u64,

    /// Total files skipped
    pub files_skipped: u64,

    /// Total files archived
    pub files_archived: u64,

    /// Average processing time in milliseconds
    pub avg_processing_time_ms: f64,

    /// Service uptime in seconds
    pub uptime_seconds: u64,

    /// Last health check timestamp
    pub last_health_check: Option<chrono::DateTime<chrono::Utc>>,

    /// Current service status
    pub status: ServiceStatus,
}

/// Service status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceStatus {
    /// Service is stopped
    Stopped,

    /// Service is starting up
    Starting,

    /// Service is running normally
    Running,

    /// Service is shutting down
    Stopping,

    /// Service encountered an error but is still running
    Degraded {
        /// Reason for degraded status
        reason: String,
    },

    /// Service failed and stopped
    Failed {
        /// Reason for failure
        reason: String,
    },
}

impl Default for ServiceStatus {
    fn default() -> Self {
        Self::Stopped
    }
}

/// Main monitoring service
#[derive(Debug)]
pub struct MonitorService {
    /// Service configuration
    config: MonitorConfig,

    /// Database connection pool
    db_pool: Arc<PgPool>,

    /// File monitor
    file_monitor: Arc<RwLock<FileMonitor>>,

    /// File processing queue
    file_queue: Arc<FileQueue>,

    /// File processor
    file_processor: Arc<FileProcessor>,

    /// Service metrics
    metrics: Arc<RwLock<ServiceMetrics>>,

    /// Running task handles
    task_handles: Arc<RwLock<Vec<JoinHandle<()>>>>,

    /// Shutdown signal
    shutdown_notify: Arc<Notify>,

    /// Shutdown sender (for broadcasting shutdown)
    shutdown_tx: broadcast::Sender<()>,

    /// Service status
    status: Arc<RwLock<ServiceStatus>>,

    /// Service start time
    start_time: Arc<RwLock<Option<Instant>>>,

    /// Processing statistics
    processing_times: Arc<DashMap<Uuid, Duration>>,

    /// Restart attempt counter
    _restart_attempts: AtomicU64,
}

impl MonitorService {
    /// Create a new monitoring service
    pub async fn new(config: MonitorConfig) -> Result<Self> {
        info!("Initializing monitoring service");

        // Create database connection pool
        let db_pool = Arc::new(PgPool::connect(&config.database.url).await.map_err(|e| {
            MonitorError::configuration(format!("Failed to connect to database: {e}"))
        })?);

        // Test database connection
        sqlx::query("SELECT 1")
            .fetch_one(db_pool.as_ref())
            .await
            .map_err(|e| {
                MonitorError::configuration(format!("Database connection test failed: {e}"))
            })?;

        // Create directories
        tokio::fs::create_dir_all(&config.watch.watch_directory).await?;
        tokio::fs::create_dir_all(&config.storage.archive_directory).await?;
        tokio::fs::create_dir_all(&config.storage.failed_directory).await?;
        tokio::fs::create_dir_all(&config.storage.temp_directory).await?;

        // Create file monitor
        let file_monitor = Arc::new(RwLock::new(FileMonitor::new(config.watch.clone())));

        // Create file queue
        let file_queue = Arc::new(FileQueue::new(
            config.queue.max_queue_size,
            config.queue.persistence_file.clone(),
            config.queue.priority_by_age,
            config.queue.priority_by_size,
        ));

        // Load queue from persistence
        file_queue.load_from_persistence().await?;

        // Create file processor
        let file_processor = Arc::new(FileProcessor::new(
            db_pool.clone(),
            config.processing.clone(),
            config.storage.archive_directory.clone(),
            config.storage.failed_directory.clone(),
            config.storage.temp_directory.clone(),
        ));

        // Create shutdown channel
        let (shutdown_tx, _) = broadcast::channel(1);

        let service = Self {
            config,
            db_pool,
            file_monitor,
            file_queue,
            file_processor,
            metrics: Arc::new(RwLock::new(ServiceMetrics::default())),
            task_handles: Arc::new(RwLock::new(Vec::new())),
            shutdown_notify: Arc::new(Notify::new()),
            shutdown_tx,
            status: Arc::new(RwLock::new(ServiceStatus::Stopped)),
            start_time: Arc::new(RwLock::new(None)),
            processing_times: Arc::new(DashMap::new()),
            _restart_attempts: AtomicU64::new(0),
        };

        info!("Monitoring service initialized successfully");
        Ok(service)
    }

    /// Start the monitoring service
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<()> {
        let mut status = self.status.write();
        if *status != ServiceStatus::Stopped {
            return Err(MonitorError::ServiceAlreadyRunning);
        }
        *status = ServiceStatus::Starting;
        drop(status);

        info!("Starting monitoring service");

        // Set start time
        *self.start_time.write() = Some(Instant::now());

        // Start file monitor
        let event_receiver = {
            let mut monitor = self.file_monitor.write();
            monitor.start().await?
        };

        // Scan for existing files
        let existing_files = {
            let monitor = self.file_monitor.read();
            monitor.scan_existing_files().await?
        };

        // Queue existing files
        for file_path in existing_files {
            if let Err(e) = self.file_queue.enqueue(file_path.clone()).await {
                warn!(
                    path = %file_path.display(),
                    error = %e,
                    "Failed to queue existing file"
                );
            }
        }

        // Start background tasks
        let mut handles = self.task_handles.write();

        // Task 1: File event handler
        handles.push(self.spawn_file_event_handler(event_receiver));

        // Task 2: File processor workers
        for worker_id in 0..self.config.processing.processing_workers {
            handles.push(self.spawn_processing_worker(worker_id));
        }

        // Task 3: Health check task
        if self.config.service.health_check_interval_seconds > 0 {
            handles.push(self.spawn_health_check_task());
        }

        // Task 4: Metrics collection task
        if self.config.service.enable_metrics {
            handles.push(self.spawn_metrics_task());
        }

        // Task 5: Queue persistence task
        if self.file_queue.persistence_file.is_some() {
            handles.push(self.spawn_persistence_task());
        }

        // Update status
        *self.status.write() = ServiceStatus::Running;

        info!(
            watch_dir = %self.config.watch.watch_directory.display(),
            workers = self.config.processing.processing_workers,
            "Monitoring service started successfully"
        );

        Ok(())
    }

    /// Stop the monitoring service
    #[instrument(skip(self))]
    pub async fn stop(&self) -> Result<()> {
        let mut status = self.status.write();
        if *status == ServiceStatus::Stopped {
            return Ok(());
        }
        *status = ServiceStatus::Stopping;
        drop(status);

        info!("Stopping monitoring service");

        // Send shutdown signal
        let _ = self.shutdown_tx.send(());
        self.shutdown_notify.notify_waiters();

        // Wait for tasks to complete with timeout
        let timeout_duration = self.config.service.shutdown_timeout();
        let shutdown_result = tokio::time::timeout(timeout_duration, async {
            // Stop file monitor
            {
                let mut monitor = self.file_monitor.write();
                monitor.stop();
            }

            // Wait for all tasks to complete
            let mut handles = self.task_handles.write();
            for handle in handles.drain(..) {
                let _ = handle.await;
            }
        })
        .await;

        if shutdown_result.is_err() {
            warn!("Service shutdown timed out, some tasks may still be running");
        }

        // Save queue state
        if let Err(e) = self.file_queue.save_to_persistence().await {
            error!("Failed to save queue state: {}", e);
        }

        // Update status
        *self.status.write() = ServiceStatus::Stopped;
        *self.start_time.write() = None;

        info!("Monitoring service stopped");
        Ok(())
    }

    /// Get service status
    #[must_use]
    pub fn status(&self) -> ServiceStatus {
        self.status.read().clone()
    }

    /// Get service metrics
    #[must_use]
    pub fn metrics(&self) -> ServiceMetrics {
        let mut metrics = self.metrics.read().clone();

        // Update uptime
        if let Some(start_time) = *self.start_time.read() {
            metrics.uptime_seconds = start_time.elapsed().as_secs();
        }

        // Update status
        metrics.status = self.status();

        metrics
    }

    /// Get queue statistics
    #[must_use]
    pub fn queue_stats(&self) -> QueueStats {
        self.file_queue.stats()
    }

    /// Wait for shutdown signal
    pub async fn wait_for_shutdown(&self) {
        self.shutdown_notify.notified().await;
    }

    /// Spawn file event handler task
    fn spawn_file_event_handler(
        &self,
        mut event_receiver: mpsc::Receiver<FileEvent>,
    ) -> JoinHandle<()> {
        let file_queue = self.file_queue.clone();
        let metrics = self.metrics.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    event = event_receiver.recv() => {
                        match event {
                            Some(event) => {
                                Self::handle_file_event(event, &file_queue, &metrics).await;
                            }
                            None => {
                                debug!("File event receiver closed");
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("File event handler shutting down");
                        break;
                    }
                }
            }
        })
    }

    /// Handle a single file event
    async fn handle_file_event(
        event: FileEvent,
        file_queue: &FileQueue,
        metrics: &Arc<RwLock<ServiceMetrics>>,
    ) {
        debug!(
            path = %event.path.display(),
            event_type = ?event.event_type,
            "Handling file event"
        );

        // Update metrics
        {
            let mut m = metrics.write();
            m.files_detected += 1;
        }

        match event.event_type {
            FileEventType::Created | FileEventType::MovedTo => {
                // Queue file for processing
                match file_queue.enqueue(event.path.clone()).await {
                    Ok(file_id) => {
                        info!(
                            file_id = %file_id,
                            path = %event.path.display(),
                            "File queued for processing"
                        );

                        let mut m = metrics.write();
                        m.files_queued += 1;
                    }
                    Err(e) => {
                        warn!(
                            path = %event.path.display(),
                            error = %e,
                            "Failed to queue file"
                        );
                    }
                }
            }
            FileEventType::Modified => {
                // For modified events, we might want to re-queue or update existing entries
                // For now, we'll just log and ignore
                debug!(
                    path = %event.path.display(),
                    "File modified, ignoring"
                );
            }
            FileEventType::Removed => {
                debug!(
                    path = %event.path.display(),
                    "File removed"
                );
                // Could potentially remove from queue if it's still pending
            }
        }
    }

    /// Spawn file processing worker task
    fn spawn_processing_worker(&self, worker_id: usize) -> JoinHandle<()> {
        let file_queue = self.file_queue.clone();
        let file_processor = self.file_processor.clone();
        let metrics = self.metrics.clone();
        let processing_times = self.processing_times.clone();
        let processing_interval = self.config.processing.processing_interval();
        let max_retries = self.config.processing.max_retry_attempts;
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            info!(worker_id, "Processing worker started");

            let mut interval = interval(processing_interval);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Some(file) = file_queue.dequeue().await {
                            debug!(
                                worker_id,
                                file_id = %file.id,
                                path = %file.path.display(),
                                "Processing file"
                            );

                            let start_time = Instant::now();
                            let result = file_processor.process_file(file.clone()).await;
                            let processing_time = start_time.elapsed();

                            // Store processing time
                            processing_times.insert(file.id, processing_time);

                            match result.status {
                                ProcessingStatus::Completed => {
                                    if let Err(e) = file_queue.mark_completed(file.id).await {
                                        error!(
                                            worker_id,
                                            file_id = %file.id,
                                            error = %e,
                                            "Failed to mark file as completed"
                                        );
                                    } else {
                                        let mut m = metrics.write();
                                        m.files_processed += 1;
                                        if result.archive_path.is_some() {
                                            m.files_archived += 1;
                                        }
                                    }
                                }
                                ProcessingStatus::Failed { error, .. } => {
                                    match file_queue.mark_failed(file.id, error.clone(), max_retries).await {
                                        Ok(will_retry) => {
                                            if !will_retry {
                                                let mut m = metrics.write();
                                                m.files_failed += 1;
                                            }
                                        }
                                        Err(e) => {
                                            error!(
                                                worker_id,
                                                file_id = %file.id,
                                                error = %e,
                                                "Failed to mark file as failed"
                                            );
                                        }
                                    }
                                }
                                ProcessingStatus::Skipped { .. } => {
                                    if let Err(e) = file_queue.mark_completed(file.id).await {
                                        error!(
                                            worker_id,
                                            file_id = %file.id,
                                            error = %e,
                                            "Failed to mark skipped file as completed"
                                        );
                                    } else {
                                        let mut m = metrics.write();
                                        m.files_skipped += 1;
                                    }
                                }
                                _ => {
                                    warn!(
                                        worker_id,
                                        file_id = %file.id,
                                        status = ?result.status,
                                        "Unexpected processing status"
                                    );
                                }
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!(worker_id, "Processing worker shutting down");
                        break;
                    }
                }
            }
        })
    }

    /// Spawn health check task
    fn spawn_health_check_task(&self) -> JoinHandle<()> {
        let db_pool = self.db_pool.clone();
        let metrics = self.metrics.clone();
        let status = self.status.clone();
        let health_check_interval = self.config.service.health_check_interval();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            let mut interval = interval(health_check_interval);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let health_ok = Self::perform_health_check(&db_pool).await;

                        let mut m = metrics.write();
                        m.last_health_check = Some(chrono::Utc::now());

                        if !health_ok {
                            warn!("Health check failed");
                            *status.write() = ServiceStatus::Degraded {
                                reason: "Health check failed".to_string(),
                            };
                        } else if matches!(*status.read(), ServiceStatus::Degraded { .. }) {
                            info!("Health check passed, service recovered");
                            *status.write() = ServiceStatus::Running;
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Health check task shutting down");
                        break;
                    }
                }
            }
        })
    }

    /// Perform health check
    async fn perform_health_check(db_pool: &PgPool) -> bool {
        // Test database connection
        match sqlx::query("SELECT 1 as health").fetch_one(db_pool).await {
            Ok(_) => {
                debug!("Database health check passed");
                true
            }
            Err(e) => {
                error!("Database health check failed: {}", e);
                false
            }
        }
    }

    /// Spawn metrics collection task
    fn spawn_metrics_task(&self) -> JoinHandle<()> {
        let metrics = self.metrics.clone();
        let processing_times = self.processing_times.clone();
        let metrics_interval = self.config.service.metrics_interval();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            let mut interval = interval(metrics_interval);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::update_metrics(&metrics, &processing_times);
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Metrics task shutting down");
                        break;
                    }
                }
            }
        })
    }

    /// Update service metrics
    fn update_metrics(
        metrics: &Arc<RwLock<ServiceMetrics>>,
        processing_times: &DashMap<Uuid, Duration>,
    ) {
        let mut m = metrics.write();

        // Calculate average processing time
        if !processing_times.is_empty() {
            let total_time: Duration = processing_times.iter().map(|entry| *entry.value()).sum();
            m.avg_processing_time_ms =
                total_time.as_millis() as f64 / processing_times.len() as f64;

            // Clean up old processing times to prevent memory leak
            if processing_times.len() > 1000 {
                let old_entries: Vec<_> = processing_times
                    .iter()
                    .take(500)
                    .map(|entry| *entry.key())
                    .collect();
                for key in old_entries {
                    processing_times.remove(&key);
                }
            }
        }

        debug!(
            files_processed = m.files_processed,
            files_queued = m.files_queued,
            files_failed = m.files_failed,
            avg_processing_time_ms = m.avg_processing_time_ms,
            "Updated metrics"
        );
    }

    /// Spawn queue persistence task
    fn spawn_persistence_task(&self) -> JoinHandle<()> {
        let file_queue = self.file_queue.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60)); // Save every minute

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = file_queue.save_to_persistence().await {
                            error!("Failed to save queue persistence: {}", e);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Persistence task shutting down");
                        // Save one final time
                        if let Err(e) = file_queue.save_to_persistence().await {
                            error!("Failed to save final queue state: {}", e);
                        }
                        break;
                    }
                }
            }
        })
    }
}

impl Drop for MonitorService {
    fn drop(&mut self) {
        // Ensure service is stopped when dropped
        if !matches!(*self.status.read(), ServiceStatus::Stopped) {
            warn!("MonitorService dropped while still running");
            let _ = self.shutdown_tx.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_service_lifecycle() {
        let temp_dir = TempDir::new().unwrap();

        let mut config = MonitorConfig::default();
        config.watch.watch_directory = temp_dir.path().to_path_buf();
        config.storage.archive_directory = temp_dir.path().join("archive");
        config.storage.failed_directory = temp_dir.path().join("failed");
        config.storage.temp_directory = temp_dir.path().join("temp");

        // This test would need a real database connection
        // In a real test environment, you'd use testcontainers
        // For now, this is just a structure test

        assert_eq!(config.service.name, "sdrtrunk-monitor");
    }
}
