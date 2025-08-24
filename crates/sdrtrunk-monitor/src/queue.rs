//! File processing queue implementation
//!
//! Provides a thread-safe, persistent queue for managing files waiting to be processed.
//! Supports priority queuing and crash recovery through optional persistence.

use crate::{MonitorError, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use uuid::Uuid;

/// A file queued for processing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueuedFile {
    /// Unique identifier for this queue entry
    pub id: Uuid,

    /// Full path to the file
    pub path: PathBuf,

    /// File size in bytes
    pub size: u64,

    /// When the file was detected/queued
    pub queued_at: DateTime<Utc>,

    /// When the file was last modified
    pub modified_at: DateTime<Utc>,

    /// Processing priority (higher = more important)
    pub priority: i32,

    /// Number of processing attempts
    pub retry_count: u32,

    /// Last processing error (if any)
    pub last_error: Option<String>,

    /// File metadata
    pub metadata: FileMetadata,
}

/// File metadata extracted during queuing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileMetadata {
    /// File extension
    pub extension: Option<String>,

    /// File stem (name without extension)
    pub stem: String,

    /// Whether the file is a symbolic link
    pub is_symlink: bool,

    /// File checksum (for integrity verification)
    pub checksum: Option<String>,
}

/// Queue statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStats {
    /// Total number of files in queue
    pub total_files: usize,

    /// Number of files currently being processed
    pub processing_files: usize,

    /// Number of files that failed processing
    pub failed_files: usize,

    /// Average queue wait time in seconds
    pub average_wait_time: f64,

    /// Oldest file in queue
    pub oldest_queued: Option<DateTime<Utc>>,

    /// Total files processed since startup
    pub total_processed: u64,
}

impl PartialOrd for QueuedFile {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedFile {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then older files first
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.queued_at.cmp(&self.queued_at))
    }
}

/// Thread-safe file processing queue
#[derive(Debug)]
pub struct FileQueue {
    /// Priority queue for pending files
    pending: Arc<RwLock<BinaryHeap<QueuedFile>>>,

    /// Currently processing files (id -> file)
    processing: Arc<DashMap<Uuid, QueuedFile>>,

    /// Failed files (id -> file)
    failed: Arc<DashMap<Uuid, QueuedFile>>,

    /// Maximum queue size
    max_size: usize,
    /// Optional file for queue persistence
    pub persistence_file: Option<PathBuf>,
    /// Whether to prioritize older files
    priority_by_age: bool,
    /// Whether to prioritize smaller files
    priority_by_size: bool,

    /// Statistics
    stats: Arc<RwLock<QueueStats>>,
}

impl FileQueue {
    /// Create a new file queue
    #[must_use]
    pub fn new(
        max_size: usize,
        persistence_file: Option<PathBuf>,
        priority_by_age: bool,
        priority_by_size: bool,
    ) -> Self {
        Self {
            pending: Arc::new(RwLock::new(BinaryHeap::new())),
            processing: Arc::new(DashMap::new()),
            failed: Arc::new(DashMap::new()),
            max_size,
            persistence_file,
            priority_by_age,
            priority_by_size,
            stats: Arc::new(RwLock::new(QueueStats {
                total_files: 0,
                processing_files: 0,
                failed_files: 0,
                average_wait_time: 0.0,
                oldest_queued: None,
                total_processed: 0,
            })),
        }
    }

    /// Load queue state from persistence file
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if:
    /// - File cannot be read
    /// - JSON deserialization fails
    pub async fn load_from_persistence(&self) -> Result<()> {
        if let Some(ref persistence_file) = self.persistence_file
            && persistence_file.exists()
        {
            tracing::info!("Loading queue state from {}", persistence_file.display());

            let data = fs::read_to_string(persistence_file).await?;
            let saved_files: Vec<QueuedFile> = serde_json::from_str(&data).map_err(|e| {
                MonitorError::queue(format!("Failed to parse persistence file: {e}"))
            })?;

            {
                let mut pending = self.pending.write();
                let num_files = saved_files.len();
                for file in saved_files {
                    pending.push(file);
                }
                drop(pending);

                self.update_stats();
                tracing::info!("Loaded {} files from persistence", num_files);
            }
        }
        Ok(())
    }

