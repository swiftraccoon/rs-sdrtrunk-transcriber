# SDRTrunk Monitor

A high-performance, cross-platform file monitoring service for the SDRTrunk transcriber project. This service watches for new MP3 files from SDRTrunk and automatically processes them for database insertion and transcription.

## Features

### Core Functionality

- **Cross-platform file monitoring** using `notify` crate with `inotify` (Linux) and `FSEvents` (macOS) support
- **Intelligent file filtering** with configurable patterns, extensions, and size limits
- **Resilient processing** with retry logic, timeout handling, and error recovery
- **Queue management** with priority queuing and crash recovery through persistence
- **Database integration** with the existing SDRTrunk database schema
- **File archiving** with configurable organization (by date, system, etc.)
- **Comprehensive logging** using structured logging with `tracing`
- **Graceful shutdown** with configurable timeout

### Performance & Reliability

- **Zero-copy file operations** where possible
- **Concurrent processing** with configurable worker threads
- **Memory-efficient queuing** with configurable size limits
- **Debounced file events** to prevent duplicate processing
- **Health monitoring** with database connection checks
- **Metrics collection** for monitoring service performance
- **Automatic restart** capabilities for critical errors

### Configuration

- **Environment variable support** for all configuration options
- **TOML configuration files** with sensible defaults
- **Runtime configuration validation**
- **Hot-reload capabilities** (planned)

## Installation

### Prerequisites

- Rust 1.85.0 or later with Edition 2024
- PostgreSQL database (for SDRTrunk data)
- Sufficient disk space for file processing and archiving

### Building from Source

```bash
# Clone the repository
git clone https://github.com/your-repo/rs-sdrtrunk-transcriber.git
cd rs-sdrtrunk-transcriber

# Build the monitor service
cargo build --release -p sdrtrunk-monitor

# The binary will be available at:
# target/release/sdrtrunk-monitor
```

## Configuration

### Configuration File

Create a `monitor.toml` file (see [monitor.toml](../../monitor.toml) for a complete example):

```toml
[watch]
watch_directory = "/path/to/sdrtrunk/recordings"
file_patterns = ["*.mp3"]
file_extensions = ["mp3"]
min_file_size = 1024
max_file_size = 100_000_000
recursive = true

[processing]
processing_workers = 2
max_retry_attempts = 3
processing_timeout_seconds = 300
move_after_processing = true

[storage]
archive_directory = "/path/to/archive"
organize_by_date = true
organize_by_system = true

[database]
url = "postgresql://user:pass@localhost/sdrtrunk"
max_connections = 50

[service]
enable_metrics = true
auto_restart = true
```

### Environment Variables

All configuration options can be overridden using environment variables with the `SDRTRUNK_MONITOR_` prefix:

```bash
export SDRTRUNK_MONITOR_WATCH__WATCH_DIRECTORY="/path/to/recordings"
export SDRTRUNK_MONITOR_DATABASE__URL="postgresql://user:pass@localhost/db"
export SDRTRUNK_MONITOR_PROCESSING__PROCESSING_WORKERS=4
```

## Usage

### Basic Usage

```bash
# Start the monitor service with default configuration
./sdrtrunk-monitor

# Start with a specific configuration file
./sdrtrunk-monitor --config /path/to/monitor.toml

# Start with debug logging
./sdrtrunk-monitor --log-level debug

# Start with JSON logging
./sdrtrunk-monitor --json
```

### Command Line Interface

#### Service Management

```bash
# Start the service
./sdrtrunk-monitor start

# Start in daemon mode (background)
./sdrtrunk-monitor start --daemon --pid-file /var/run/sdrtrunk-monitor.pid

# Stop a running service
./sdrtrunk-monitor stop --pid-file /var/run/sdrtrunk-monitor.pid

# Check service status
./sdrtrunk-monitor status
```

#### Monitoring & Metrics

```bash
# Show current metrics
./sdrtrunk-monitor metrics

# Watch metrics (update every 5 seconds)
./sdrtrunk-monitor metrics --watch 5

# Show metrics in JSON format
./sdrtrunk-monitor metrics --format json
```

#### Configuration Management

```bash
# Validate configuration
./sdrtrunk-monitor config --validate

# Show resolved configuration
./sdrtrunk-monitor config --show

# Show configuration in JSON format
./sdrtrunk-monitor config --show --format json
```

#### Queue Management

```bash
# Show queue status
./sdrtrunk-monitor queue status

# List queued files
./sdrtrunk-monitor queue list

# List only failed files
./sdrtrunk-monitor queue list --failed

# Retry a specific failed file
./sdrtrunk-monitor queue retry <file-id>

# Retry all failed files
./sdrtrunk-monitor queue retry --all

# Clear failed files (dry-run)
./sdrtrunk-monitor queue clear

# Clear failed files (execute)
./sdrtrunk-monitor queue clear --execute

# Manually add a file to the queue
./sdrtrunk-monitor queue add /path/to/file.mp3
```

#### Directory Scanning

```bash
# Scan directory for existing files (dry-run)
./sdrtrunk-monitor scan /path/to/directory

# Scan and queue files for processing
./sdrtrunk-monitor scan /path/to/directory --execute

# Use configured watch directory
./sdrtrunk-monitor scan --execute
```

## Architecture

