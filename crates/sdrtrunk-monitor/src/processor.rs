//! File processing implementation
//!
//! Handles the processing of queued MP3 files, including database integration,
//! file archiving, and error handling with retry logic.

use crate::{MonitorError, QueuedFile, Result, config::ProcessingConfig};
use chrono::Utc;
use sdrtrunk_database::models::RadioCallDb;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, types::Decimal};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::time::{Duration, timeout};
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

/// System information tuple (`system_id`, `system_label`)
type SystemInfo = (Option<String>, Option<String>);

/// Status of file processing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProcessingStatus {
    /// File is pending processing
    Pending,

    /// File is currently being processed
    Processing,

    /// File was processed successfully
    Completed,

    /// File processing failed
    Failed {
        /// Error message
        error: String,
        /// Number of retry attempts
        retry_count: u32,
    },

    /// File was archived
    Archived,

    /// File was skipped (e.g., already exists)
    Skipped {
        /// Reason for skipping
        reason: String,
    },
}

/// Result of processing a file
#[derive(Debug, Clone)]
pub struct ProcessingResult {
    /// File that was processed
    pub file: QueuedFile,

    /// Processing status
    pub status: ProcessingStatus,

    /// Database record ID (if created)
    pub record_id: Option<Uuid>,

    /// Archive path (if archived)
    pub archive_path: Option<PathBuf>,

    /// Processing duration
    pub processing_duration: Duration,

    /// Additional metadata
    pub metadata: Option<serde_json::Value>,
}

/// File processor that handles individual file processing
#[derive(Debug)]
pub struct FileProcessor {
    /// Database connection pool
    db_pool: Arc<PgPool>,

    /// Processing configuration
    config: ProcessingConfig,

    /// Archive directory
    archive_dir: PathBuf,

    /// Failed files directory
    _failed_dir: PathBuf,

    /// Temporary directory
    _temp_dir: PathBuf,
}

impl FileProcessor {
    /// Create a new file processor
    #[must_use]
    pub const fn new(
        db_pool: Arc<PgPool>,
        config: ProcessingConfig,
        archive_dir: PathBuf,
        failed_dir: PathBuf,
        temp_dir: PathBuf,
    ) -> Self {
        Self {
            db_pool,
            config,
            archive_dir,
            _failed_dir: failed_dir,
            _temp_dir: temp_dir,
        }
    }

    /// Process a single file
    #[instrument(skip(self), fields(file_id = %file.id, path = %file.path.display()))]
    pub async fn process_file(&self, file: QueuedFile) -> ProcessingResult {
        let start_time = std::time::Instant::now();

        info!("Starting file processing");

        // Create processing result
        let mut result = ProcessingResult {
            file: file.clone(),
            status: ProcessingStatus::Processing,
            record_id: None,
            archive_path: None,
            processing_duration: Duration::from_secs(0),
            metadata: None,
        };

        // Process with timeout
        let processing_result = timeout(
            self.config.processing_timeout(),
            self.process_file_internal(&file),
        )
        .await;

        result.processing_duration = start_time.elapsed();

        match processing_result {
            Ok(Ok(processing_status)) => {
                result.status = processing_status;
                info!(
                    status = ?result.status,
                    duration_ms = result.processing_duration.as_millis(),
                    "File processing completed"
                );
            }
            Ok(Err(e)) => {
                result.status = ProcessingStatus::Failed {
                    error: e.to_string(),
                    retry_count: file.retry_count,
                };
                error!(
                    error = %e,
                    duration_ms = result.processing_duration.as_millis(),
                    "File processing failed"
                );
            }
            Err(_) => {
                result.status = ProcessingStatus::Failed {
                    error: "Processing timeout".to_string(),
                    retry_count: file.retry_count,
                };
                error!(
                    timeout_seconds = self.config.processing_timeout_seconds,
                    "File processing timed out"
                );
            }
        }

        result.file = file.clone();
        result
    }

    /// Internal file processing logic
    /// Process a file internally
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if:
    /// - File cannot be read or is corrupted
    /// - Database operations fail
    /// - File archiving fails
    /// - Audio processing fails
    async fn process_file_internal(&self, file: &QueuedFile) -> Result<ProcessingStatus> {
        // Verify file still exists and is accessible
        if !file.path.exists() {
            return Ok(ProcessingStatus::Skipped {
                reason: "File no longer exists".to_string(),
            });
        }

        // Verify file integrity if configured
        if self.config.verify_file_integrity {
            self.verify_file_integrity(&file.path).await?;
        }

        // Check if file is already in database
        if let Some(existing_record) = self.find_existing_record(file).await? {
            warn!(
                record_id = %existing_record,
                "File already exists in database, skipping"
            );
            return Ok(ProcessingStatus::Skipped {
                reason: format!("Already exists in database: {existing_record}"),
            });
        }

        // Extract metadata from file
        let metadata = self.extract_file_metadata(&file.path).await?;

        // Create database record
        let record_id = self.create_database_record(file, &metadata).await?;

        debug!(record_id = %record_id, "Created database record");

        // Archive file if configured
        let _archive_path = if self.config.move_after_processing {
            Some(self.archive_file(file).await?)
        } else if self.config.delete_after_processing {
            self.delete_file(&file.path).await?;
            None
        } else {
            None
        };

        Ok(ProcessingStatus::Completed)
    }

    /// Verify file integrity (basic checks)
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if:
    /// - File cannot be opened or read
    /// - File has invalid MP3 format (for MP3 files)
    /// - File is corrupted or truncated
    async fn verify_file_integrity(&self, path: &Path) -> Result<()> {
        debug!("Verifying file integrity");

        // Check file is readable
        let mut file = fs::File::open(path).await?;

        // For MP3 files, check for basic MP3 header
        if let Some(ext) = path.extension().and_then(|e| e.to_str())
            && ext.eq_ignore_ascii_case("mp3")
        {
            Self::verify_mp3_format(&mut file, path).await?;
        }

        Ok(())
    }

    /// Verify MP3 file format
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if the file has invalid MP3 format
    async fn verify_mp3_format(file: &mut fs::File, path: &Path) -> Result<()> {
        use tokio::io::AsyncReadExt;

        let mut buffer = [0u8; 4];
        if file.read_exact(&mut buffer).await.is_ok() {
            // Check for MP3 sync word (0xFFE or 0xFFF at start)
            if buffer[0] == 0xFF && (buffer[1] & 0xE0) == 0xE0 {
                debug!("MP3 header verification passed");
                return Ok(());
            }

            // Check for ID3 tag
            if &buffer[0..3] == b"ID3" {
                debug!("ID3 tag found, assuming valid MP3");
                return Ok(());
            }
        }

        Err(MonitorError::invalid_file(path, "Invalid MP3 file format"))
    }

    /// Check if file already exists in database
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if database query fails
    async fn find_existing_record(&self, file: &QueuedFile) -> Result<Option<Uuid>> {
        let file_path_str = file.path.to_string_lossy();
        let filename = file.path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        let record = sqlx::query_as::<_, (uuid::Uuid,)>(
            "SELECT id FROM radio_calls WHERE audio_file_path = $1 OR audio_filename = $2 LIMIT 1",
        )
        .bind(file_path_str.as_ref())
        .bind(filename)
        .fetch_optional(self.db_pool.as_ref())
        .await?;

        Ok(record.map(|r| r.0))
    }