    /// Save queue state to persistence file
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if:
    /// - File cannot be written
    /// - JSON serialization fails
    pub async fn save_to_persistence(&self) -> Result<()> {
        if let Some(ref persistence_file) = self.persistence_file {
            let files = {
                let pending = self.pending.read();
                pending.clone().into_sorted_vec()
            }; // Drop the lock before async operations

            let data = serde_json::to_string_pretty(&files)
                .map_err(|e| MonitorError::queue(format!("Failed to serialize queue: {e}")))?;

            // Create parent directory if it doesn't exist
            if let Some(parent) = persistence_file.parent() {
                fs::create_dir_all(parent).await?;
            }

            fs::write(persistence_file, data).await?;
            tracing::debug!("Saved queue state to {}", persistence_file.display());
        }
        Ok(())
    }

    /// Add a file to the processing queue
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if:
    /// - Queue is at maximum capacity
    /// - File metadata cannot be read
    /// - Persistence saving fails
    pub async fn enqueue(&self, path: PathBuf) -> Result<Uuid> {
        // Check if queue is full
        if self.pending.read().len() >= self.max_size {
            return Err(MonitorError::queue("Queue is full"));
        }

        // Check if file already exists in queue
        {
            let pending = self.pending.read();
            if pending.iter().any(|f| f.path == path) {
                return Err(MonitorError::queue(format!(
                    "File already in queue: {}",
                    path.display()
                )));
            }
        } // Drop the lock before async operations

        // Get file metadata
        let metadata = fs::metadata(&path).await?;
        let modified_at = metadata.modified()?.into();

        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .map(ToString::to_string);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let file_metadata = FileMetadata {
            extension,
            stem,
            is_symlink: metadata.file_type().is_symlink(),
            checksum: None, // Will be computed during processing if needed
        };

        let priority = self.calculate_priority(metadata.len(), modified_at);

        let queued_file = QueuedFile {
            id: Uuid::new_v4(),
            path,
            size: metadata.len(),
            queued_at: Utc::now(),
            modified_at,
            priority,
            retry_count: 0,
            last_error: None,
            metadata: file_metadata,
        };

        let id = queued_file.id;

        // Add to queue
        self.pending.write().push(queued_file.clone());

        tracing::debug!(
            file_id = %id,
            path = %queued_file.path.display(),
            priority = priority,
            "Enqueued file for processing"
        );

        self.update_stats();
        self.save_to_persistence().await?;

        Ok(id)
    }

    /// Get the next file to process
    pub fn dequeue(&self) -> Option<QueuedFile> {
        let file = {
            let mut pending = self.pending.write();
            pending.pop()
        }; // Drop the lock before async operations

        if let Some(file) = file {
            // Move to processing
            self.processing.insert(file.id, file.clone());

            tracing::debug!(
                file_id = %file.id,
                path = %file.path.display(),
                "Dequeued file for processing"
            );

            self.update_stats();
            Some(file)
        } else {
            None
        }
    }

    /// Mark a file as successfully processed
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if persistence saving fails
    pub async fn mark_completed(&self, file_id: Uuid) -> Result<()> {
        if let Some((_, file)) = self.processing.remove(&file_id) {
            tracing::debug!(
                file_id = %file_id,
                path = %file.path.display(),
                "Marked file as completed"
            );

            // Update statistics
            {
                let mut stats = self.stats.write();
                stats.total_processed += 1;
            }

            self.update_stats();
            self.save_to_persistence().await?;
            Ok(())
        } else {
            Err(MonitorError::queue(format!(
                "File not found in processing queue: {file_id}"
            )))
        }
    }

