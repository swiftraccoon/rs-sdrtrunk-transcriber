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
    pub fn new(
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
    pub async fn process_file(&self, mut file: QueuedFile) -> ProcessingResult {
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
            self.process_file_internal(&mut file),
        )
        .await;

        result.processing_duration = start_time.elapsed().into();

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

        result.file = file;
        result
    }

    /// Internal file processing logic
    async fn process_file_internal(&self, file: &mut QueuedFile) -> Result<ProcessingStatus> {
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
    async fn verify_file_integrity(&self, path: &Path) -> Result<()> {
        debug!("Verifying file integrity");

        // Check file is readable
        let mut file = fs::File::open(path).await?;

        // For MP3 files, check for basic MP3 header
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext.eq_ignore_ascii_case("mp3") {
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

                return Err(MonitorError::invalid_file(path, "Invalid MP3 file format"));
            }
        }

        Ok(())
    }

    /// Check if file already exists in database
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
    async fn extract_file_metadata(&self, path: &Path) -> Result<serde_json::Value> {
        debug!("Extracting file metadata");

        let metadata = fs::metadata(path).await?;

        let mut meta = serde_json::json!({
            "file_size": metadata.len(),
            "modified_at": metadata.modified()?.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
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
            if let Some(system_info) = self.extract_system_info_from_filename(filename) {
                meta["extracted_system_info"] = system_info;
            }
        }

        // For MP3 files, we could extract audio metadata here
        // This is a simplified version - in production you might want to use a proper audio library
        meta["duration_estimate"] = self.estimate_mp3_duration(path).await?;

        Ok(meta)
    }

    /// Create database record for the processed file
    async fn create_database_record(
        &self,
        file: &QueuedFile,
        metadata: &serde_json::Value,
    ) -> Result<Uuid> {
        debug!("Creating database record");

        let record_id = Uuid::new_v4();
        let now = Utc::now();

        let filename = file
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .map(ToString::to_string);

        let file_path_str = file.path.to_string_lossy().to_string();

        // Extract system information from metadata or filename
        let (system_id, system_label) = self.extract_system_info(metadata, &filename);

        let duration_seconds = metadata
            .get("duration_estimate")
            .and_then(|v| v.as_f64())
            .map(Decimal::from_f64_retain)
            .flatten();

        // Create the database record
        let record = RadioCallDb {
            id: record_id,
            created_at: now,
            call_timestamp: file.modified_at,
            system_id: system_id.unwrap_or_else(|| "unknown".to_string()),
            system_label,
            frequency: None,    // Will be populated later if available
            talkgroup_id: None, // Will be populated later if available
            talkgroup_label: None,
            talkgroup_group: None,
            talkgroup_tag: None,
            source_radio_id: None,
            talker_alias: None,
            audio_filename: filename,
            audio_file_path: Some(file_path_str),
            audio_size_bytes: Some(file.size as i64),
            audio_content_type: Some("audio/mpeg".to_string()),
            duration_seconds,
            transcription_text: None, // Will be populated by transcription service
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
        };

        // Insert into database using a simpler approach
        let rows_affected = sqlx::query(
            r#"INSERT INTO radio_calls (
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
            )"#,
        )
        .bind(record.id)
        .bind(record.created_at)
        .bind(record.call_timestamp)
        .bind(record.system_id)
        .bind(record.system_label)
        .bind(record.frequency)
        .bind(record.talkgroup_id)
        .bind(record.talkgroup_label)
        .bind(record.talkgroup_group)
        .bind(record.talkgroup_tag)
        .bind(record.source_radio_id)
        .bind(record.talker_alias)
        .bind(record.audio_filename)
        .bind(record.audio_file_path)
        .bind(record.audio_size_bytes)
        .bind(record.audio_content_type)
        .bind(record.duration_seconds)
        .bind(record.transcription_text)
        .bind(record.transcription_confidence)
        .bind(record.transcription_language)
        .bind(record.transcription_status)
        .bind(record.speaker_segments)
        .bind(record.speaker_count)
        .bind(record.patches)
        .bind(record.frequencies)
        .bind(record.sources)
        .bind(record.upload_ip)
        .bind(record.upload_timestamp)
        .bind(record.upload_api_key_id)
        .execute(self.db_pool.as_ref())
        .await?;

        if rows_affected.rows_affected() == 0 {
            return Err(MonitorError::processing(
                file.path.clone(),
                "Failed to insert database record",
            ));
        }

        info!(record_id = %record_id, "Created database record");

        Ok(record_id)
    }

    /// Archive a processed file
    async fn archive_file(&self, file: &QueuedFile) -> Result<PathBuf> {
        debug!("Archiving file");

        // Ensure archive directory exists
        fs::create_dir_all(&self.archive_dir).await?;

        // Generate archive path
        let archive_path = self.generate_archive_path(file).await?;

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
    async fn generate_archive_path(&self, file: &QueuedFile) -> Result<PathBuf> {
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
                final_path = archive_path.with_extension(&counter.to_string());
            }
            counter += 1;
        }

        Ok(final_path)
    }

    /// Delete a file
    async fn delete_file(&self, path: &Path) -> Result<()> {
        debug!(path = %path.display(), "Deleting file");

        fs::remove_file(path).await?;

        info!(path = %path.display(), "File deleted");
        Ok(())
    }

    /// Extract system information from metadata or filename
    fn extract_system_info(
        &self,
        metadata: &serde_json::Value,
        filename: &Option<String>,
    ) -> (Option<String>, Option<String>) {
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
        if let Some(filename) = filename {
            if let Some(system_info) = self.extract_system_info_from_filename(filename) {
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
        }

        (None, None)
    }

    /// Extract system information from filename (SDRTrunk naming convention)
    fn extract_system_info_from_filename(&self, filename: &str) -> Option<serde_json::Value> {
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
    async fn estimate_mp3_duration(&self, path: &Path) -> Result<serde_json::Value> {
        let metadata = fs::metadata(path).await?;
        let file_size = metadata.len() as f64;

        // Very rough estimate: assume 128kbps MP3
        // Duration ≈ file_size_bytes * 8 / bitrate_bps
        let estimated_duration = file_size * 8.0 / 128_000.0;

        Ok(serde_json::Value::Number(
            serde_json::Number::from_f64(estimated_duration).unwrap_or(serde_json::Number::from(0)),
        ))
    }

    /// Get database connection pool
    #[must_use]
    pub fn db_pool(&self) -> &Arc<PgPool> {
        &self.db_pool
    }
}

#[cfg(test)]
mod tests {
    // Tests disabled for now due to database dependency
    // TODO: Add proper tests with testcontainers
}