    /// Extract metadata from the file
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if:
    /// - File metadata cannot be read
    /// - File modification time cannot be determined
    ///
    /// # Panics
    ///
    /// Panics if the file modification time is before the Unix epoch
    /// (should not happen on modern systems)
    async fn extract_file_metadata(&self, path: &Path) -> Result<serde_json::Value> {
        debug!("Extracting file metadata");

        let metadata = fs::metadata(path).await?;

        let mut meta = serde_json::json!({
            "file_size": metadata.len(),
            "modified_at": metadata.modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .expect("File modification time is before Unix epoch")
                .as_secs(),
            "file_type": "audio/mpeg", // Assuming MP3 for now
            "processor_version": env!("CARGO_PKG_VERSION"),
            "processed_at": Utc::now().timestamp(),
        });

        // Add file-specific metadata
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            meta["filename"] = serde_json::Value::String(filename.to_string());
            meta["file_stem"] = serde_json::Value::String(
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string(),
            );

            // Try to extract system info from filename if it follows SDRTrunk naming convention
            if let Some(system_info) = Self::extract_system_info_from_filename(filename) {
                meta["extracted_system_info"] = system_info;
            }
        }

        // For MP3 files, we could extract audio metadata here
        // This is a simplified version - in production you might want to use a proper audio library
        meta["duration_estimate"] = self.estimate_mp3_duration(path).await?;