    /// Mark a file as failed and optionally retry
    /// Mark a file as failed processing
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if persistence saving fails
    pub async fn mark_failed(
        &self,
        file_id: Uuid,
        error: String,
        max_retries: u32,
    ) -> Result<bool> {
        if let Some((_, mut file)) = self.processing.remove(&file_id) {
            file.retry_count += 1;
            file.last_error = Some(error.clone());

            let should_retry = file.retry_count <= max_retries;

            if should_retry {
                // Re-queue for retry with updated retry count
                self.pending.write().push(file.clone());

                tracing::warn!(
                    file_id = %file_id,
                    path = %file.path.display(),
                    retry_count = file.retry_count,
                    error = %error,
                    "File processing failed, will retry"
                );
            } else {
                // Move to failed queue
                self.failed.insert(file_id, file.clone());

                tracing::error!(
                    file_id = %file_id,
                    path = %file.path.display(),
                    retry_count = file.retry_count,
                    error = %error,
                    "File processing failed permanently"
                );
            }

            self.update_stats();
            self.save_to_persistence().await?;
            Ok(should_retry)
        } else {
            Err(MonitorError::queue(format!(
                "File not found in processing queue: {file_id}"
            )))
        }
    }

    /// Get queue statistics
    #[must_use]
    pub fn stats(&self) -> QueueStats {
        self.stats.read().clone()
    }

