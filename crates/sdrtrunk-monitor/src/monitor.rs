//! File system monitoring implementation
//!
//! Provides cross-platform file system monitoring using the `notify` crate with
//! debouncing support. Monitors for new MP3 files and filters based on configuration.

use crate::{MonitorError, Result, config::WatchConfig};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{
    DebounceEventResult, DebouncedEvent, Debouncer, FileIdMap, new_debouncer,
};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// File system event from the monitor
#[derive(Debug, Clone)]
pub struct FileEvent {
    /// Path to the file that changed
    pub path: PathBuf,

    /// Type of event
    pub event_type: FileEventType,

    /// File size in bytes (if available)
    pub size: Option<u64>,

    /// Whether this is the final event for this file
    pub is_final: bool,
}

/// Types of file system events we care about
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEventType {
    /// File was created
    Created,

    /// File was modified
    Modified,

    /// File was moved/renamed to this location
    MovedTo,

    /// File was removed
    Removed,
}

/// File system monitor that watches for MP3 files
#[derive(Debug)]
pub struct FileMonitor {
    /// Configuration for monitoring
    config: WatchConfig,

    /// File system watcher/debouncer
    _debouncer: Option<Debouncer<RecommendedWatcher, FileIdMap>>,

    /// Event receiver
    event_receiver: Option<mpsc::Receiver<FileEvent>>,
}

impl FileMonitor {
    /// Create a new file monitor
    pub fn new(config: WatchConfig) -> Self {
        Self {
            config,
            _debouncer: None,
            event_receiver: None,
        }
    }