        Ok(meta)
    }

    /// Create database record for the processed file
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if:
    /// - Database transaction fails
    /// - Required metadata fields are missing
    /// - SQL query execution fails
    async fn create_database_record(
        &self,
        file: &QueuedFile,
        metadata: &serde_json::Value,
    ) -> Result<Uuid> {
        debug!("Creating database record");

        let record = Self::build_radio_call_record(file, metadata);
        self.insert_radio_call_record(&record).await?;

        info!(record_id = %record.id, "Created database record");
        Ok(record.id)
    }

    /// Build a `RadioCallDb` record from file and metadata
    fn build_radio_call_record(file: &QueuedFile, metadata: &serde_json::Value) -> RadioCallDb {
        let record_id = Uuid::new_v4();
        let now = Utc::now();

        let filename = file
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .map(ToString::to_string);

        let file_path_str = file.path.to_string_lossy().to_string();
        let (system_id, system_label) = Self::extract_system_info(metadata, filename.as_ref());
        let duration_seconds = Self::extract_duration_from_metadata(metadata);

        RadioCallDb {
            id: record_id,
            created_at: now,
            call_timestamp: file.modified_at,
            system_id: system_id.unwrap_or_else(|| "unknown".to_string()),
            system_label,
            frequency: None,
            talkgroup_id: None,
            talkgroup_label: None,
            talkgroup_group: None,
            talkgroup_tag: None,
            source_radio_id: None,
            talker_alias: None,
            audio_filename: filename,
            audio_file_path: Some(file_path_str),
            audio_size_bytes: Some(i64::try_from(file.size).unwrap_or(0)),
            audio_content_type: Some("audio/mpeg".to_string()),
            duration_seconds,
            transcription_text: None,
            transcription_confidence: None,
            transcription_language: None,
            transcription_status: Some("pending".to_string()),
            speaker_segments: None,
            speaker_count: None,
            patches: None,
            frequencies: None,
            sources: None,
            upload_ip: None,
            upload_timestamp: now,
            upload_api_key_id: Some("file-monitor".to_string()),
        }
    }

    /// Extract duration from metadata
    fn extract_duration_from_metadata(metadata: &serde_json::Value) -> Option<Decimal> {
        metadata
            .get("duration_estimate")
            .and_then(serde_json::Value::as_f64)
            .and_then(Decimal::from_f64_retain)
    }

    /// Insert `RadioCallDb` record into database
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if database insert fails
    async fn insert_radio_call_record(&self, record: &RadioCallDb) -> Result<()> {
        let rows_affected = sqlx::query(
            "INSERT INTO radio_calls (
                id, created_at, call_timestamp, system_id, system_label,
                frequency, talkgroup_id, talkgroup_label, talkgroup_group, talkgroup_tag,
                source_radio_id, talker_alias, audio_filename, audio_file_path,
                audio_size_bytes, audio_content_type, duration_seconds,
                transcription_text, transcription_confidence, transcription_language,
                transcription_status, speaker_segments, speaker_count,
                patches, frequencies, sources, upload_ip, upload_timestamp, upload_api_key_id
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24, $25, $26, $27, $28, $29
            )",
        )
        .bind(record.id)
        .bind(record.created_at)
        .bind(record.call_timestamp)
        .bind(&record.system_id)
        .bind(&record.system_label)
        .bind(record.frequency)
        .bind(record.talkgroup_id)
        .bind(&record.talkgroup_label)
        .bind(&record.talkgroup_group)
        .bind(&record.talkgroup_tag)
        .bind(record.source_radio_id)
        .bind(&record.talker_alias)
        .bind(&record.audio_filename)
        .bind(&record.audio_file_path)
        .bind(record.audio_size_bytes)
        .bind(&record.audio_content_type)
        .bind(record.duration_seconds)
        .bind(&record.transcription_text)
        .bind(record.transcription_confidence)
        .bind(&record.transcription_language)
        .bind(&record.transcription_status)
        .bind(&record.speaker_segments)
        .bind(record.speaker_count)
        .bind(&record.patches)
        .bind(&record.frequencies)
        .bind(&record.sources)
        .bind(record.upload_ip)
        .bind(record.upload_timestamp)
        .bind(&record.upload_api_key_id)
        .execute(self.db_pool.as_ref())
        .await?;

        if rows_affected.rows_affected() == 0 {
            return Err(MonitorError::processing(
                PathBuf::from(record.audio_file_path.as_ref().map_or("unknown", |s| s)),
                "Failed to insert database record",
            ));
        }

        Ok(())
    }

    /// Archive a processed file
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if:
    /// - Cannot create archive directories
    /// - File copy/move operation fails
    /// - Archive path generation fails
    #[allow(clippy::cognitive_complexity)]
    async fn archive_file(&self, file: &QueuedFile) -> Result<PathBuf> {
        debug!("Archiving file");

        // Ensure archive directory exists
        fs::create_dir_all(&self.archive_dir).await?;

        // Generate archive path
        let archive_path = self.generate_archive_path(file)?;

        // Ensure archive subdirectory exists
        if let Some(parent) = archive_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Move or copy file to archive
        if let Err(e) = fs::rename(&file.path, &archive_path).await {
            // If rename fails (cross-device), try copy + remove
            warn!(
                error = %e,
                "Failed to rename file to archive, trying copy"
            );

            fs::copy(&file.path, &archive_path).await?;
            fs::remove_file(&file.path).await?;
        }

        info!(
            original = %file.path.display(),
            archived = %archive_path.display(),
            "File archived successfully"
        );

        Ok(archive_path)
    }

    /// Generate archive path for a file
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if path generation fails
    fn generate_archive_path(&self, file: &QueuedFile) -> Result<PathBuf> {
        let mut archive_path = self.archive_dir.clone();

        // Organize by date if configured
        if self
            .archive_dir
            .to_string_lossy()
            .contains("organize_by_date")
        {
            let date = file.modified_at.format("%Y/%m/%d");
            archive_path = archive_path.join(date.to_string());
        }

        // Organize by system if configured
        // This would need system extraction logic
        // For now, just use the original filename

        let filename = file
            .path
            .file_name()
            .ok_or_else(|| MonitorError::archive(file.path.clone(), "No filename"))?;

        archive_path = archive_path.join(filename);

        // Handle filename conflicts
        let mut counter = 1;
        let mut final_path = archive_path.clone();

        while final_path.exists() {
            if let Some(stem) = archive_path.file_stem().and_then(|s| s.to_str()) {
                if let Some(ext) = archive_path.extension().and_then(|s| s.to_str()) {
                    final_path = archive_path.with_file_name(format!("{stem}_{counter}.{ext}"));
                } else {
                    final_path = archive_path.with_file_name(format!("{stem}_{counter}"));
                }
            } else {
                final_path = archive_path.with_extension(counter.to_string());
            }
            counter += 1;
        }

        Ok(final_path)
    }

    /// Delete a file
    /// Delete a file from filesystem
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if file deletion fails
    async fn delete_file(&self, path: &Path) -> Result<()> {
        debug!(path = %path.display(), "Deleting file");

        fs::remove_file(path).await?;

        info!(path = %path.display(), "File deleted");
        Ok(())
    }

    /// Extract system information from metadata or filename
    fn extract_system_info(metadata: &serde_json::Value, filename: Option<&String>) -> SystemInfo {
        // First check if metadata contains extracted system info
        if let Some(system_info) = metadata.get("extracted_system_info") {
            let system_id = system_info
                .get("system_id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let system_label = system_info
                .get("system_label")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            return (system_id, system_label);
        }

        // Fallback to filename-based extraction
        if let Some(filename) = filename
            && let Some(system_info) = Self::extract_system_info_from_filename(filename)
        {
            let system_id = system_info
                .get("system_id")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let system_label = system_info
                .get("system_label")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            return (system_id, system_label);
        }

        (None, None)
    }

    /// Extract system information from filename (`SDRTrunk` naming convention)
    fn extract_system_info_from_filename(filename: &str) -> Option<serde_json::Value> {
        // SDRTrunk typically uses patterns like:
        // SystemName_TG123_20240101_120000.mp3
        // Try to parse common patterns

        let parts: Vec<&str> = filename.split('_').collect();
        if parts.len() >= 2 {
            let system_name = parts[0];

            // Look for talkgroup pattern (TG followed by numbers)
            let talkgroup = parts
                .iter()
                .find(|part| {
                    part.starts_with("TG") && part[2..].chars().all(|c| c.is_ascii_digit())
                })
                .and_then(|tg| tg[2..].parse::<u32>().ok());

            let mut info = serde_json::json!({
                "system_id": system_name,
                "system_label": system_name,
            });

            if let Some(tg) = talkgroup {
                info["talkgroup_id"] = serde_json::Value::Number(tg.into());
            }

            return Some(info);
        }

        None
    }

    /// Estimate MP3 duration (simplified - in production use proper audio library)
    /// Estimate MP3 file duration based on file size
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if file metadata cannot be read
    async fn estimate_mp3_duration(&self, path: &Path) -> Result<serde_json::Value> {
        let metadata = fs::metadata(path).await?;
        #[allow(clippy::cast_precision_loss)]
        let file_size = metadata.len() as f64;

        // Very rough estimate: assume 128kbps MP3
        // Duration ≈ file_size_bytes * 8 / bitrate_bps
        let estimated_duration = file_size * 8.0 / 128_000.0;

        Ok(serde_json::Value::Number(
            serde_json::Number::from_f64(estimated_duration)
                .unwrap_or_else(|| serde_json::Number::from(0)),
        ))
    }

    /// Get database connection pool
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn db_pool(&self) -> &Arc<PgPool> {
        &self.db_pool
    }
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;
    use crate::{QueuedFile, config::ProcessingConfig};
    use tempfile::tempdir;
    use tokio::io::AsyncWriteExt;
    use uuid::Uuid;

    // Test helper functions
    async fn create_test_mp3_file(dir: &Path, filename: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(filename);
        let mut file = fs::File::create(&path).await.unwrap();
        file.write_all(content).await.unwrap();
        file.sync_all().await.unwrap();
        path
    }

    fn create_test_config() -> ProcessingConfig {
        ProcessingConfig {
            processing_interval_seconds: 1,
            processing_workers: 2,
            max_retry_attempts: 3,
            retry_delay_seconds: 1,
            processing_timeout_seconds: 30,
            move_after_processing: false,
            delete_after_processing: false,
            verify_file_integrity: true,
        }
    }

    fn create_test_queued_file(path: PathBuf) -> QueuedFile {
        use crate::queue::FileMetadata;
        QueuedFile {
            id: Uuid::new_v4(),
            path: path.clone(),
            size: 1024,
            queued_at: Utc::now(),
            modified_at: Utc::now(),
            priority: 0,
            retry_count: 0,
            last_error: None,
            metadata: FileMetadata {
                extension: path.extension().and_then(|e| e.to_str()).map(String::from),
                stem: path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("test")
                    .to_string(),
                is_symlink: false,
                checksum: None,
            },
        }
    }

    // Valid MP3 header bytes for testing
    const VALID_MP3_HEADER: &[u8] = &[0xFF, 0xE3, 0x18, 0xC4]; // MPEG-1 Layer III, 128kbps, 44.1kHz
    const ID3_HEADER: &[u8] = b"ID3\x03\x00\x00\x00\x00\x00\x00"; // ID3v2 header
    const INVALID_HEADER: &[u8] = &[0x00, 0x01, 0x02, 0x03];

    #[test]
    fn test_processing_status_variants() {
        // Test all ProcessingStatus variants
        let pending = ProcessingStatus::Pending;
        assert!(matches!(pending, ProcessingStatus::Pending));

        let processing = ProcessingStatus::Processing;
        assert!(matches!(processing, ProcessingStatus::Processing));

        let completed = ProcessingStatus::Completed;
        assert!(matches!(completed, ProcessingStatus::Completed));

        let failed = ProcessingStatus::Failed {
            error: "Test error".to_string(),
            retry_count: 2,
        };
        if let ProcessingStatus::Failed { error, retry_count } = failed {
            assert_eq!(error, "Test error");
            assert_eq!(retry_count, 2);
        }

        let archived = ProcessingStatus::Archived;
        assert!(matches!(archived, ProcessingStatus::Archived));

        let skipped = ProcessingStatus::Skipped {
            reason: "Already exists".to_string(),
        };
        if let ProcessingStatus::Skipped { reason } = skipped {
            assert_eq!(reason, "Already exists");
        }
    }

    #[test]
    fn test_processing_result_creation() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.mp3");
        let queued_file = create_test_queued_file(file_path);

        let result = ProcessingResult {
            file: queued_file.clone(),
            status: ProcessingStatus::Completed,
            record_id: Some(Uuid::new_v4()),
            archive_path: Some(PathBuf::from("/archive/test.mp3")),
            processing_duration: Duration::from_millis(500),
            metadata: Some(serde_json::json!({"test": "data"})),
        };

        assert!(matches!(result.status, ProcessingStatus::Completed));
        assert!(result.record_id.is_some());
        assert!(result.archive_path.is_some());
        assert_eq!(result.processing_duration, Duration::from_millis(500));
        assert!(result.metadata.is_some());
        assert_eq!(result.file.id, queued_file.id);
    }

    #[tokio::test]
    async fn test_verify_mp3_format_valid_header() {
        let temp_dir = tempdir().unwrap();
        let mp3_path = create_test_mp3_file(temp_dir.path(), "test.mp3", VALID_MP3_HEADER).await;
        let mut file = fs::File::open(&mp3_path).await.unwrap();

        let result = FileProcessor::verify_mp3_format(&mut file, &mp3_path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_verify_mp3_format_id3_header() {
        let temp_dir = tempdir().unwrap();
        let mp3_path = create_test_mp3_file(temp_dir.path(), "test.mp3", ID3_HEADER).await;
        let mut file = fs::File::open(&mp3_path).await.unwrap();

        let result = FileProcessor::verify_mp3_format(&mut file, &mp3_path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_verify_mp3_format_invalid_header() {
        let temp_dir = tempdir().unwrap();
        let mp3_path = create_test_mp3_file(temp_dir.path(), "test.mp3", INVALID_HEADER).await;
        let mut file = fs::File::open(&mp3_path).await.unwrap();

        let result = FileProcessor::verify_mp3_format(&mut file, &mp3_path).await;
        assert!(result.is_err());

        if let Err(e) = result {
            assert!(e.to_string().contains("Invalid MP3 file format"));
        }
    }

    #[tokio::test]
    async fn test_verify_mp3_format_empty_file() {
        let temp_dir = tempdir().unwrap();
        let mp3_path = create_test_mp3_file(temp_dir.path(), "empty.mp3", &[]).await;
        let mut file = fs::File::open(&mp3_path).await.unwrap();

        let result = FileProcessor::verify_mp3_format(&mut file, &mp3_path).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_system_info_from_filename() {
        // Test valid SDRTrunk filename pattern: SystemName_TG123_20240101_120000.mp3
        let filename = "Police_TG12345_20231201_123456.mp3";
        let result = FileProcessor::extract_system_info_from_filename(filename);

        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info["system_id"], "Police");
        assert_eq!(info["system_label"], "Police");
        assert_eq!(info["talkgroup_id"], 12345);
    }

    #[test]
    fn test_extract_system_info_from_filename_invalid() {
        // Test invalid filename (single part)
        let filename = "invalid.mp3";
        let result = FileProcessor::extract_system_info_from_filename(filename);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_system_info_from_filename_edge_cases() {
        // Test filename with no talkgroup - should still work, just without talkgroup_id
        let filename_no_tg = "System_20231201_123456.mp3";
        let result = FileProcessor::extract_system_info_from_filename(filename_no_tg);
        assert!(result.is_some()); // Should work without talkgroup
        let info = result.unwrap();
        assert_eq!(info["system_id"], "System");
        assert!(!info.as_object().unwrap().contains_key("talkgroup_id"));

        // Test empty filename
        let result_empty = FileProcessor::extract_system_info_from_filename("");
        assert!(result_empty.is_none());
    }

    #[test]
    fn test_system_info_type_alias() {
        // Test that SystemInfo type alias works
        let system_info: SystemInfo = (
            Some("test_system".to_string()),
            Some("Test System".to_string()),
        );
        assert_eq!(system_info.0, Some("test_system".to_string()));
        assert_eq!(system_info.1, Some("Test System".to_string()));

        // Test None cases
        let empty_info: SystemInfo = (None, None);
        assert!(empty_info.0.is_none());
        assert!(empty_info.1.is_none());
    }

    #[test]
    fn test_processing_config_methods() {
        let config = ProcessingConfig {
            processing_interval_seconds: 5,
            processing_workers: 4,
            max_retry_attempts: 2,
            retry_delay_seconds: 10,
            processing_timeout_seconds: 60,
            move_after_processing: true,
            delete_after_processing: false,
            verify_file_integrity: true,
        };

        assert_eq!(config.processing_interval(), Duration::from_secs(5));
        assert_eq!(config.retry_delay(), Duration::from_secs(10));
        assert_eq!(config.processing_timeout(), Duration::from_secs(60));

        assert_eq!(config.processing_workers, 4);
        assert_eq!(config.max_retry_attempts, 2);
        assert!(config.move_after_processing);
        assert!(!config.delete_after_processing);
        assert!(config.verify_file_integrity);
    }

    #[test]
    fn test_processing_status_equality() {
        let status1 = ProcessingStatus::Pending;
        let status2 = ProcessingStatus::Pending;
        assert_eq!(status1, status2);

        let status3 = ProcessingStatus::Processing;
        assert_ne!(status1, status3);

        let failed1 = ProcessingStatus::Failed {
            error: "Error".to_string(),
            retry_count: 1,
        };
        let failed2 = ProcessingStatus::Failed {
            error: "Error".to_string(),
            retry_count: 1,
        };
        assert_eq!(failed1, failed2);

        let failed3 = ProcessingStatus::Failed {
            error: "Different Error".to_string(),
            retry_count: 1,
        };
        assert_ne!(failed1, failed3);
    }

    #[test]
    fn test_processing_status_serialization() {
        let status = ProcessingStatus::Failed {
            error: "Test error message".to_string(),
            retry_count: 3,
        };

        // Test serialization
        let serialized = serde_json::to_string(&status).unwrap();
        assert!(serialized.contains("Test error message"));
        assert!(serialized.contains('3'));

        // Test deserialization
        let deserialized: ProcessingStatus = serde_json::from_str(&serialized).unwrap();
        assert_eq!(status, deserialized);
    }

    #[test]
    fn test_queued_file_creation() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_file.mp3");
        let queued_file = create_test_queued_file(file_path.clone());

        assert_eq!(queued_file.path, file_path);
        assert_eq!(queued_file.size, 1024);
        assert_eq!(queued_file.retry_count, 0);
        assert!(!queued_file.id.is_nil());
    }

    // Mock implementations for testing (without database)
    #[test]
    fn test_filename_parsing_edge_cases() {
        // Test various filename formats
        let test_cases = vec![
            ("System_20231201_123456.mp3", true), // Valid without talkgroup
            ("System_TG123_20231201_123456.mp3", true), // Valid with talkgroup
            ("invalid.txt", false),               // Single part
            ("", false),                          // Empty
            ("System_INVALID_20231201.mp3", true), // Valid system, invalid talkgroup format
        ];

        for (filename, should_extract) in test_cases {
            let result = FileProcessor::extract_system_info_from_filename(filename);
            if should_extract {
                assert!(result.is_some(), "Failed to extract from: {filename}");
            } else {
                assert!(result.is_none(), "Unexpected extraction from: {filename}");
            }
        }
    }

    // Test duration estimation (simplified)
    #[tokio::test]
    async fn test_mp3_duration_estimation_concept() {
        // Test the concept behind duration estimation
        let file_size = 128_000_u64; // 128KB
        let bitrate = 128_000.0; // 128kbps

        // Duration = file_size_bytes * 8 / bitrate_bps
        let expected_duration = file_size as f64 * 8.0 / bitrate;

        assert!((expected_duration - 8.0).abs() < 0.1); // ~8 seconds for 128KB at 128kbps
    }

    #[test]
    fn test_debug_implementations() {
        // Test Debug implementations
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("debug_test.mp3");
        let queued_file = create_test_queued_file(file_path);

        let result = ProcessingResult {
            file: queued_file,
            status: ProcessingStatus::Processing,
            record_id: None,
            archive_path: None,
            processing_duration: Duration::from_millis(100),
            metadata: None,
        };

        let debug_output = format!("{result:?}");
        assert!(debug_output.contains("ProcessingResult"));
        assert!(debug_output.contains("Processing"));
    }

    #[test]
    fn test_config_defaults() {
        let config = create_test_config();

        // Test that our test config has reasonable defaults
        assert!(config.processing_interval_seconds > 0);
        assert!(config.processing_workers > 0);
        assert!(config.max_retry_attempts > 0);
        assert!(config.processing_timeout_seconds > 0);
    }

    #[test]
    fn test_status_clone() {
        let original = ProcessingStatus::Skipped {
            reason: "Test reason".to_string(),
        };
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    // Test error scenarios conceptually
    #[test]
    fn test_error_handling_concepts() {
        // Test that we handle various error conditions appropriately
        let error_message = "File processing failed";
        let status = ProcessingStatus::Failed {
            error: error_message.to_string(),
            retry_count: 2,
        };

        if let ProcessingStatus::Failed { error, retry_count } = status {
            assert_eq!(error, error_message);
            assert_eq!(retry_count, 2);
        } else {
            panic!("Expected Failed status");
        }
    }

    #[test]
    fn test_processing_status_additional_variants() {
        // Test all ProcessingStatus variants
        let pending = ProcessingStatus::Pending;
        let processing = ProcessingStatus::Processing;
        let completed = ProcessingStatus::Completed;
        let archived = ProcessingStatus::Archived;

        assert_eq!(pending, ProcessingStatus::Pending);
        assert_eq!(processing, ProcessingStatus::Processing);
        assert_eq!(completed, ProcessingStatus::Completed);
        assert_eq!(archived, ProcessingStatus::Archived);

        let failed = ProcessingStatus::Failed {
            error: "Test error".to_string(),
            retry_count: 1,
        };
        let skipped = ProcessingStatus::Skipped {
            reason: "Already processed".to_string(),
        };

        if let ProcessingStatus::Failed { error, retry_count } = failed {
            assert_eq!(error, "Test error");
            assert_eq!(retry_count, 1);
        }

        if let ProcessingStatus::Skipped { reason } = skipped {
            assert_eq!(reason, "Already processed");
        }
    }

    #[test]
    fn test_processing_status_debug() {
        let statuses = vec![
            ProcessingStatus::Pending,
            ProcessingStatus::Processing,
            ProcessingStatus::Completed,
            ProcessingStatus::Archived,
            ProcessingStatus::Failed {
                error: "Debug error".to_string(),
                retry_count: 3,
            },
            ProcessingStatus::Skipped {
                reason: "Debug skip".to_string(),
            },
        ];

        for status in &statuses {
            let debug_str = format!("{status:?}");
            assert!(!debug_str.is_empty());
            // Check that it contains expected debug info
            match status {
                ProcessingStatus::Pending => assert!(debug_str.contains("Pending")),
                ProcessingStatus::Processing => assert!(debug_str.contains("Processing")),
                ProcessingStatus::Completed => assert!(debug_str.contains("Completed")),
                ProcessingStatus::Archived => assert!(debug_str.contains("Archived")),
                ProcessingStatus::Failed { .. } => assert!(debug_str.contains("Failed")),
                ProcessingStatus::Skipped { .. } => assert!(debug_str.contains("Skipped")),
            }
        }
    }

    #[test]
    fn test_processing_result_detailed() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("result_test.mp3");
        let queued_file = create_test_queued_file(file_path);

        // Test successful result
        let success_result = ProcessingResult {
            file: queued_file.clone(),
            status: ProcessingStatus::Completed,
            record_id: Some(Uuid::new_v4()),
            archive_path: Some(temp_dir.path().join("archived.mp3")),
            processing_duration: Duration::from_millis(1500),
            metadata: Some(serde_json::json!({"test": "value"})),
        };

        assert_eq!(success_result.status, ProcessingStatus::Completed);
        assert!(success_result.record_id.is_some());
        assert!(success_result.archive_path.is_some());
        assert_eq!(
            success_result.processing_duration,
            Duration::from_millis(1500)
        );
        assert!(success_result.metadata.is_some());

        // Test failed result
        let failed_result = ProcessingResult {
            file: queued_file,
            status: ProcessingStatus::Failed {
                error: "Processing failed".to_string(),
                retry_count: 2,
            },
            record_id: None,
            archive_path: None,
            processing_duration: Duration::from_millis(500),
            metadata: None,
        };

        if let ProcessingStatus::Failed { error, retry_count } = &failed_result.status {
            assert_eq!(error, "Processing failed");
            assert_eq!(*retry_count, 2);
        }
        assert!(failed_result.record_id.is_none());
        assert!(failed_result.archive_path.is_none());
    }

    #[test]
    fn test_system_info_types() {
        // Test SystemInfo tuple type
        let system_with_label: SystemInfo =
            (Some("System1".to_string()), Some("Label1".to_string()));
        let system_without_label: SystemInfo = (Some("System2".to_string()), None);
        let empty_system: SystemInfo = (None, None);

        assert_eq!(system_with_label.0, Some("System1".to_string()));
        assert_eq!(system_with_label.1, Some("Label1".to_string()));

        assert_eq!(system_without_label.0, Some("System2".to_string()));
        assert!(system_without_label.1.is_none());

        assert!(empty_system.0.is_none());
        assert!(empty_system.1.is_none());
    }

    #[test]
    fn test_filename_variations() {
        // Test various filename patterns
        let test_cases = vec![
            ("System_20231201_123456.mp3", true),
            ("System_TG123_20231201_123456.mp3", true),
            ("LongSystemName_TG999999_20231231_235959.mp3", true),
            ("Sys_20230101_000001.mp3", true),
            ("System_.mp3", true),  // Has 2 parts when split by _
            ("Invalid.mp3", false), // Single part
            ("", false),
            ("NoExtension", false), // Single part
        ];

        for (filename, should_parse) in test_cases {
            let result = FileProcessor::extract_system_info_from_filename(filename);
            if should_parse {
                assert!(result.is_some(), "Failed to parse: {filename}");
            } else {
                assert!(result.is_none(), "Unexpectedly parsed: {filename}");
            }
        }
    }

    #[test]
    fn test_config_duration_methods() {
        let config = ProcessingConfig {
            processing_interval_seconds: 10,
            processing_workers: 2,
            max_retry_attempts: 3,
            retry_delay_seconds: 5,
            processing_timeout_seconds: 120,
            move_after_processing: true,
            delete_after_processing: false,
            verify_file_integrity: true,
        };

        // Test duration methods
        assert_eq!(config.processing_interval(), Duration::from_secs(10));
        assert_eq!(config.retry_delay(), Duration::from_secs(5));
        assert_eq!(config.processing_timeout(), Duration::from_secs(120));

        // Test configuration values
        assert_eq!(config.processing_workers, 2);
        assert_eq!(config.max_retry_attempts, 3);
        assert!(config.move_after_processing);
        assert!(!config.delete_after_processing);
        assert!(config.verify_file_integrity);
    }

    #[test]
    fn test_processing_status_serialization_roundtrip() {
        let statuses = vec![
            ProcessingStatus::Pending,
            ProcessingStatus::Processing,
            ProcessingStatus::Completed,
            ProcessingStatus::Archived,
            ProcessingStatus::Failed {
                error: "Serialization test error".to_string(),
                retry_count: 5,
            },
            ProcessingStatus::Skipped {
                reason: "Serialization test skip".to_string(),
            },
        ];

        for status in statuses {
            // Test serialization
            let serialized = serde_json::to_string(&status).unwrap();
            assert!(!serialized.is_empty());

            // Test deserialization
            let deserialized: ProcessingStatus = serde_json::from_str(&serialized).unwrap();
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn test_queued_file_variations() {
        let temp_dir = tempdir().unwrap();

        // Test with different file extensions
        let mp3_file = create_test_queued_file(temp_dir.path().join("test.mp3"));
        let wav_file = create_test_queued_file(temp_dir.path().join("test.wav"));
        let flac_file = create_test_queued_file(temp_dir.path().join("test.flac"));

        assert!(mp3_file.path.to_string_lossy().ends_with(".mp3"));
        assert!(wav_file.path.to_string_lossy().ends_with(".wav"));
        assert!(flac_file.path.to_string_lossy().ends_with(".flac"));

        // Test with different sizes
        for &size in &[0, 1024, 1_048_576, 104_857_600] {
            // 0B, 1KB, 1MB, 100MB
            let mut queued_file =
                create_test_queued_file(temp_dir.path().join(format!("test_{size}.mp3")));
            queued_file.size = size;
            let file = queued_file;
            assert_eq!(file.size, size);
            assert!(!file.id.is_nil());
        }
    }

    #[test]
    fn test_duration_calculations() {
        // Test MP3 duration estimation concepts
        let test_cases = vec![
            (128_000_u64, 128_000.0, 8.0),  // 128KB at 128kbps = ~8 seconds
            (256_000_u64, 128_000.0, 16.0), // 256KB at 128kbps = ~16 seconds
            (64_000_u64, 64_000.0, 8.0),    // 64KB at 64kbps = ~8 seconds
            (512_000_u64, 256_000.0, 16.0), // 512KB at 256kbps = ~16 seconds
        ];

        for (file_size, bitrate, expected_duration) in test_cases {
            // Duration = file_size_bytes * 8 / bitrate_bps
            let calculated_duration = file_size as f64 * 8.0 / bitrate;
            assert!(
                (calculated_duration - expected_duration).abs() < 0.1,
                "Expected ~{expected_duration} seconds for {file_size}B at {bitrate}bps, got {calculated_duration}"
            );
        }
    }

    #[test]
    fn test_metadata_handling() {
        // Test metadata creation and manipulation
        let metadata_values = vec![
            serde_json::json!({"test": "value"}),
            serde_json::json!({"duration": 30.5, "bitrate": 128_000}),
            serde_json::json!({"error": "processing failed", "retry": 3}),
            serde_json::json!({"filename": "test.mp3", "size": 1024}),
        ];

        for metadata in metadata_values {
            let temp_dir = tempdir().unwrap();
            let file = create_test_queued_file(temp_dir.path().join("metadata_test.mp3"));

            let result = ProcessingResult {
                file,
                status: ProcessingStatus::Completed,
                record_id: Some(Uuid::new_v4()),
                archive_path: None,
                processing_duration: Duration::from_millis(100),
                metadata: Some(metadata.clone()),
            };

            assert!(result.metadata.is_some());
            assert_eq!(result.metadata.unwrap(), metadata);
        }
    }

    #[test]
    fn test_system_info_extraction_basic() {
        // Test basic system info extraction logic
        let valid_filename = "Police_TG123_20240101.mp3";
        let result = FileProcessor::extract_system_info_from_filename(valid_filename);

        assert!(result.is_some(), "Should extract info from valid filename");
        let info = result.unwrap();
        assert_eq!(info["system_id"].as_str(), Some("Police"));
        assert_eq!(info["system_label"].as_str(), Some("Police"));

        // Test invalid filename
        let invalid_filename = "NoUnderscores.mp3";
        let result = FileProcessor::extract_system_info_from_filename(invalid_filename);
        assert!(
            result.is_none(),
            "Should not extract info from invalid filename"
        );

        // Test empty filename
        let empty_filename = "";
        let result = FileProcessor::extract_system_info_from_filename(empty_filename);
        assert!(
            result.is_none(),
            "Should not extract info from empty filename"
        );
    }

    #[test]
    fn test_system_info_metadata_precedence() {
        // Test that metadata contains expected system info structure
        let metadata_with_system = serde_json::json!({
            "extracted_system_info": {
                "system_id": "MetadataSystem",
                "system_label": "System from Metadata"
            }
        });

        // Test metadata structure
        assert!(metadata_with_system.get("extracted_system_info").is_some());
        let system_info = &metadata_with_system["extracted_system_info"];
        assert_eq!(system_info["system_id"].as_str(), Some("MetadataSystem"));
        assert_eq!(
            system_info["system_label"].as_str(),
            Some("System from Metadata")
        );

        // Test empty metadata
        let empty_metadata = serde_json::json!({});
        assert!(empty_metadata.get("extracted_system_info").is_none());

        // Test filename patterns
        let filename = "FileSystem_TG123_test.mp3";
        let parts: Vec<&str> = filename.split('_').collect();
        assert_eq!(parts[0], "FileSystem");
        assert!(parts.len() >= 2);
    }

    #[test]
    fn test_duration_estimation_logic() {
        use sqlx::types::Decimal;

        // Test extract_duration_from_metadata function
        let metadata_with_duration = serde_json::json!({
            "duration_estimate": 45.75
        });

        let duration = FileProcessor::extract_duration_from_metadata(&metadata_with_duration);
        assert!(duration.is_some());
        assert_eq!(duration.unwrap(), Decimal::try_from(45.75).unwrap());

        // Test with invalid duration
        let metadata_invalid = serde_json::json!({
            "duration_estimate": "not a number"
        });

        let duration = FileProcessor::extract_duration_from_metadata(&metadata_invalid);
        assert!(duration.is_none());

        // Test with missing duration
        let metadata_no_duration = serde_json::json!({});

        let duration = FileProcessor::extract_duration_from_metadata(&metadata_no_duration);
        assert!(duration.is_none());

        // Test with null duration
        let metadata_null = serde_json::json!({
            "duration_estimate": null
        });

        let duration = FileProcessor::extract_duration_from_metadata(&metadata_null);
        assert!(duration.is_none());
    }

    #[test]
    fn test_additional_processing_status_variants() {
        // Test all ProcessingStatus variants
        let pending = ProcessingStatus::Pending;
        let processing = ProcessingStatus::Processing;
        let completed = ProcessingStatus::Completed;
        let failed = ProcessingStatus::Failed {
            error: "Test error".to_string(),
            retry_count: 3,
        };
        let archived = ProcessingStatus::Archived;
        let skipped = ProcessingStatus::Skipped {
            reason: "Already exists".to_string(),
        };

        // Test Debug trait
        let debug_strings = vec![
            format!("{:?}", pending),
            format!("{:?}", processing),
            format!("{:?}", completed),
            format!("{:?}", failed),
            format!("{:?}", archived),
            format!("{:?}", skipped),
        ];

        // All should be non-empty debug representations
        for debug_str in debug_strings {
            assert!(!debug_str.is_empty());
            // Check that it's a meaningful debug representation
            assert!(
                debug_str.len() > 5,
                "Debug string should be descriptive: {debug_str}"
            );
        }

        // Test Clone trait
        let cloned_failed = failed.clone();
        if let (
            ProcessingStatus::Failed {
                error: e1,
                retry_count: r1,
            },
            ProcessingStatus::Failed {
                error: e2,
                retry_count: r2,
            },
        ) = (&failed, &cloned_failed)
        {
            assert_eq!(e1, e2);
            assert_eq!(r1, r2);
        }

        // Test PartialEq
        let another_failed = ProcessingStatus::Failed {
            error: "Test error".to_string(),
            retry_count: 3,
        };
        assert_eq!(failed, another_failed);
        assert_ne!(failed, completed);
    }

    #[test]
    fn test_database_record_logic() {
        let temp_dir = tempdir().unwrap();
        let file = create_test_queued_file(temp_dir.path().join("db_test.mp3"));

        // Test file path extraction
        let path_str = file.path.to_string_lossy();
        assert!(path_str.contains("db_test.mp3"));

        // Test filename extraction from path
        let filename = file
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown.mp3");
        assert_eq!(filename, "db_test.mp3");

        // Test size conversion
        let size_as_i64 = file.size as i64;
        assert!(size_as_i64 >= 0);

        // Test with duration in metadata
        let metadata_with_duration = serde_json::json!({
            "duration_estimate": 120.5
        });

        let duration_extracted =
            FileProcessor::extract_duration_from_metadata(&metadata_with_duration);
        assert!(duration_extracted.is_some());
        assert_eq!(
            duration_extracted.unwrap(),
            Decimal::try_from(120.5).unwrap()
        );

        // Test timestamp logic
        let now = chrono::Utc::now();
        assert!(now > chrono::Utc::now() - chrono::Duration::seconds(1));
    }

    #[test]
    fn test_file_size_handling() {
        let temp_dir = tempdir().unwrap();

        // Test with various file sizes
        let file_sizes = vec![0, 1, 1024, 1024 * 1024];

        for size in file_sizes {
            let mut file =
                create_test_queued_file(temp_dir.path().join(format!("size_{size}.mp3")));
            file.size = size;

            // Test size conversion logic
            let size_as_i64 = size as i64;
            assert_eq!(size_as_i64, size as i64);

            // Test that we can create valid filenames with size info
            let filename = format!("size_{size}.mp3");
            assert!(filename.contains(&size.to_string()));
            assert!(filename.ends_with(".mp3"));
        }
    }

    #[test]
    fn test_filename_path_extraction() {
        use std::path::PathBuf;

        let test_paths = vec![
            ("/full/path/to/file.mp3", "file.mp3"),
            ("relative/path/audio.wav", "audio.wav"),
            ("no_path_file.mp3", "no_path_file.mp3"),
            ("/path/with spaces/file name.mp3", "file name.mp3"),
            ("/path/with/unicode/文件名.mp3", "文件名.mp3"),
            ("", ""), // Empty path edge case
        ];

        for (full_path, expected_filename) in test_paths {
            let path = PathBuf::from(full_path);
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            assert_eq!(filename, expected_filename, "Failed for path: {full_path}");
        }
    }

    #[test]
    fn test_processor_configuration_edge_cases() {
        use crate::config::ProcessingConfig;
        use std::time::Duration;

        // Test configuration creation with valid fields
        let config = ProcessingConfig {
            processing_interval_seconds: 30,
            processing_timeout_seconds: 300,
            processing_workers: 4,
            max_retry_attempts: 3,
            retry_delay_seconds: 5,
            verify_file_integrity: true,
            move_after_processing: true,
            delete_after_processing: false,
        };

        assert_eq!(
            config.processing_timeout(),
            Duration::from_secs(config.processing_timeout_seconds)
        );
        assert_eq!(
            config.retry_delay(),
            Duration::from_secs(config.retry_delay_seconds)
        );
        assert_eq!(config.max_retry_attempts, 3);

        // Test edge case values
        let minimal_config = ProcessingConfig {
            processing_interval_seconds: 1,
            processing_timeout_seconds: 1,
            processing_workers: 1,
            max_retry_attempts: 0,
            retry_delay_seconds: 0,
            verify_file_integrity: false,
            move_after_processing: false,
            delete_after_processing: true,
        };

        assert_eq!(minimal_config.processing_timeout(), Duration::from_secs(1));
        assert_eq!(minimal_config.retry_delay(), Duration::from_secs(0));
        assert_eq!(minimal_config.max_retry_attempts, 0);
    }

    #[test]
    fn test_build_radio_call_record() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir
            .path()
            .join("test_system_TG123_20240101_120000.mp3");
        let queued_file = create_test_queued_file(file_path);

        // Test with extracted system info in metadata (the actual format)
        let metadata = serde_json::json!({
            "extracted_system_info": {
                "system_id": "test_system",
                "system_label": "Test System"
            },
            "duration_estimate": 45.0,
            "extra_field": "ignored"
        });

        let record = FileProcessor::build_radio_call_record(&queued_file, &metadata);

        // Verify record fields
        assert_eq!(record.system_id, "test_system");
        assert_eq!(record.system_label, Some("Test System".to_string()));
        assert!(record.audio_filename.is_some());
        assert_eq!(record.audio_content_type, Some("audio/mpeg".to_string()));
        assert_eq!(record.audio_size_bytes, Some(1024));
        assert_eq!(record.transcription_status, Some("pending".to_string()));
        assert_eq!(record.upload_api_key_id, Some("file-monitor".to_string()));
        assert!(record.duration_seconds.is_some());
        assert!(!record.id.is_nil());
    }

    #[test]
    fn test_build_radio_call_record_minimal_metadata() {
        let temp_dir = tempdir().unwrap();
        // Use a filename that will be parsed (at least 2 parts)
        let file_path = temp_dir.path().join("unknown_system.mp3");
        let queued_file = create_test_queued_file(file_path);

        // Test with empty metadata - should fall back to filename parsing
        let metadata = serde_json::json!({});

        let record = FileProcessor::build_radio_call_record(&queued_file, &metadata);

        // With "unknown_system.mp3", it should extract "unknown" as system_id and system_label
        assert_eq!(record.system_id, "unknown");
        assert_eq!(record.system_label, Some("unknown".to_string())); // Set to same as system_id from filename
        assert!(record.duration_seconds.is_none());
        assert_eq!(record.audio_content_type, Some("audio/mpeg".to_string()));
        assert_eq!(record.transcription_status, Some("pending".to_string()));
    }

    #[test]
    fn test_extract_duration_from_metadata() {
        // Test with duration_estimate (the actual field name used)
        let metadata = serde_json::json!({
            "duration_estimate": 45.0
        });
        let duration = FileProcessor::extract_duration_from_metadata(&metadata);
        assert!(duration.is_some());
        assert_eq!(duration.unwrap().to_string(), "45");

        // Test with float value
        let metadata = serde_json::json!({
            "duration_estimate": 30.5
        });
        let duration = FileProcessor::extract_duration_from_metadata(&metadata);
        assert!(duration.is_some());
        assert_eq!(duration.unwrap().to_string(), "30.5");

        // Test with no duration
        let metadata = serde_json::json!({});
        let duration = FileProcessor::extract_duration_from_metadata(&metadata);
        assert!(duration.is_none());

        // Test with wrong field name
        let metadata = serde_json::json!({
            "duration_ms": 45000.0
        });
        let duration = FileProcessor::extract_duration_from_metadata(&metadata);
        assert!(duration.is_none());

        // Test with invalid duration
        let metadata = serde_json::json!({
            "duration_estimate": "invalid"
        });
        let duration = FileProcessor::extract_duration_from_metadata(&metadata);
        assert!(duration.is_none());
    }

    #[test]
    fn test_extract_system_info() {
        // Test with extracted_system_info in metadata (the actual structure used)
        let metadata = serde_json::json!({
            "extracted_system_info": {
                "system_id": "metro_police",
                "system_label": "Metro Police Department"
            }
        });
        let filename = Some("test_file.mp3".to_string());
        let (system_id, system_label) =
            FileProcessor::extract_system_info(&metadata, filename.as_ref());

        assert_eq!(system_id, Some("metro_police".to_string()));
        assert_eq!(system_label, Some("Metro Police Department".to_string()));

        // Test with only system_id in extracted_system_info
        let metadata = serde_json::json!({
            "extracted_system_info": {
                "system_id": "fire_dept"
            }
        });
        let (system_id, system_label) =
            FileProcessor::extract_system_info(&metadata, filename.as_ref());
        assert_eq!(system_id, Some("fire_dept".to_string()));
        assert!(system_label.is_none());

        // Test falling back to filename parsing
        let metadata = serde_json::json!({});
        let filename = Some("System123_TG456_20240101_120000.mp3".to_string());
        let (system_id, system_label) =
            FileProcessor::extract_system_info(&metadata, filename.as_ref());
        assert_eq!(system_id, Some("System123".to_string()));
        assert_eq!(system_label, Some("System123".to_string())); // Both set to same value

        // Test with no info available (filename with only 1 part when split by '_')
        let metadata = serde_json::json!({});
        let filename = Some("invalidfilename.mp3".to_string()); // No underscore = only 1 part
        let (system_id, system_label) =
            FileProcessor::extract_system_info(&metadata, filename.as_ref());
        assert!(system_id.is_none());
        assert!(system_label.is_none());
    }

    #[test]
    fn test_extract_system_info_from_filename_comprehensive() {
        // Test valid SDRTrunk filename formats
        let test_cases = vec![
            // Basic format: System_YYYYMMDD_HHMMSS.mp3
            ("MetroPolice_20240101_120000.mp3", Some("MetroPolice")),
            // With talkgroup: System_TG123_YYYYMMDD_HHMMSS.mp3
            ("FireDept_TG789_20240201_140000.mp3", Some("FireDept")),
            // With patches: System_TG123_patch_YYYYMMDD_HHMMSS.mp3
            ("EMS_TG555_patch_20240301_160000.mp3", Some("EMS")),
            // Long system name
            (
                "CountySherifffDepartment_TG100_20240401_180000.mp3",
                Some("CountySherifffDepartment"),
            ),
        ];

        for (filename, expected_system) in test_cases {
            let result = FileProcessor::extract_system_info_from_filename(filename);
            if let Some(expected) = expected_system {
                assert!(result.is_some(), "Failed to extract from: {filename}");
                let json = result.unwrap();
                assert_eq!(json["system_id"].as_str(), Some(expected));
            } else {
                assert!(result.is_none(), "Should not extract from: {filename}");
            }
        }

        // Test invalid formats (filename needs at least 2 parts when split by '_')
        let invalid_cases = vec![
            "nounderscores.mp3", // Only 1 part when split by '_'
            "",                  // Empty string
            ".mp3",              // Only extension
        ];

        for filename in invalid_cases {
            let result = FileProcessor::extract_system_info_from_filename(filename);
            assert!(
                result.is_none(),
                "Should not extract from invalid: {filename}"
            );
        }

        // Test cases that should work (have at least 2 parts when split by '_')
        let valid_cases = vec![
            "single_part.mp3", // Has 2 parts: "single" and "part.mp3"
            "too_short.mp3",   // Has 2 parts: "too" and "short.mp3"
            "System_.mp3",     // Has 2 parts: "System" and ".mp3"
        ];

        for filename in valid_cases {
            let result = FileProcessor::extract_system_info_from_filename(filename);
            assert!(result.is_some(), "Should extract from valid: {filename}");
        }
    }

    #[test]
    fn test_processing_result_comprehensive() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("comprehensive_test.mp3");
        let queued_file = create_test_queued_file(file_path.clone());

        let metadata = serde_json::json!({
            "processing_time": 1500,
            "worker_id": "worker-1",
            "success": true
        });

        let result = ProcessingResult {
            file: queued_file,
            status: ProcessingStatus::Completed,
            record_id: Some(Uuid::new_v4()),
            archive_path: Some(temp_dir.path().join("archive").join("test.mp3")),
            processing_duration: Duration::from_millis(1500),
            metadata: Some(metadata),
        };

        // Verify all fields are properly set
        assert_eq!(result.file.path, file_path);
        assert!(matches!(result.status, ProcessingStatus::Completed));
        assert!(result.record_id.is_some());
        assert!(result.archive_path.is_some());
        assert_eq!(result.processing_duration, Duration::from_millis(1500));
        assert!(result.metadata.is_some());

        // Check metadata contents
        let meta = result.metadata.unwrap();
        assert_eq!(meta["processing_time"], 1500);
        assert_eq!(meta["worker_id"], "worker-1");
        assert_eq!(meta["success"], true);
    }

    #[test]
    fn test_system_info_extraction_edge_cases() {
        // Test edge cases for system info extraction
        let edge_cases = vec![
            // Empty metadata (no extracted_system_info)
            (serde_json::json!({"system_id": ""}), None, (None, None)),
            (serde_json::json!({"system_label": ""}), None, (None, None)),
            // Null values in wrong structure
            (serde_json::json!({"system_id": null}), None, (None, None)),
            // Non-string values in wrong structure
            (serde_json::json!({"system_id": 123}), None, (None, None)),
            (
                serde_json::json!({"system_label": true}),
                None,
                (None, None),
            ),
            // Correct structure with empty values (empty strings are extracted)
            (
                serde_json::json!({"extracted_system_info": {"system_id": ""}}),
                None,
                (Some(String::new()), None),
            ),
            // Null values in correct structure (filtered out)
            (
                serde_json::json!({"extracted_system_info": {"system_id": null}}),
                None,
                (None, None),
            ),
            // Very long strings in correct structure
            (
                serde_json::json!({"extracted_system_info": {"system_id": "a".repeat(100)}}),
                None,
                (Some("a".repeat(100)), None),
            ),
        ];

        for (metadata, filename, expected) in edge_cases {
            let result = FileProcessor::extract_system_info(&metadata, filename.as_ref());
            assert_eq!(result, expected, "Failed for metadata: {metadata:?}");
        }
    }
}