    /// Get all files currently being processed
    #[must_use]
    pub fn processing_files(&self) -> Vec<QueuedFile> {
        self.processing
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get all failed files
    #[must_use]
    pub fn failed_files(&self) -> Vec<QueuedFile> {
        self.failed
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Retry a failed file
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if:
    /// - File is not found in failed queue
    /// - Persistence saving fails
    pub async fn retry_failed(&self, file_id: Uuid) -> Result<()> {
        if let Some((_, mut file)) = self.failed.remove(&file_id) {
            file.retry_count = 0;
            file.last_error = None;
            file.queued_at = Utc::now();

            self.pending.write().push(file);

            tracing::info!(
                file_id = %file_id,
                "Retrying failed file"
            );

            self.update_stats();
            self.save_to_persistence().await?;
            Ok(())
        } else {
            Err(MonitorError::queue(format!(
                "Failed file not found: {file_id}"
            )))
        }
    }

    /// Clear all failed files
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if persistence saving fails
    pub fn clear_failed(&self) -> Result<usize> {
        let count = self.failed.len();
        self.failed.clear();

        tracing::info!(count, "Cleared failed files");

        self.update_stats();
        Ok(count)
    }

    /// Calculate priority based on configuration
    fn calculate_priority(&self, file_size: u64, modified_at: DateTime<Utc>) -> i32 {
        let mut priority = 0i32;

        if self.priority_by_age {
            #[allow(clippy::cast_possible_truncation)]
            let age_hours = Utc::now()
                .signed_duration_since(modified_at)
                .num_hours()
                .max(0) as i32;
            priority += age_hours; // Older files get higher priority
        }

        if self.priority_by_size {
            // Smaller files get higher priority (easier to process quickly)
            let size_mb = (file_size / (1024 * 1024)).max(1);
            #[allow(clippy::cast_possible_truncation)]
            let size_mb_i32 = size_mb as i32;
            priority += (1000 / size_mb_i32).max(1);
        }

        priority
    }

    /// Update internal statistics
    fn update_stats(&self) {
        let pending_count = self.pending.read().len();
        let processing_count = self.processing.len();
        let failed_count = self.failed.len();

        let oldest_queued = self.pending.read().iter().map(|f| f.queued_at).min();

        let mut stats = self.stats.write();
        stats.total_files = pending_count;
        stats.processing_files = processing_count;
        stats.failed_files = failed_count;
        stats.oldest_queued = oldest_queued;

        // Calculate average wait time for processing files
        if !self.processing.is_empty() {
            let total_wait: i64 = self
                .processing
                .iter()
                .map(|entry| {
                    Utc::now()
                        .signed_duration_since(entry.queued_at)
                        .num_seconds()
                })
                .sum();
            #[allow(clippy::cast_precision_loss)]
            {
                stats.average_wait_time = total_wait as f64 / processing_count as f64;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    /// Test queue enqueue and dequeue operations
    ///
    /// # Panics
    ///
    /// Panics if test setup fails (temp directory creation, file operations)
    async fn test_queue_enqueue_dequeue() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.mp3");
        tokio::fs::write(&test_file, b"test content").await.unwrap();

        let queue = FileQueue::new(100, None, false, false);

        // Test enqueue
        let file_id = queue.enqueue(test_file.clone()).await.unwrap();
        assert_eq!(queue.stats().total_files, 1);

        // Test dequeue
        let file = queue.dequeue().unwrap();
        assert_eq!(file.id, file_id);
        assert_eq!(file.path, test_file);
        assert_eq!(queue.stats().processing_files, 1);

        // Test completion
        queue.mark_completed(file_id).await.unwrap();
        assert_eq!(queue.stats().processing_files, 0);
        assert_eq!(queue.stats().total_processed, 1);
    }

    #[tokio::test]
    /// Test queue retry logic
    ///
    /// # Panics
    ///
    /// Panics if test setup fails (temp directory creation, file operations)
    async fn test_queue_retry_logic() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.mp3");
        tokio::fs::write(&test_file, b"test content").await.unwrap();

        let queue = FileQueue::new(100, None, false, false);

        let file_id = queue.enqueue(test_file).await.unwrap();
        let _file = queue.dequeue().unwrap();

        // Test retry
        let should_retry = queue
            .mark_failed(file_id, "test error".to_string(), 3)
            .await
            .unwrap();
        assert!(should_retry);
        assert_eq!(queue.stats().total_files, 1); // Back in pending queue

        // Test final failure
        let _file = queue.dequeue().unwrap();
        let should_retry = queue
            .mark_failed(file_id, "test error".to_string(), 1)
            .await
            .unwrap();
        assert!(!should_retry);
        assert_eq!(queue.stats().failed_files, 1);
    }

    #[tokio::test]
    async fn test_queue_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.mp3");
        let persist_file = temp_dir.path().join("queue.json");
        tokio::fs::write(&test_file, b"test content").await.unwrap();

        // Create queue with persistence
        let queue = FileQueue::new(100, Some(persist_file.clone()), false, false);
        let file_id = queue.enqueue(test_file.clone()).await.unwrap();
        queue.save_to_persistence().await.unwrap();

        // Create new queue and load persistence
        let queue2 = FileQueue::new(100, Some(persist_file), false, false);
        queue2.load_from_persistence().await.unwrap();
        assert_eq!(queue2.stats().total_files, 1);

        let file = queue2.dequeue().unwrap();
        assert_eq!(file.id, file_id);
        assert_eq!(file.path, test_file);
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let temp_dir = TempDir::new().unwrap();
        let test_file1 = temp_dir.path().join("test1.mp3");
        let test_file2 = temp_dir.path().join("test2.mp3");
        tokio::fs::write(&test_file1, b"test content 1")
            .await
            .unwrap();
        tokio::fs::write(&test_file2, b"test content 2")
            .await
            .unwrap();

        let queue = FileQueue::new(100, None, false, false);

        // Test initial stats
        let stats = queue.stats();
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.processing_files, 0);
        assert_eq!(stats.failed_files, 0);
        assert_eq!(stats.total_processed, 0);

        // Enqueue files
        let file1_id = queue.enqueue(test_file1).await.unwrap();
        let file2_id = queue.enqueue(test_file2).await.unwrap();
        assert_eq!(queue.stats().total_files, 2);

        // Start processing
        let _file1 = queue.dequeue().unwrap();
        let _file2 = queue.dequeue().unwrap();
        assert_eq!(queue.stats().processing_files, 2);
        assert_eq!(queue.stats().total_files, 0);

        // Complete one, fail the other
        queue.mark_completed(file1_id).await.unwrap();
        queue
            .mark_failed(file2_id, "test error".to_string(), 0)
            .await
            .unwrap();

        let stats = queue.stats();
        assert_eq!(stats.processing_files, 0);
        assert_eq!(stats.failed_files, 1);
        assert_eq!(stats.total_processed, 1);
    }

    #[tokio::test]
    async fn test_processing_and_failed_files() {
        let temp_dir = TempDir::new().unwrap();
        let test_file1 = temp_dir.path().join("test1.mp3");
        let test_file2 = temp_dir.path().join("test2.mp3");
        tokio::fs::write(&test_file1, b"test content 1")
            .await
            .unwrap();
        tokio::fs::write(&test_file2, b"test content 2")
            .await
            .unwrap();

        let queue = FileQueue::new(100, None, false, false);

        let file1_id = queue.enqueue(test_file1.clone()).await.unwrap();
        let file2_id = queue.enqueue(test_file2.clone()).await.unwrap();

        // Start processing
        let _file1 = queue.dequeue().unwrap();
        let _file2 = queue.dequeue().unwrap();

        // Check processing files
        let processing = queue.processing_files();
        assert_eq!(processing.len(), 2);
        assert!(processing.iter().any(|f| f.id == file1_id));
        assert!(processing.iter().any(|f| f.id == file2_id));

        // Fail one file
        queue
            .mark_failed(file1_id, "test error".to_string(), 0)
            .await
            .unwrap();

        // Check failed files
        let failed = queue.failed_files();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].id, file1_id);
        assert_eq!(failed[0].last_error, Some("test error".to_string()));
        assert_eq!(failed[0].retry_count, 1);
    }

    #[tokio::test]
    async fn test_retry_failed_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.mp3");
        tokio::fs::write(&test_file, b"test content").await.unwrap();

        let queue = FileQueue::new(100, None, false, false);
        let file_id = queue.enqueue(test_file.clone()).await.unwrap();

        // Process and fail
        let _file = queue.dequeue().unwrap();
        queue
            .mark_failed(file_id, "test error".to_string(), 0)
            .await
            .unwrap();
        assert_eq!(queue.stats().failed_files, 1);

        // Retry the failed file
        queue.retry_failed(file_id).await.unwrap();
        assert_eq!(queue.stats().failed_files, 0);
        assert_eq!(queue.stats().total_files, 1);

        // Should be able to dequeue again
        let retried_file = queue.dequeue().unwrap();
        assert_eq!(retried_file.id, file_id);
        assert_eq!(retried_file.retry_count, 0);
        assert_eq!(retried_file.last_error, None);
    }

    #[tokio::test]
    async fn test_queue_max_size() {
        let temp_dir = TempDir::new().unwrap();
        let queue = FileQueue::new(2, None, false, false); // Max size 2

        // Create test files
        for i in 0..3 {
            let test_file = temp_dir.path().join(format!("test{i}.mp3"));
            tokio::fs::write(&test_file, b"test content").await.unwrap();

            if i < 2 {
                // First two should succeed
                queue.enqueue(test_file).await.unwrap();
            } else {
                // Third should fail due to max size
                let result = queue.enqueue(test_file).await;
                assert!(result.is_err());
            }
        }

        assert_eq!(queue.stats().total_files, 2);
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let temp_dir = TempDir::new().unwrap();
        let queue = FileQueue::new(100, None, true, true); // Enable priority by age and size

        // Create files of different sizes and times
        let small_file = temp_dir.path().join("small.mp3");
        let large_file = temp_dir.path().join("large.mp3");

        tokio::fs::write(&small_file, b"small").await.unwrap();
        tokio::fs::write(&large_file, b"large file content here")
            .await
            .unwrap();

        // Enqueue in reverse priority order
        let small_id = queue.enqueue(small_file).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await; // Ensure different timestamps
        let large_id = queue.enqueue(large_file).await.unwrap();

        // Smaller, newer file should come first due to priority (smaller files have higher priority)
        let first_file = queue.dequeue().unwrap();
        assert_eq!(first_file.id, small_id);

        let second_file = queue.dequeue().unwrap();
        assert_eq!(second_file.id, large_id);
    }

    #[tokio::test]
    async fn test_error_handling() {
        let queue = FileQueue::new(100, None, false, false);
        let invalid_id = Uuid::new_v4();

        // Test marking non-existent file as completed
        let result = queue.mark_completed(invalid_id).await;
        assert!(result.is_err());

        // Test marking non-existent file as failed
        let result = queue.mark_failed(invalid_id, "error".to_string(), 3).await;
        assert!(result.is_err());

        // Test retrying non-existent failed file
        let result = queue.retry_failed(invalid_id).await;
        assert!(result.is_err());

        // Test enqueuing non-existent file
        let non_existent_file = std::path::PathBuf::from("/non/existent/file.mp3");
        let result = queue.enqueue(non_existent_file).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_queue_clear_operations() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.mp3");
        tokio::fs::write(&test_file, b"test content").await.unwrap();

        let queue = FileQueue::new(100, None, false, false);
        let file_id = queue.enqueue(test_file).await.unwrap();

        // Process and fail the file
        let _file = queue.dequeue().unwrap();
        queue
            .mark_failed(file_id, "test error".to_string(), 0)
            .await
            .unwrap();

        assert_eq!(queue.stats().failed_files, 1);
        assert_eq!(queue.failed_files().len(), 1);

        // Clear failed files
        let _ = queue.clear_failed();
        assert_eq!(queue.stats().failed_files, 0);
        assert_eq!(queue.failed_files().len(), 0);
    }

    #[test]
    fn test_queued_file_ordering() {
        let now = Utc::now();

        let file1 = QueuedFile {
            id: Uuid::new_v4(),
            path: "/test1.mp3".into(),
            size: 1000,
            queued_at: now,
            modified_at: now,
            priority: 1,
            retry_count: 0,
            last_error: None,
            metadata: FileMetadata {
                extension: Some("mp3".to_string()),
                stem: "test".to_string(),
                is_symlink: false,
                checksum: Some("abc123".to_string()),
            },
        };

        let file2 = QueuedFile {
            id: Uuid::new_v4(),
            path: "/test2.mp3".into(),
            size: 2000,
            queued_at: now,
            modified_at: now,
            priority: 2,
            retry_count: 0,
            last_error: None,
            metadata: FileMetadata {
                extension: Some("mp3".to_string()),
                stem: "test".to_string(),
                is_symlink: false,
                checksum: Some("abc123".to_string()),
            },
        };

        // Higher priority should come first
        assert!(file2 > file1);
        assert!(file1 < file2);
        assert_eq!(file1.cmp(&file2), Ordering::Less);
        assert_eq!(file2.cmp(&file1), Ordering::Greater);
        assert_eq!(file1.cmp(&file1), Ordering::Equal);
    }

    #[test]
    fn test_file_metadata_serialization() {
        let metadata = FileMetadata {
            extension: Some("mp3".to_string()),
            stem: "test_file".to_string(),
            is_symlink: false,
            checksum: Some("123abc456def".to_string()),
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: FileMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(metadata, deserialized);
    }

    #[test]
    fn test_queue_stats_default() {
        let stats = QueueStats {
            total_files: 0,
            processing_files: 0,
            failed_files: 0,
            average_wait_time: 0.0,
            oldest_queued: None,
            total_processed: 0,
        };

        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.processing_files, 0);
        assert_eq!(stats.failed_files, 0);
        assert_eq!(stats.average_wait_time, 0.0);
        assert!(stats.oldest_queued.is_none());
        assert_eq!(stats.total_processed, 0);
    }
}