### Components

#### FileMonitor

- Watches filesystem for changes using cross-platform `notify` crate
- Filters events based on configured patterns and file properties
- Debounces events to prevent duplicate processing
- Scans directories for existing files on startup

#### FileQueue

- Thread-safe priority queue for managing files waiting to be processed
- Supports priority based on file age and size
- Provides crash recovery through optional persistence to disk
- Tracks processing attempts and failures

#### FileProcessor

- Processes individual files with configurable timeout
- Extracts metadata and creates database records
- Handles file archiving with configurable organization
- Implements retry logic with exponential backoff

#### MonitorService

- Orchestrates all components with proper lifecycle management
- Spawns configurable number of worker threads
- Provides health monitoring and metrics collection
- Handles graceful shutdown with configurable timeout

### Processing Flow

1. **File Detection**: FileMonitor detects new MP3 files
2. **Queuing**: Files are added to the FileQueue with priority calculation
3. **Processing**: Worker threads dequeue files and process them:
   - Verify file integrity (optional)
   - Check for existing database records
   - Extract metadata from filename/file
   - Create database record
   - Archive file (optional)
4. **Error Handling**: Failed files are retried or moved to failed directory
5. **Monitoring**: Metrics are collected and health checks performed

## Database Integration

The service integrates with the existing SDRTrunk database schema and creates records in the `radio_calls` table with the following information:

- File metadata (path, size, timestamps)
- System information extracted from filename
- Talkgroup and frequency data (if available)
- Processing status for transcription pipeline
- Archive location and processing metadata

## Monitoring & Observability

### Metrics

The service collects comprehensive metrics:

- Files detected, queued, processed, failed, and archived
- Average processing times
- Queue depths and processing rates
- Service uptime and health status

### Logging

Structured logging with configurable levels:

- **TRACE**: Detailed execution flow
- **DEBUG**: Development and troubleshooting information
- **INFO**: General operational information
- **WARN**: Recoverable errors and warnings
- **ERROR**: Non-recoverable errors

### Health Checks

- Database connectivity
- Disk space availability
- Queue processing rates
- Worker thread health

## Performance Tuning

### Configuration Recommendations

```toml
[processing]
# Adjust based on CPU cores and I/O capacity
processing_workers = 4  # 2x CPU cores for I/O bound work

# Tune for your average file sizes and processing time
processing_timeout_seconds = 180  # 3 minutes for large files

[queue]
# Adjust based on available memory
max_queue_size = 5000  # ~50MB for metadata

# Enable for crash recovery
persistence_file = "/var/lib/sdrtrunk-monitor/queue.json"

[storage]
# Reduce I/O if not needed
verify_file_integrity = false  # Skip for trusted sources
organize_by_date = true  # Better for large archives
```

### System Recommendations

- **CPU**: 2+ cores recommended for concurrent processing
- **Memory**: 1GB+ recommended for large queues
- **Disk**: SSD recommended for database and temporary storage
- **Network**: Low latency connection to database server

## Security Considerations

### File System Access

- Run with minimal required permissions
- Use dedicated user account for service
- Restrict access to watch and archive directories

### Database Security

- Use dedicated database user with minimal permissions
- Enable SSL/TLS for database connections
- Regular security updates for dependencies

### Configuration Security

- Protect configuration files containing database credentials
- Use environment variables for sensitive values
- Regular rotation of database passwords

## Troubleshooting

### Common Issues

#### High Memory Usage

```bash
# Check queue size
./sdrtrunk-monitor queue status

# Reduce queue size in configuration
max_queue_size = 1000
```

#### Processing Delays

```bash
# Check worker utilization
./sdrtrunk-monitor metrics

# Increase worker threads
processing_workers = 8
```

#### Database Connection Errors

```bash
# Verify connection
./sdrtrunk-monitor config --validate

# Check database health
./sdrtrunk-monitor status
```

### Log Analysis

```bash
# Follow logs in real-time
tail -f /var/log/sdrtrunk-monitor.log

# Search for errors
grep ERROR /var/log/sdrtrunk-monitor.log

# Analyze processing times
grep "File processing completed" /var/log/sdrtrunk-monitor.log | \
  jq '.duration_ms' | \
  awk '{sum+=$1; n++} END {print "Average: " sum/n "ms"}'
```

## Development

### Building

```bash
cargo build -p sdrtrunk-monitor
```

### Testing

```bash
# Run unit tests
cargo test -p sdrtrunk-monitor

# Run integration tests (requires database)
DATABASE_URL=postgresql://test_user:test_pass@localhost/test_db \
  cargo test -p sdrtrunk-monitor --features integration-tests
```

### Benchmarking

```bash
# Run benchmarks
cargo bench -p sdrtrunk-monitor

# Profile with specific benchmark
cargo bench -p sdrtrunk-monitor -- pattern_matching
```

### Documentation

```bash
# Generate documentation
cargo doc -p sdrtrunk-monitor --open
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make changes with appropriate tests
4. Update documentation
5. Submit a pull request

### Code Standards

- Follow Rust 2024 idioms
- Comprehensive error handling
- Structured logging
- Performance benchmarks for critical paths
- Security considerations for all external inputs

## License

This project is licensed under GPL-3.0. See [LICENSE](../../LICENSE) for details.
