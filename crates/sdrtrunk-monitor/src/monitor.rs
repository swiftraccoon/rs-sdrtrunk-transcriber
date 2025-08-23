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
    debouncer: Option<Debouncer<RecommendedWatcher, FileIdMap>>,

    /// Event receiver
    event_receiver: Option<mpsc::Receiver<FileEvent>>,
}

impl FileMonitor {
    /// Create a new file monitor
    #[must_use]
    pub const fn new(config: WatchConfig) -> Self {
        Self {
            config,
            debouncer: None,
            event_receiver: None,
        }
    }

    /// Start monitoring the configured directory
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if:
    /// - Cannot create the watch directory
    /// - Cannot initialize the file system watcher
    /// - Cannot watch the specified directory
    ///
    /// # Panics
    ///
    /// Panics if called multiple times on the same instance without calling `stop()` first,
    /// as it relies on `event_receiver` being `Some` after successful initialization.
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

        // Get a handle to the current runtime for spawning tasks from the callback
        let runtime_handle = tokio::runtime::Handle::current();

        // Create debounced watcher
        let mut debouncer = new_debouncer(
            self.config.debounce_delay(),
            None,
            move |result: DebounceEventResult| {
                let tx = tx.clone();
                let config = config.clone();
                let runtime_handle = runtime_handle.clone();

                // Spawn the async task using the runtime handle
                runtime_handle.spawn(async move {
                    match result {
                        Ok(events) => {
                            for event in events {
                                if let Some(file_event) =
                                    Self::process_debounced_event(event, &config).await
                                    && let Err(e) = tx.send(file_event).await
                                {
                                    error!("Failed to send file event: {}", e);
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

        self.debouncer = Some(debouncer);
        self.event_receiver = Some(rx);

        Ok(self.event_receiver.take().unwrap())
    }

    /// Stop the file monitor
    pub fn stop(&mut self) {
        if self.debouncer.is_some() {
            info!("Stopping file system monitor");
            self.debouncer = None;
            self.event_receiver = None;
        }
    }

    /// Process a debounced file system event
    async fn process_debounced_event(
        event: DebouncedEvent,
        config: &WatchConfig,
    ) -> Option<FileEvent> {
        let path = event.paths.first()?;
        debug!("Processing debounced event for: {}", path.display());

        // Basic filtering
        if !Self::should_process_path(path, config).await? {
            return None;
        }

        // Get file metadata and size
        let file_size = Self::get_file_size(path).await?;

        // Check size constraints
        if !Self::is_size_within_limits(file_size, config, path) {
            return None;
        }

        // Convert and validate event type
        let event_type = Self::convert_event_type(event.event.kind)?;

        // Validate file existence for certain event types
        if !Self::validate_file_existence(&event_type, path) {
            return None;
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

    /// Check if a path should be processed based on patterns and symlink settings
    async fn should_process_path(path: &Path, config: &WatchConfig) -> Option<bool> {
        // Check if file matches our patterns
        if !Self::matches_patterns(path, &config.file_patterns, &config.file_extensions) {
            debug!("File does not match patterns: {}", path.display());
            return Some(false);
        }

        // Skip symbolic links if not configured to follow them
        if !config.follow_symlinks
            && let Ok(metadata) = tokio::fs::symlink_metadata(path).await
            && metadata.file_type().is_symlink()
        {
            debug!("Skipping symbolic link: {}", path.display());
            return Some(false);
        }

        Some(true)
    }

    /// Get file size, returning None if metadata cannot be read
    async fn get_file_size(path: &Path) -> Option<u64> {
        match tokio::fs::metadata(path).await {
            Ok(metadata) => Some(metadata.len()),
            Err(e) => {
                warn!(
                    path = %path.display(),
                    error = %e,
                    "Failed to get file metadata"
                );
                None
            }
        }
    }

    /// Check if file size is within configured limits
    fn is_size_within_limits(file_size: u64, config: &WatchConfig, path: &Path) -> bool {
        if file_size < config.min_file_size {
            debug!(
                path = %path.display(),
                size = file_size,
                min_size = config.min_file_size,
                "File too small, skipping"
            );
            return false;
        }

        if file_size > config.max_file_size {
            debug!(
                path = %path.display(),
                size = file_size,
                max_size = config.max_file_size,
                "File too large, skipping"
            );
            return false;
        }

        true
    }

    /// Convert notify event kind to our event type
    fn convert_event_type(event_kind: notify::EventKind) -> Option<FileEventType> {
        match event_kind {
            notify::EventKind::Create(_) => Some(FileEventType::Created),
            notify::EventKind::Modify(_) => Some(FileEventType::Modified),
            notify::EventKind::Remove(_) => Some(FileEventType::Removed),
            notify::EventKind::Access(_) => {
                // Skip access events as they're too noisy
                None
            }
            _ => {
                debug!("Unhandled event kind: {:?}", event_kind);
                None
            }
        }
    }

    /// Validate that file exists for events that require it
    fn validate_file_existence(event_type: &FileEventType, path: &Path) -> bool {
        match event_type {
            FileEventType::Created | FileEventType::Modified => {
                if !path.exists() {
                    debug!("File no longer exists: {}", path.display());
                    return false;
                }
            }
            FileEventType::Removed | FileEventType::MovedTo => {
                // File was removed or moved, we still want to process this event
            }
        }
        true
    }

    /// Check if a file matches the configured patterns and extensions
    #[must_use]
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

            if let Some(ext) = pattern.strip_prefix("*.") {
                return file_name.ends_with(&format!(".{}", ext.to_lowercase()));
            }

            if pattern.contains('*') {
                // Basic wildcard matching
                let pattern_lower = pattern.to_lowercase();
                if pattern_lower.starts_with('*') && pattern_lower.ends_with('*') {
                    if let Some(middle) = pattern_lower
                        .strip_prefix('*')
                        .and_then(|s| s.strip_suffix('*'))
                    {
                        return file_name.contains(middle);
                    }
                } else if let Some(suffix) = pattern_lower.strip_prefix('*') {
                    return file_name.ends_with(suffix);
                } else if let Some(prefix) = pattern_lower.strip_suffix('*') {
                    return file_name.starts_with(prefix);
                }
            }

            // Exact match
            file_name == pattern.to_lowercase() || path_str == *pattern
        })
    }

    /// Manually scan the watch directory for existing files
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if:
    /// - Cannot read the watch directory or subdirectories
    /// - I/O errors occur during directory traversal
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
    ///
    /// # Errors
    ///
    /// Returns [`MonitorError`] if:
    /// - Cannot read directory entries
    /// - I/O errors occur during file metadata access
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
    /// Test basic file monitoring functionality
    ///
    /// # Panics
    ///
    /// Panics if test setup fails (temp directory creation, file operations)
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
            () = sleep(TokioDuration::from_secs(5)) => {
                panic!("Timeout waiting for file event");
            }
        }

        monitor.stop();
    }

    #[test]
    /// Test pattern matching functionality
    ///
    /// # Panics
    ///
    /// Panics if assertions fail
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
    /// Test scanning for existing files
    ///
    /// # Panics
    ///
    /// Panics if test setup fails (temp directory creation, file operations)
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