    /// Start monitoring the configured directory
    pub async fn start(&mut self) -> Result<mpsc::Receiver<FileEvent>> {
        info!(
            watch_dir = %self.config.watch_directory.display(),
            recursive = self.config.recursive,
            patterns = ?self.config.file_patterns,
            "Starting file system monitor"
        );

        // Ensure watch directory exists
        if !self.config.watch_directory.exists() {
            tokio::fs::create_dir_all(&self.config.watch_directory).await?;
            info!(
                "Created watch directory: {}",
                self.config.watch_directory.display()
            );
        }

        // Create event channel
        let (tx, rx) = mpsc::channel(1000);

        // Clone config for the closure
        let config = self.config.clone();

        // Create debounced watcher
        let mut debouncer = new_debouncer(
            self.config.debounce_delay(),
            None,
            move |result: DebounceEventResult| {
                let tx = tx.clone();
                let config = config.clone();

                tokio::spawn(async move {
                    match result {
                        Ok(events) => {
                            for event in events {
                                if let Some(file_event) =
                                    Self::process_debounced_event(event, &config).await
                                {
                                    if let Err(e) = tx.send(file_event).await {
                                        error!("Failed to send file event: {}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("File system watcher error: {:?}", e);
                        }
                    }
                });
            },
        )
        .map_err(|e| MonitorError::watcher(e.to_string()))?;

        // Start watching
        let recursive_mode = if self.config.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        debouncer
            .watcher()
            .watch(&self.config.watch_directory, recursive_mode)
            .map_err(|e| {
                MonitorError::watcher(format!(
                    "Failed to watch {}: {}",
                    self.config.watch_directory.display(),
                    e
                ))
            })?;

        info!("File system monitor started successfully");

        self._debouncer = Some(debouncer);
        self.event_receiver = Some(rx);

        Ok(self.event_receiver.take().unwrap())
    }

    /// Stop the file monitor
    pub fn stop(&mut self) {
        if self._debouncer.is_some() {
            info!("Stopping file system monitor");
            self._debouncer = None;
            self.event_receiver = None;
        }
    }

    /// Process a debounced file system event
    async fn process_debounced_event(
        event: DebouncedEvent,
        config: &WatchConfig,
    ) -> Option<FileEvent> {
        // Get the first path from the event (DebouncedEvent contains multiple paths)
        let path = event.paths.first()?;

        debug!("Processing debounced event for: {}", path.display());

        // Check if file matches our patterns
        if !Self::matches_patterns(path, &config.file_patterns, &config.file_extensions) {
            debug!("File does not match patterns: {}", path.display());
            return None;
        }

        // Skip symbolic links if not configured to follow them
        if !config.follow_symlinks {
            if let Ok(metadata) = tokio::fs::symlink_metadata(path).await {
                if metadata.file_type().is_symlink() {
                    debug!("Skipping symbolic link: {}", path.display());
                    return None;
                }
            }
        }

        // Get file metadata
        let metadata = match tokio::fs::metadata(path).await {
            Ok(metadata) => metadata,
            Err(e) => {
                warn!(
                    path = %path.display(),
                    error = %e,
                    "Failed to get file metadata"
                );
                return None;
            }
        };

        let file_size = metadata.len();

        // Check file size constraints
        if file_size < config.min_file_size {
            debug!(
                path = %path.display(),
                size = file_size,
                min_size = config.min_file_size,
                "File too small, skipping"
            );
            return None;
        }

        if file_size > config.max_file_size {
            debug!(
                path = %path.display(),
                size = file_size,
                max_size = config.max_file_size,
                "File too large, skipping"
            );
            return None;
        }

        // Convert notify event to our event type
        let event_type = match event.event.kind {
            notify::EventKind::Create(_) => FileEventType::Created,
            notify::EventKind::Modify(_) => FileEventType::Modified,
            notify::EventKind::Remove(_) => FileEventType::Removed,
            notify::EventKind::Access(_) => {
                // Skip access events as they're too noisy
                return None;
            }
            _ => {
                debug!("Unhandled event kind: {:?}", event.event.kind);
                return None;
            }
        };

        // Only process create and modify events for files that currently exist
        match event_type {
            FileEventType::Created | FileEventType::Modified => {
                if !path.exists() {
                    debug!("File no longer exists: {}", path.display());
                    return None;
                }
            }
            FileEventType::Removed => {
                // File was removed, we still want to process this event
            }
            _ => {}
        }

        debug!(
            path = %path.display(),
            event_type = ?event_type,
            size = file_size,
            "Generated file event"
        );

        Some(FileEvent {
            path: path.clone(),
            event_type,
            size: Some(file_size),
            is_final: true, // Debounced events are always final
        })
    }

    /// Check if a file matches the configured patterns and extensions
    pub fn matches_patterns(path: &Path, patterns: &[String], extensions: &[String]) -> bool {
        let path_str = path.to_string_lossy();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Check file extensions first (more efficient)
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if !extensions
                .iter()
                .any(|pattern_ext| ext.eq_ignore_ascii_case(pattern_ext))
            {
                return false;
            }
        } else if !extensions.is_empty() {
            // No extension but we have extension filters
            return false;
        }

        // Check glob patterns
        if patterns.is_empty() {
            return true; // No patterns means match everything
        }

        patterns.iter().any(|pattern| {
            // Simple glob matching for common patterns
            if pattern == "*" {
                return true;
            }

            if pattern.starts_with("*.") {
                let ext = &pattern[2..];
                return file_name.ends_with(&format!(".{}", ext.to_lowercase()));
            }

            if pattern.contains('*') {
                // Basic wildcard matching
                let pattern_lower = pattern.to_lowercase();
                if pattern_lower.starts_with('*') && pattern_lower.ends_with('*') {
                    let middle = &pattern_lower[1..pattern_lower.len() - 1];
                    return file_name.contains(middle);
                } else if pattern_lower.starts_with('*') {
                    let suffix = &pattern_lower[1..];
                    return file_name.ends_with(suffix);
                } else if pattern_lower.ends_with('*') {
                    let prefix = &pattern_lower[..pattern_lower.len() - 1];
                    return file_name.starts_with(prefix);
                }
            }

            // Exact match
            file_name == pattern.to_lowercase() || path_str == *pattern
        })
    }

    /// Manually scan the watch directory for existing files
    pub async fn scan_existing_files(&self) -> Result<Vec<PathBuf>> {
        info!(
            directory = %self.config.watch_directory.display(),
            "Scanning for existing files"
        );

        let mut files = Vec::new();
        self.scan_directory(&self.config.watch_directory, &mut files, 0)
            .await?;

        info!("Found {} existing files", files.len());
        Ok(files)
    }

    /// Recursively scan a directory for matching files
    #[async_recursion::async_recursion]
    async fn scan_directory(
        &self,
        directory: &Path,
        files: &mut Vec<PathBuf>,
        depth: usize,
    ) -> Result<()> {
        const MAX_DEPTH: usize = 10; // Prevent infinite recursion

        if depth > MAX_DEPTH {
            warn!("Maximum directory depth reached: {}", directory.display());
            return Ok(());
        }

        let mut entries = match tokio::fs::read_dir(directory).await {
            Ok(entries) => entries,
            Err(e) => {
                warn!(
                    directory = %directory.display(),
                    error = %e,
                    "Failed to read directory"
                );
                return Ok(()); // Continue with other directories
            }
        };

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_dir() && self.config.recursive {
                self.scan_directory(&path, files, depth + 1).await?;
            } else if path.is_file() {
                // Check if file matches patterns
                if Self::matches_patterns(
                    &path,
                    &self.config.file_patterns,
                    &self.config.file_extensions,
                ) {
                    // Check file size
                    if let Ok(metadata) = tokio::fs::metadata(&path).await {
                        let file_size = metadata.len();
                        if file_size >= self.config.min_file_size
                            && file_size <= self.config.max_file_size
                        {
                            files.push(path);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl Drop for FileMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::time::{Duration as TokioDuration, sleep};

    #[tokio::test]
    async fn test_file_monitor_basic() {
        let temp_dir = TempDir::new().unwrap();
        let watch_dir = temp_dir.path().to_path_buf();

        let config = WatchConfig {
            watch_directory: watch_dir.clone(),
            file_patterns: vec!["*.mp3".to_string()],
            file_extensions: vec!["mp3".to_string()],
            min_file_size: 0,
            max_file_size: 1024 * 1024,
            debounce_delay_ms: 100,
            recursive: false,
            follow_symlinks: false,
        };

        let mut monitor = FileMonitor::new(config);
        let mut receiver = monitor.start().await.unwrap();

        // Create a test file
        let test_file = watch_dir.join("test.mp3");
        tokio::fs::write(&test_file, b"test content").await.unwrap();

        // Wait for the event
        tokio::select! {
            event = receiver.recv() => {
                let event = event.unwrap();
                assert_eq!(event.path, test_file);
                assert!(matches!(event.event_type, FileEventType::Created));
            }
            _ = sleep(TokioDuration::from_secs(5)) => {
                panic!("Timeout waiting for file event");
            }
        }

        monitor.stop();
    }

    #[test]
    fn test_pattern_matching() {
        let path = Path::new("/test/file.mp3");

        // Test extension matching
        assert!(FileMonitor::matches_patterns(
            path,
            &["*.mp3".to_string()],
            &["mp3".to_string()]
        ));

        // Test case insensitive
        assert!(FileMonitor::matches_patterns(
            Path::new("/test/FILE.MP3"),
            &["*.mp3".to_string()],
            &["mp3".to_string()]
        ));

        // Test no match
        assert!(!FileMonitor::matches_patterns(
            Path::new("/test/file.txt"),
            &["*.mp3".to_string()],
            &["mp3".to_string()]
        ));
    }

    #[tokio::test]
    async fn test_scan_existing_files() {
        let temp_dir = TempDir::new().unwrap();
        let watch_dir = temp_dir.path().to_path_buf();

        // Create test files
        tokio::fs::write(watch_dir.join("test1.mp3"), b"content1")
            .await
            .unwrap();
        tokio::fs::write(watch_dir.join("test2.mp3"), b"content2")
            .await
            .unwrap();
        tokio::fs::write(watch_dir.join("test.txt"), b"content3")
            .await
            .unwrap(); // Should be ignored

        let config = WatchConfig {
            watch_directory: watch_dir,
            file_patterns: vec!["*.mp3".to_string()],
            file_extensions: vec!["mp3".to_string()],
            min_file_size: 0,
            max_file_size: 1024 * 1024,
            debounce_delay_ms: 100,
            recursive: false,
            follow_symlinks: false,
        };

        let monitor = FileMonitor::new(config);
        let files = monitor.scan_existing_files().await.unwrap();

        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|p| p.file_name().unwrap() == "test1.mp3"));
        assert!(files.iter().any(|p| p.file_name().unwrap() == "test2.mp3"));
    }
}
