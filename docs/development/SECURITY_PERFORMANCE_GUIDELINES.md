# Security and Performance Guidelines

## Rust SDR Trunk Transcriber Production Application

### Table of Contents

- [1. Security Requirements](#1-security-requirements)
- [2. Performance Requirements](#2-performance-requirements)
- [3. Audit Requirements](#3-audit-requirements)
- [4. Operational Security](#4-operational-security)

---

## 1. Security Requirements

### 1.1 Unsafe Code Policies

#### Mandatory Policy

- **ZERO unsafe code permitted** in production builds
- Workspace-level enforcement: `unsafe_code = "forbid"`
- Automated CI/CD verification with `cargo audit` and `clippy`

#### Exception Process (Emergency Only)

```rust
// Only permitted in isolated modules with explicit justification
#[allow(unsafe_code)]
mod ffi_bridge {
    // SAFETY: Documentation required explaining why unsafe is needed
    // AUDIT: All unsafe blocks must have security review within 48 hours
    // LIMIT: Maximum 100 lines of unsafe code per module
}
```

#### Verification Requirements

- Daily automated scans: `cargo geiger --forbid-unsafe`
- Pre-commit hooks blocking unsafe code
- Quarterly security reviews of any exception modules

### 1.2 Input Validation Standards

#### Audio File Upload Validation

```rust
pub struct AudioValidationRules {
    // File size limits
    pub max_file_size: usize = 100_000_000,  // 100MB absolute max
    pub min_file_size: usize = 1_024,        // 1KB minimum
    
    // Format restrictions
    pub allowed_formats: Vec<&'static str> = vec!["mp3", "wav", "flac"],
    pub max_duration_seconds: u32 = 7200,    // 2 hours max
    
    // Content validation
    pub require_audio_header: bool = true,
    pub scan_for_malware: bool = true,
    pub validate_metadata: bool = true,
}

impl AudioValidator {
    pub fn validate_upload(&self, file: &[u8]) -> Result<ValidationResult, SecurityError> {
        // 1. Size validation (first check for DoS protection)
        if file.len() > self.rules.max_file_size {
            return Err(SecurityError::FileTooLarge(file.len()));
        }
        
        // 2. Magic byte validation
        let format = self.detect_format(file)?;
        if !self.rules.allowed_formats.contains(&format.as_str()) {
            return Err(SecurityError::InvalidFileFormat(format));
        }
        
        // 3. Header integrity check
        self.validate_audio_header(file)?;
        
        // 4. Content scanning
        if self.rules.scan_for_malware {
            self.malware_scan(file)?;
        }
        
        // 5. Metadata sanitization
        let sanitized_metadata = self.sanitize_metadata(file)?;
        
        Ok(ValidationResult {
            format,
            duration: self.calculate_duration(file)?,
            metadata: sanitized_metadata,
        })
    }
}
```

#### API Parameter Validation

```rust
pub struct ApiValidation;

impl ApiValidation {
    // String input validation with length limits
    pub fn validate_system_id(id: &str) -> Result<SystemId, ValidationError> {
        if id.is_empty() || id.len() > 50 {
            return Err(ValidationError::InvalidLength("system_id", 1, 50));
        }
        
        if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(ValidationError::InvalidCharacters("system_id"));
        }
        
        Ok(SystemId::from(id))
    }
    
    // Numeric validation with bounds checking
    pub fn validate_frequency(freq: u64) -> Result<Frequency, ValidationError> {
        const MIN_FREQ: u64 = 25_000_000;    // 25 MHz
        const MAX_FREQ: u64 = 1_300_000_000; // 1.3 GHz
        
        if freq < MIN_FREQ || freq > MAX_FREQ {
            return Err(ValidationError::OutOfRange("frequency", MIN_FREQ, MAX_FREQ));
        }
        
        Ok(freq)
    }
    
    // Timestamp validation (prevent replay attacks)
    pub fn validate_timestamp(ts: DateTime<Utc>) -> Result<DateTime<Utc>, ValidationError> {
        let now = Utc::now();
        let max_age = Duration::hours(24);
        let max_future = Duration::minutes(5);
        
        if ts < now - max_age {
            return Err(ValidationError::TimestampTooOld);
        }
        
        if ts > now + max_future {
            return Err(ValidationError::TimestampTooNew);
        }
        
        Ok(ts)
    }
}
```

### 1.3 Authentication/Authorization Patterns

#### API Key Management

```rust
pub struct ApiKeyConfig {
    // Key properties
    pub key_id: String,
    pub key_hash: [u8; 32],        // Argon2id hash
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    
    // Access control
    pub allowed_ips: Vec<IpAddr>,
    pub allowed_systems: Vec<SystemId>,
    pub rate_limits: RateLimits,
    pub permissions: EnumSet<Permission>,
    
    // Security features
    pub require_tls: bool = true,
    pub max_request_size: usize = 100_000_000,
    pub session_timeout_seconds: u64 = 3600,
}

// Rate limiting per API key
pub struct RateLimits {
    pub requests_per_second: u32 = 100,
    pub requests_per_minute: u32 = 1000,
    pub requests_per_hour: u32 = 10000,
    pub uploads_per_hour: u32 = 1000,
    pub bandwidth_per_hour_mb: u32 = 1000,
}

#[derive(EnumSetType)]
pub enum Permission {
    CallUpload,
    CallRead,
    CallSearch,
    CallDelete,
    StreamAccess,
    SystemAdmin,
    AnalyticsRead,
    ConfigRead,
    ConfigWrite,
}
```

#### Authentication Implementation

```rust
pub struct AuthService {
    key_store: Arc<RwLock<HashMap<String, ApiKeyConfig>>>,
    rate_limiter: Arc<RwLock<HashMap<String, RateLimitState>>>,
    failed_attempts: Arc<RwLock<HashMap<IpAddr, FailedAttempts>>>,
}

impl AuthService {
    pub async fn authenticate(&self, request: &HttpRequest) -> Result<AuthContext, AuthError> {
        // 1. Extract API key from header
        let api_key = self.extract_api_key(request)?;
        
        // 2. Validate key format (prevent timing attacks)
        if api_key.len() != 64 || !api_key.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(AuthError::InvalidKeyFormat);
        }
        
        // 3. Rate limiting check (before expensive operations)
        let client_ip = self.get_client_ip(request);
        self.check_rate_limits(&api_key, client_ip).await?;
        
        // 4. Key validation (constant-time comparison)
        let key_config = self.validate_key(&api_key).await?;
        
        // 5. IP address validation
        if !key_config.allowed_ips.is_empty() {
            if !key_config.allowed_ips.contains(&client_ip) {
                self.log_failed_attempt(client_ip, "ip_not_allowed");
                return Err(AuthError::IpNotAllowed);
            }
        }
        
        // 6. Expiration check
        if let Some(expires_at) = key_config.expires_at {
            if Utc::now() > expires_at {
                return Err(AuthError::KeyExpired);
            }
        }
        
        Ok(AuthContext {
            key_id: key_config.key_id,
            permissions: key_config.permissions,
            rate_limits: key_config.rate_limits,
            client_ip,
        })
    }
}
```

### 1.4 Cryptography Requirements

#### Approved Cryptographic Algorithms

```rust
// Use only FIPS 140-2 approved algorithms
pub struct CryptoConfig {
    // Hashing (API keys, passwords)
    pub hash_algorithm: HashAlgorithm = HashAlgorithm::Argon2id,
    pub hash_iterations: u32 = 100_000,
    pub hash_memory_kb: u32 = 65_536,
    pub hash_parallelism: u32 = 4,
    
    // Symmetric encryption (data at rest)
    pub symmetric_cipher: SymmetricCipher = SymmetricCipher::ChaCha20Poly1305,
    pub key_derivation: KeyDerivation = KeyDerivation::PBKDF2,
    
    // Transport security
    pub tls_version: TlsVersion = TlsVersion::V1_3,
    pub cipher_suites: Vec<CipherSuite> = vec![
        CipherSuite::TLS_CHACHA20_POLY1305_SHA256,
        CipherSuite::TLS_AES_256_GCM_SHA384,
    ],
}

// Key management implementation
impl CryptoService {
    // Generate secure random keys
    pub fn generate_api_key() -> String {
        let mut rng = ring::rand::SystemRandom::new();
        let mut key_bytes = [0u8; 32];
        rng.fill(&mut key_bytes).expect("RNG failure");
        hex::encode(key_bytes)
    }
    
    // Constant-time key comparison
    pub fn verify_api_key(&self, provided: &str, stored_hash: &[u8]) -> bool {
        let provided_bytes = match hex::decode(provided) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        
        ring::constant_time::verify_slices_are_equal(&provided_bytes, stored_hash).is_ok()
    }
    
    // Secure password hashing
    pub fn hash_password(&self, password: &str, salt: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let config = argon2::Config {
            variant: argon2::Variant::Argon2id,
            version: argon2::Version::Version13,
            mem_cost: self.config.hash_memory_kb,
            time_cost: self.config.hash_iterations,
            lanes: self.config.hash_parallelism,
            secret: &[],
            ad: &[],
            hash_length: 32,
        };
        
        argon2::hash_raw(password.as_bytes(), salt, &config)
            .map_err(CryptoError::from)
    }
}
```

### 1.5 Dependency Security Policies

#### Dependency Management

```toml
# Cargo.toml security requirements
[workspace.metadata.audit]
# Require all dependencies to be audited
require-audit = true

# Vulnerability database requirements  
[workspace.metadata.security]
# Maximum CVSS score allowed
max_cvss_score = 7.0

# Required security updates within timeframes
critical_update_hours = 24
high_update_hours = 72
medium_update_days = 7
low_update_days = 30

# Banned dependencies (known security issues)
banned_crates = [
    "openssl",      # Use rustls instead
    "native-tls",   # Use rustls instead  
    "yaml-rust",    # Unmaintained, use serde_yaml
]
```

#### Automated Security Scanning

```bash
#!/bin/bash
# security_scan.sh - Run daily

# Check for vulnerabilities
cargo audit --deny warnings

# Check for banned dependencies
cargo deny check bans

# Check licenses
cargo deny check licenses

# Check for outdated dependencies
cargo outdated --exit-code 1

# Check for unused dependencies  
cargo machete

# Static analysis
cargo clippy -- -D warnings -D clippy::all -D clippy::pedantic

# Memory safety verification
cargo geiger --forbid-unsafe
```

### 1.6 Vulnerability Disclosure Process

#### Internal Vulnerability Handling

```rust
pub struct VulnerabilityReport {
    pub id: Uuid,
    pub severity: Severity,
    pub component: String,
    pub description: String,
    pub reporter: String,
    pub created_at: DateTime<Utc>,
    pub status: VulnerabilityStatus,
    pub fix_deadline: DateTime<Utc>,
    pub cve_id: Option<String>,
}

pub enum Severity {
    Critical,    // Fix within 24 hours
    High,        // Fix within 72 hours  
    Medium,      // Fix within 1 week
    Low,         // Fix within 1 month
}

pub enum VulnerabilityStatus {
    Reported,
    Acknowledged,
    InProgress,
    Fixed,
    Closed,
}
```

---

## 2. Performance Requirements

### 2.1 Memory Allocation Limits

#### Memory Pool Configuration

```rust
pub struct MemoryLimits {
    // Global limits
    pub max_heap_size_mb: usize = 2048,        // 2GB baseline
    pub max_stack_size_mb: usize = 8,          // 8MB stack
    
    // Per-component limits
    pub audio_buffer_pool_mb: usize = 512,     // 512MB for audio buffers
    pub transcription_worker_mb: usize = 256,  // 256MB per worker
    pub database_pool_mb: usize = 128,         // 128MB connection pool
    pub cache_size_mb: usize = 256,            // 256MB Redis cache
    
    // Request limits
    pub max_request_size_mb: usize = 100,      // 100MB max upload
    pub max_concurrent_uploads: usize = 50,    // Limit concurrent uploads
    pub upload_buffer_kb: usize = 64,          // 64KB streaming buffer
}

// Memory pool implementation
pub struct AudioBufferPool {
    pool: Arc<Mutex<Vec<Vec<f32>>>>,
    max_buffers: usize,
    buffer_size: usize,
    allocated_bytes: AtomicUsize,
    max_allocated_bytes: usize,
}

impl AudioBufferPool {
    pub fn get_buffer(&self) -> Result<Vec<f32>, MemoryError> {
        let mut pool = self.pool.lock().unwrap();
        
        // Check memory limits
        let current_allocated = self.allocated_bytes.load(Ordering::Acquire);
        if current_allocated + (self.buffer_size * 4) > self.max_allocated_bytes {
            return Err(MemoryError::PoolExhausted);
        }
        
        // Reuse existing buffer or create new one
        match pool.pop() {
            Some(mut buffer) => {
                buffer.clear();
                buffer.reserve(self.buffer_size);
                Ok(buffer)
            }
            None => {
                self.allocated_bytes.fetch_add(self.buffer_size * 4, Ordering::Release);
                Ok(Vec::with_capacity(self.buffer_size))
            }
        }
    }
    
    pub fn return_buffer(&self, buffer: Vec<f32>) {
        let mut pool = self.pool.lock().unwrap();
        if pool.len() < self.max_buffers {
            pool.push(buffer);
        } else {
            // Buffer pool full, deallocate
            self.allocated_bytes.fetch_sub(buffer.capacity() * 4, Ordering::Release);
        }
    }
}
```

#### Memory Monitoring

```rust
pub struct MemoryMonitor {
    metrics: Arc<Metrics>,
    limits: MemoryLimits,
}

impl MemoryMonitor {
    pub async fn monitor_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        
        loop {
            interval.tick().await;
            
            let usage = self.collect_memory_stats().await;
            self.metrics.memory_usage.set(usage.total_mb as f64);
            
            // Alert on high memory usage
            if usage.total_mb > self.limits.max_heap_size_mb * 85 / 100 {
                tracing::warn!(
                    usage_mb = usage.total_mb,
                    limit_mb = self.limits.max_heap_size_mb,
                    "High memory usage detected"
                );
            }
            
            // Emergency garbage collection
            if usage.total_mb > self.limits.max_heap_size_mb * 95 / 100 {
                tracing::error!(
                    usage_mb = usage.total_mb,
                    limit_mb = self.limits.max_heap_size_mb,
                    "Critical memory usage - triggering emergency cleanup"
                );
                self.emergency_cleanup().await;
            }
        }
    }
}
```

### 2.2 Latency Targets for Operations

#### Performance SLA Definitions

```rust
pub struct PerformanceTargets {
    // API response times (95th percentile)
    pub api_upload_p95_ms: u64 = 100,          // File upload API
    pub api_query_p95_ms: u64 = 50,            // Search/query APIs
    pub api_metadata_p95_ms: u64 = 25,         // Metadata APIs
    
    // Processing latencies
    pub file_detection_ms: u64 = 1000,         // File system detection
    pub metadata_extraction_ms: u64 = 2000,    // Audio metadata extraction  
    pub database_insert_ms: u64 = 100,         // Database operations
    pub cache_lookup_ms: u64 = 10,             // Redis cache operations
    
    // Transcription processing (varies by audio length)
    pub transcription_ratio: f64 = 0.5,        // 0.5x real-time (30s for 60s audio)
    pub transcription_startup_ms: u64 = 5000,  // WhisperX initialization
    pub diarization_overhead: f64 = 0.2,       // 20% overhead for speaker diarization
    
    // Streaming latencies
    pub websocket_latency_ms: u64 = 100,       // WebSocket message delivery
    pub icecast_latency_ms: u64 = 2000,        // Live stream latency
    pub audio_buffer_ms: u64 = 500,            // Audio buffer duration
}

// Latency measurement implementation
pub struct LatencyTracker {
    histograms: HashMap<String, Histogram>,
    targets: PerformanceTargets,
}

impl LatencyTracker {
    pub fn record_operation<F, R>(&self, operation: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();
        
        if let Some(histogram) = self.histograms.get(operation) {
            histogram.observe(duration.as_millis() as f64);
        }
        
        // Alert on SLA violations
        let target_ms = self.get_target_for_operation(operation);
        if duration.as_millis() as u64 > target_ms {
            tracing::warn!(
                operation = operation,
                duration_ms = duration.as_millis(),
                target_ms = target_ms,
                "Performance target exceeded"
            );
        }
        
        result
    }
}
```

### 2.3 Resource Utilization Caps

#### System Resource Limits

```rust
pub struct ResourceLimits {
    // CPU utilization
    pub max_cpu_percent: f64 = 85.0,           // 85% sustained CPU
    pub cpu_alert_threshold: f64 = 70.0,       // Alert at 70%
    pub max_cpu_cores: usize = 16,             // Limit CPU cores used
    
    // Network bandwidth  
    pub max_bandwidth_mbps: u64 = 1000,        // 1 Gbps max bandwidth
    pub max_connections: usize = 1000,         // Max concurrent connections
    pub connection_timeout_seconds: u64 = 30,  // Connection timeout
    
    // Disk I/O
    pub max_disk_io_mbps: u64 = 500,          // 500 MB/s disk I/O
    pub max_open_files: usize = 10000,        // File descriptor limit
    pub disk_space_threshold_percent: f64 = 90.0, // Alert at 90% full
    
    // Database connections
    pub max_db_connections: u32 = 50,         // PostgreSQL connection pool
    pub db_connection_timeout_ms: u64 = 5000, // Connection timeout
    pub query_timeout_ms: u64 = 30000,       // Query timeout
}

// Resource monitoring implementation
pub struct ResourceMonitor {
    limits: ResourceLimits,
    metrics: Arc<Metrics>,
    alerts: AlertManager,
}

impl ResourceMonitor {
    pub async fn enforce_limits(&self) -> Result<(), ResourceError> {
        // CPU utilization check
        let cpu_usage = self.get_cpu_usage().await?;
        if cpu_usage > self.limits.max_cpu_percent {
            self.alerts.send_alert(Alert::CpuLimitExceeded { 
                usage: cpu_usage,
                limit: self.limits.max_cpu_percent,
            });
            
            // Throttle processing if critical
            if cpu_usage > 95.0 {
                return Err(ResourceError::CpuExhausted);
            }
        }
        
        // Memory check
        let memory_usage = self.get_memory_usage().await?;
        if memory_usage.percent > 90.0 {
            self.alerts.send_alert(Alert::MemoryLimitExceeded {
                usage_mb: memory_usage.used_mb,
                total_mb: memory_usage.total_mb,
            });
        }
        
        // Network bandwidth check
        let network_stats = self.get_network_stats().await?;
        if network_stats.bandwidth_mbps > self.limits.max_bandwidth_mbps {
            return Err(ResourceError::BandwidthExceeded);
        }
        
        Ok(())
    }
}
```

### 2.4 Profiling Requirements

#### Mandatory Profiling Integration

```rust
// Profile configuration
pub struct ProfilingConfig {
    pub enabled: bool = true,
    pub sample_rate_hz: u32 = 100,             // 100 samples/second
    pub profile_duration_seconds: u64 = 300,   // 5-minute profiles
    pub retention_days: u32 = 7,               // Keep profiles for 7 days
    
    // Profiling targets
    pub profile_cpu: bool = true,
    pub profile_memory: bool = true, 
    pub profile_locks: bool = true,
    pub profile_async_tasks: bool = true,
}

// Built-in profiling instrumentation
pub struct ProfiledApplication {
    app: SdrTrunkTranscriber,
    profiler: Arc<Profiler>,
}

impl ProfiledApplication {
    pub async fn run_with_profiling(&self) -> Result<(), AppError> {
        // Start continuous profiling
        let _profiler_guard = self.profiler.start_continuous_profiling();
        
        // Profile application startup
        let startup_profile = self.profiler.profile_async("application_startup", async {
            self.app.initialize().await
        }).await?;
        
        tracing::info!(
            startup_time_ms = startup_profile.duration_ms,
            "Application startup completed"
        );
        
        // Run with periodic profiling snapshots
        self.run_with_periodic_snapshots().await
    }
    
    async fn run_with_periodic_snapshots(&self) -> Result<(), AppError> {
        let mut interval = tokio::time::interval(
            Duration::from_secs(self.profiler.config.profile_duration_seconds)
        );
        
        loop {
            interval.tick().await;
            
            // Generate CPU profile
            let cpu_profile = self.profiler.capture_cpu_profile().await?;
            self.analyze_cpu_hotspots(&cpu_profile).await;
            
            // Generate memory profile
            let memory_profile = self.profiler.capture_memory_profile().await?;
            self.analyze_memory_usage(&memory_profile).await;
            
            // Check for performance regressions
            self.detect_performance_regressions().await?;
        }
    }
}
```

#### Performance Benchmarking

```rust
// Automated performance benchmarks
use criterion::{Criterion, BenchmarkId, Throughput};

pub fn benchmark_audio_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("audio_processing");
    
    // Test different buffer sizes
    for buffer_size in [1024, 2048, 4096, 8192].iter() {
        group.throughput(Throughput::Elements(*buffer_size as u64));
        group.bench_with_input(
            BenchmarkId::new("frame_processing", buffer_size),
            buffer_size,
            |b, &size| {
                let frame = create_test_audio_frame(size);
                b.iter(|| {
                    process_audio_frame(&frame)
                });
            },
        );
    }
    
    group.finish();
}

pub fn benchmark_transcription_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("transcription");
    
    // Benchmark different audio lengths
    for duration_seconds in [30, 60, 300, 600].iter() {
        group.bench_with_input(
            BenchmarkId::new("whisperx_transcription", duration_seconds),
            duration_seconds,
            |b, &duration| {
                let audio_data = generate_test_audio(duration);
                b.iter(|| {
                    transcribe_audio_sync(&audio_data)
                });
            },
        );
    }
    
    group.finish();
}

// Performance regression detection
pub struct RegressionDetector {
    baseline_metrics: HashMap<String, f64>,
    threshold_percent: f64,
}

impl RegressionDetector {
    pub fn detect_regression(&self, metric: &str, current_value: f64) -> Option<Regression> {
        if let Some(&baseline) = self.baseline_metrics.get(metric) {
            let change_percent = ((current_value - baseline) / baseline) * 100.0;
            
            if change_percent > self.threshold_percent {
                Some(Regression {
                    metric: metric.to_string(),
                    baseline_value: baseline,
                    current_value,
                    regression_percent: change_percent,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}
```

### 2.5 Optimization Guidelines

#### Code-Level Optimizations

```rust
// Memory-efficient processing patterns
pub struct OptimizedAudioProcessor {
    // Reuse allocations to minimize GC pressure
    scratch_buffer: Vec<f32>,
    fft_planner: Arc<dyn rustfft::Fft<f32>>,
    window_function: Vec<f32>,
}

impl OptimizedAudioProcessor {
    // Zero-allocation audio processing
    pub fn process_frame_in_place(&mut self, samples: &mut [f32]) -> Result<(), ProcessingError> {
        // Reuse scratch buffer to avoid allocation
        self.scratch_buffer.clear();
        self.scratch_buffer.extend_from_slice(samples);
        
        // Apply window function (SIMD optimized)
        for (sample, window) in self.scratch_buffer.iter_mut().zip(&self.window_function) {
            *sample *= window;
        }
        
        // In-place FFT processing
        self.fft_planner.process(&mut self.scratch_buffer);
        
        // Copy results back (avoid allocation)
        samples.copy_from_slice(&self.scratch_buffer[..samples.len()]);
        
        Ok(())
    }
    
    // Batch processing for better cache locality
    pub fn process_batch(&mut self, frames: &mut [AudioFrame]) -> Result<(), ProcessingError> {
        // Process frames in cache-friendly chunks
        const BATCH_SIZE: usize = 16;
        
        for chunk in frames.chunks_mut(BATCH_SIZE) {
            for frame in chunk {
                self.process_frame_in_place(&mut frame.samples)?;
            }
        }
        
        Ok(())
    }
}

// Database query optimization
pub struct OptimizedQueries;

impl OptimizedQueries {
    // Use prepared statements and connection pooling
    pub async fn search_calls_optimized(
        &self,
        pool: &PgPool,
        params: &SearchParams,
    ) -> Result<Vec<RadioCall>, sqlx::Error> {
        // Use explicit query with indexes
        sqlx::query_as!(
            RadioCall,
            r#"
            SELECT * FROM radio_calls 
            WHERE ($1::text IS NULL OR system_id = $1)
              AND ($2::timestamptz IS NULL OR call_timestamp >= $2)  
              AND ($3::timestamptz IS NULL OR call_timestamp <= $3)
              AND ($4::text IS NULL OR to_tsvector('english', transcription_text) @@ plainto_tsquery('english', $4))
            ORDER BY call_timestamp DESC
            LIMIT $5 OFFSET $6
            "#,
            params.system_id,
            params.start_date,
            params.end_date,
            params.search_text,
            params.limit,
            params.offset
        )
        .fetch_all(pool)
        .await
    }
}
```

---

## 3. Audit Requirements

### 3.1 Security Audit Frequency

#### Mandatory Audit Schedule

```rust
pub struct AuditSchedule {
    // Internal audits
    pub code_review_frequency: Duration = Duration::days(7),
    pub dependency_scan_frequency: Duration = Duration::days(1),
    pub vulnerability_scan_frequency: Duration = Duration::hours(12),
    pub penetration_test_frequency: Duration = Duration::days(90),
    
    // External audits
    pub third_party_security_audit: Duration = Duration::days(365),
    pub compliance_audit_frequency: Duration = Duration::days(180),
    pub code_audit_frequency: Duration = Duration::days(90),
    
    // Continuous monitoring
    pub log_analysis_frequency: Duration = Duration::minutes(15),
    pub anomaly_detection_frequency: Duration = Duration::minutes(5),
    pub performance_audit_frequency: Duration = Duration::hours(24),
}
```

#### Automated Audit Pipeline

```bash
#!/bin/bash
# audit_pipeline.sh - Runs every 12 hours

set -euo pipefail

echo "Starting automated security audit..."

# 1. Dependency vulnerability scan
echo "Scanning dependencies..."
cargo audit --deny warnings --json > audit_results.json

# 2. Static code analysis
echo "Running static analysis..."
cargo clippy --all-targets --all-features -- -D warnings

# 3. Security linting
echo "Security-focused linting..."
cargo semver-checks check-release
cargo geiger --forbid-unsafe

# 4. License compliance
echo "Checking licenses..."
cargo deny check licenses

# 5. Performance regression tests
echo "Performance regression testing..."
cargo bench --bench security_benchmarks

# 6. Memory safety verification
echo "Memory safety checks..."
MIRIFLAGS="-Zmiri-strict-provenance -Zmiri-symbolic-alignment-check" cargo +nightly miri test

# 7. Generate audit report
echo "Generating audit report..."
python3 scripts/generate_audit_report.py \
    --audit-results audit_results.json \
    --clippy-results clippy_results.json \
    --bench-results benchmark_results.json \
    --output audit_report_$(date +%Y%m%d_%H%M%S).html

echo "Audit completed successfully"
```

### 3.2 Penetration Testing Requirements

#### Quarterly Penetration Test Scope

```rust
pub struct PenetrationTestScope {
    // Network security testing
    pub network_scanning: bool = true,
    pub port_scanning: bool = true,
    pub service_enumeration: bool = true,
    
    // Application security testing
    pub api_security_testing: bool = true,
    pub authentication_bypass: bool = true,
    pub authorization_testing: bool = true,
    pub input_validation_testing: bool = true,
    
    // File upload security
    pub malicious_file_upload: bool = true,
    pub file_type_bypass: bool = true,
    pub path_traversal_testing: bool = true,
    
    // DoS/DDoS testing
    pub rate_limiting_testing: bool = true,
    pub resource_exhaustion: bool = true,
    pub application_dos: bool = true,
    
    // Data security
    pub sql_injection_testing: bool = true,
    pub data_extraction_testing: bool = true,
    pub privacy_testing: bool = true,
}

// Automated penetration testing integration
pub struct PenTestAutomation {
    tools: Vec<PenTestTool>,
    results_aggregator: ResultsAggregator,
}

impl PenTestAutomation {
    pub async fn run_automated_pentest(&self) -> Result<PenTestReport, PenTestError> {
        let mut results = Vec::new();
        
        // Network scanning with nmap
        let nmap_results = self.run_nmap_scan().await?;
        results.push(TestResult::NetworkScan(nmap_results));
        
        // Web application scanning
        let web_scan_results = self.run_web_app_scan().await?;
        results.push(TestResult::WebAppScan(web_scan_results));
        
        // API security testing
        let api_test_results = self.run_api_security_tests().await?;
        results.push(TestResult::ApiSecurity(api_test_results));
        
        // Generate comprehensive report
        Ok(self.results_aggregator.generate_report(results))
    }
    
    async fn run_api_security_tests(&self) -> Result<ApiSecurityResults, PenTestError> {
        let mut results = ApiSecurityResults::new();
        
        // Test authentication bypass
        results.auth_bypass = self.test_auth_bypass().await?;
        
        // Test rate limiting
        results.rate_limiting = self.test_rate_limiting().await?;
        
        // Test input validation
        results.input_validation = self.test_input_validation().await?;
        
        // Test file upload security
        results.file_upload = self.test_file_upload_security().await?;
        
        Ok(results)
    }
}
```

### 3.3 Compliance Standards

#### Required Compliance Frameworks

```rust
pub struct ComplianceRequirements {
    // Security frameworks
    pub nist_cybersecurity_framework: bool = true,
    pub owasp_top_10: bool = true,
    pub sans_top_25: bool = true,
    
    // Industry standards  
    pub iso_27001: bool = true,
    pub soc2_type2: bool = false,  // Optional for internal use
    
    // Government requirements (if applicable)
    pub fips_140_2: bool = true,   // Cryptographic modules
    pub fedramp_moderate: bool = false,  // If used by government
    
    // Privacy regulations
    pub gdpr_compliance: bool = true,     // EU users
    pub ccpa_compliance: bool = true,     // California users
    pub pipeda_compliance: bool = true,   // Canadian users
}

// Compliance monitoring implementation
pub struct ComplianceMonitor {
    requirements: ComplianceRequirements,
    evidence_collector: EvidenceCollector,
    audit_trail: AuditTrail,
}

impl ComplianceMonitor {
    pub async fn generate_compliance_report(&self) -> Result<ComplianceReport, ComplianceError> {
        let mut report = ComplianceReport::new();
        
        // NIST Cybersecurity Framework assessment
        if self.requirements.nist_cybersecurity_framework {
            report.nist_assessment = self.assess_nist_compliance().await?;
        }
        
        // OWASP Top 10 assessment
        if self.requirements.owasp_top_10 {
            report.owasp_assessment = self.assess_owasp_compliance().await?;
        }
        
        // ISO 27001 controls assessment
        if self.requirements.iso_27001 {
            report.iso27001_assessment = self.assess_iso27001_compliance().await?;
        }
        
        // Privacy compliance assessment
        report.privacy_assessment = self.assess_privacy_compliance().await?;
        
        Ok(report)
    }
    
    async fn assess_nist_compliance(&self) -> Result<NistAssessment, ComplianceError> {
        let mut assessment = NistAssessment::new();
        
        // Identify (ID)
        assessment.identify_score = self.assess_asset_management().await?;
        
        // Protect (PR)
        assessment.protect_score = self.assess_protective_controls().await?;
        
        // Detect (DE)
        assessment.detect_score = self.assess_detection_capabilities().await?;
        
        // Respond (RS)
        assessment.respond_score = self.assess_response_capabilities().await?;
        
        // Recover (RC)
        assessment.recover_score = self.assess_recovery_capabilities().await?;
        
        Ok(assessment)
    }
}
```

### 3.4 Logging and Monitoring Requirements

#### Comprehensive Security Logging

```rust
pub struct SecurityLogger {
    log_appender: Arc<tracing_appender::RollingFileAppender>,
    siem_forwarder: SiemForwarder,
    log_encryption: LogEncryption,
}

impl SecurityLogger {
    // Log all authentication events
    pub fn log_authentication_event(&self, event: AuthEvent) {
        let log_entry = LogEntry {
            timestamp: Utc::now(),
            event_type: EventType::Authentication,
            severity: match event.result {
                AuthResult::Success => Severity::Info,
                AuthResult::Failed => Severity::Warning,
                AuthResult::Blocked => Severity::Error,
            },
            source_ip: event.source_ip,
            user_agent: event.user_agent,
            details: serde_json::to_value(&event).unwrap(),
        };
        
        // Local logging
        tracing::info!(
            target: "security.auth",
            event = ?event,
            "Authentication event"
        );
        
        // Forward to SIEM
        if matches!(event.result, AuthResult::Failed | AuthResult::Blocked) {
            self.siem_forwarder.send_alert(log_entry.clone());
        }
    }
    
    // Log all data access events
    pub fn log_data_access(&self, access: DataAccessEvent) {
        tracing::info!(
            target: "security.data_access",
            user_id = access.user_id,
            resource = access.resource,
            action = access.action,
            ip_address = %access.ip_address,
            timestamp = %access.timestamp,
            "Data access event"
        );
        
        // Log sensitive data access separately
        if access.is_sensitive {
            tracing::warn!(
                target: "security.sensitive_access",
                event = ?access,
                "Sensitive data accessed"
            );
        }
    }
    
    // Log security violations
    pub fn log_security_violation(&self, violation: SecurityViolation) {
        tracing::error!(
            target: "security.violations",
            violation_type = %violation.violation_type,
            severity = %violation.severity,
            source_ip = %violation.source_ip,
            details = %violation.details,
            "Security violation detected"
        );
        
        // Immediate SIEM alert for critical violations
        if matches!(violation.severity, Severity::Critical | Severity::High) {
            self.siem_forwarder.send_critical_alert(violation);
        }
    }
}

// Required log retention
pub struct LogRetentionPolicy {
    // Security logs
    pub authentication_logs_days: u32 = 365,      // 1 year
    pub audit_logs_days: u32 = 2555,              // 7 years
    pub access_logs_days: u32 = 90,               // 90 days
    pub error_logs_days: u32 = 180,               // 6 months
    
    // Application logs
    pub application_logs_days: u32 = 30,          // 30 days
    pub performance_logs_days: u32 = 14,          // 14 days
    pub debug_logs_days: u32 = 7,                 // 7 days
    
    // Compliance requirements
    pub regulatory_logs_days: u32 = 2555,         // 7 years for compliance
    pub financial_logs_days: u32 = 2555,          // 7 years for financial records
}
```

---

## 4. Operational Security

### 4.1 Secret Management

#### HashiCorp Vault Integration

```rust
pub struct VaultSecretManager {
    client: vaultrs::Client,
    config: VaultConfig,
    cache: Arc<RwLock<HashMap<String, CachedSecret>>>,
}

impl VaultSecretManager {
    pub async fn new(config: VaultConfig) -> Result<Self, VaultError> {
        let client = vaultrs::Client::new(
            vaultrs::Config::builder()
                .address(&config.vault_url)
                .token(&config.vault_token)
                .build()?,
        );
        
        // Verify connectivity and authentication
        client.sys().health().await?;
        
        Ok(Self {
            client,
            config,
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    // Retrieve secret with caching and automatic refresh
    pub async fn get_secret(&self, path: &str) -> Result<String, VaultError> {
        // Check cache first
        if let Some(cached) = self.get_from_cache(path).await {
            if !cached.is_expired() {
                return Ok(cached.value);
            }
        }
        
        // Fetch from Vault
        let secret_data = self.client
            .kv2()
            .read(&self.config.mount_path, path)
            .await?
            .data;
        
        let secret_value = secret_data
            .get("value")
            .ok_or(VaultError::SecretNotFound)?
            .as_str()
            .ok_or(VaultError::InvalidSecretFormat)?
            .to_string();
        
        // Update cache
        self.update_cache(path, &secret_value).await;
        
        Ok(secret_value)
    }
    
    // Store secret securely
    pub async fn store_secret(
        &self, 
        path: &str, 
        value: &str,
        metadata: SecretMetadata,
    ) -> Result<(), VaultError> {
        let secret_data = HashMap::from([
            ("value".to_string(), serde_json::Value::String(value.to_string())),
            ("created_by".to_string(), serde_json::Value::String(metadata.created_by)),
            ("purpose".to_string(), serde_json::Value::String(metadata.purpose)),
        ]);
        
        self.client
            .kv2()
            .set(&self.config.mount_path, path, &secret_data)
            .await?;
        
        // Clear cache to force refresh
        self.clear_cache_entry(path).await;
        
        Ok(())
    }
}

// Secret rotation automation
pub struct SecretRotationManager {
    vault: Arc<VaultSecretManager>,
    rotation_schedule: HashMap<String, RotationPolicy>,
}

impl SecretRotationManager {
    pub async fn start_rotation_scheduler(&self) -> Result<(), RotationError> {
        let mut interval = tokio::time::interval(Duration::from_hours(1));
        
        loop {
            interval.tick().await;
            
            for (secret_path, policy) in &self.rotation_schedule {
                if self.needs_rotation(secret_path, policy).await? {
                    self.rotate_secret(secret_path).await?;
                }
            }
        }
    }
    
    async fn rotate_secret(&self, secret_path: &str) -> Result<(), RotationError> {
        match secret_path {
            "database/credentials" => self.rotate_database_credentials().await?,
            "api/keys" => self.rotate_api_keys().await?,
            "encryption/keys" => self.rotate_encryption_keys().await?,
            _ => return Err(RotationError::UnsupportedSecretType),
        }
        
        tracing::info!(
            secret_path = secret_path,
            "Secret rotated successfully"
        );
        
        Ok(())
    }
}
```

### 4.2 Environment Variable Handling

#### Secure Environment Configuration

```rust
pub struct EnvironmentConfig {
    // Never log these environment variables
    sensitive_vars: HashSet<String>,
    // Required environment variables
    required_vars: Vec<String>,
    // Environment variable validation rules
    validation_rules: HashMap<String, ValidationRule>,
}

impl EnvironmentConfig {
    pub fn new() -> Self {
        let sensitive_vars = [
            "DATABASE_PASSWORD",
            "VAULT_TOKEN", 
            "API_SECRET_KEY",
            "ENCRYPTION_KEY",
            "REDIS_PASSWORD",
            "WHISPERX_API_KEY",
        ].into_iter().map(String::from).collect();
        
        let required_vars = vec![
            "DATABASE_URL".to_string(),
            "VAULT_URL".to_string(),
            "LOG_LEVEL".to_string(),
            "API_BIND_ADDRESS".to_string(),
        ];
        
        Self {
            sensitive_vars,
            required_vars,
            validation_rules: HashMap::new(),
        }
    }
    
    pub fn validate_environment(&self) -> Result<(), ConfigError> {
        // Check required variables exist
        for var in &self.required_vars {
            if env::var(var).is_err() {
                return Err(ConfigError::MissingRequiredVariable(var.clone()));
            }
        }
        
        // Validate variable formats
        for (var, rule) in &self.validation_rules {
            if let Ok(value) = env::var(var) {
                rule.validate(&value)?;
            }
        }
        
        // Check for accidentally exposed secrets
        self.check_for_exposed_secrets()?;
        
        Ok(())
    }
    
    fn check_for_exposed_secrets(&self) -> Result<(), ConfigError> {
        // Check if sensitive variables appear in logs or config files
        for sensitive_var in &self.sensitive_vars {
            if let Ok(value) = env::var(sensitive_var) {
                if value.len() < 8 {
                    return Err(ConfigError::WeakSecret(sensitive_var.clone()));
                }
                
                // Check if it looks like a default/example value
                if value.contains("example") || value.contains("changeme") || value == "password" {
                    return Err(ConfigError::DefaultSecret(sensitive_var.clone()));
                }
            }
        }
        
        Ok(())
    }
    
    // Safe environment variable access with audit logging
    pub fn get_env_var(&self, key: &str) -> Result<String, ConfigError> {
        let value = env::var(key)
            .map_err(|_| ConfigError::VariableNotFound(key.to_string()))?;
        
        // Log access to sensitive variables (but not the value)
        if self.sensitive_vars.contains(key) {
            tracing::info!(
                target: "security.config",
                variable = key,
                "Sensitive environment variable accessed"
            );
        } else {
            tracing::debug!(
                variable = key,
                value = %value,
                "Environment variable accessed"
            );
        }
        
        Ok(value)
    }
}
```

### 4.3 Container Security

#### Secure Container Configuration

```dockerfile
# Multi-stage build for minimal attack surface
FROM rust:1.82-slim as builder

# Create non-root user for build
RUN groupadd -r rustuser && useradd -r -g rustuser rustuser

# Install only required dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/

# Build dependencies
RUN cargo build --release

# Runtime stage with minimal base image
FROM debian:bookworm-slim

# Security updates and minimal runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root runtime user
RUN groupadd -r -g 1001 sdrtrunk && \
    useradd -r -u 1001 -g sdrtrunk -s /sbin/nologin \
    -c "SDRTrunk Transcriber" sdrtrunk

# Create required directories with proper permissions
RUN mkdir -p /app/data /app/logs /app/config && \
    chown -R sdrtrunk:sdrtrunk /app

# Copy binary from builder stage
COPY --from=builder --chown=sdrtrunk:sdrtrunk /app/target/release/sdrtrunk-transcriber /app/

# Security hardening
RUN chmod 755 /app/sdrtrunk-transcriber && \
    chmod 750 /app/data /app/logs /app/config

# Switch to non-root user
USER sdrtrunk

# Set secure working directory
WORKDIR /app

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Expose only required ports
EXPOSE 8080

# Run application
ENTRYPOINT ["./sdrtrunk-transcriber"]
CMD ["--config", "/app/config/app.toml"]
```

#### Container Security Scanning

```bash
#!/bin/bash
# container_security_scan.sh

set -euo pipefail

IMAGE_NAME="sdrtrunk-transcriber:latest"

echo "Starting container security scan..."

# 1. Build secure image
docker build --no-cache -t "$IMAGE_NAME" .

# 2. Vulnerability scanning with Trivy
echo "Scanning for vulnerabilities..."
trivy image --exit-code 1 --severity HIGH,CRITICAL "$IMAGE_NAME"

# 3. Best practices check with Hadolint
echo "Checking Dockerfile best practices..."
hadolint Dockerfile

# 4. Container configuration security check
echo "Checking container configuration..."
docker-bench-security

# 5. Runtime security check
echo "Runtime security analysis..."
docker run --rm -it --security-opt no-new-privileges \
    --cap-drop ALL \
    --read-only \
    --tmpfs /tmp:rw,noexec,nosuid,size=100m \
    "$IMAGE_NAME" --version

echo "Container security scan completed successfully"
```

### 4.4 Network Security

#### Network Security Configuration

```rust
pub struct NetworkSecurityConfig {
    // TLS configuration
    pub tls_config: TlsConfig,
    // Rate limiting
    pub rate_limits: NetworkRateLimits,
    // IP filtering
    pub ip_filtering: IpFilterConfig,
    // DDoS protection
    pub ddos_protection: DdosProtectionConfig,
}

impl NetworkSecurityConfig {
    pub fn production_defaults() -> Self {
        Self {
            tls_config: TlsConfig {
                min_version: TlsVersion::V1_3,
                cipher_suites: vec![
                    "TLS_AES_256_GCM_SHA384".to_string(),
                    "TLS_CHACHA20_POLY1305_SHA256".to_string(),
                ],
                require_client_certificates: false,
                hsts_max_age_seconds: 31536000, // 1 year
                certificate_path: "/app/certs/server.crt".to_string(),
                private_key_path: "/app/certs/server.key".to_string(),
            },
            rate_limits: NetworkRateLimits {
                requests_per_second_per_ip: 10,
                burst_size: 20,
                upload_bandwidth_limit_mbps: 10,
                concurrent_connections_per_ip: 5,
            },
            ip_filtering: IpFilterConfig {
                enable_geo_blocking: true,
                blocked_countries: vec!["CN", "RU", "KP"].into_iter().map(String::from).collect(),
                whitelist_ips: vec![],
                blacklist_ips: vec![],
                enable_tor_blocking: true,
            },
            ddos_protection: DdosProtectionConfig {
                enable_rate_limiting: true,
                enable_connection_limiting: true,
                suspicious_pattern_detection: true,
                auto_blacklist_threshold: 1000,
                blacklist_duration_minutes: 60,
            },
        }
    }
}

// Network security middleware
pub struct NetworkSecurityMiddleware {
    config: NetworkSecurityConfig,
    rate_limiter: Arc<RwLock<HashMap<IpAddr, RateLimitState>>>,
    ip_filter: IpFilter,
    metrics: Arc<NetworkMetrics>,
}

impl NetworkSecurityMiddleware {
    pub async fn process_request(&self, req: &mut Request) -> Result<(), NetworkSecurityError> {
        let client_ip = self.extract_client_ip(req)?;
        
        // 1. IP filtering (fastest check first)
        if self.ip_filter.is_blocked(&client_ip) {
            self.metrics.blocked_requests.inc();
            return Err(NetworkSecurityError::IpBlocked(client_ip));
        }
        
        // 2. Rate limiting
        if !self.check_rate_limit(&client_ip).await? {
            self.metrics.rate_limited_requests.inc();
            return Err(NetworkSecurityError::RateLimitExceeded(client_ip));
        }
        
        // 3. DDoS detection
        if self.detect_ddos_pattern(&client_ip, req).await? {
            self.auto_blacklist_ip(client_ip).await?;
            return Err(NetworkSecurityError::DdosDetected(client_ip));
        }
        
        // 4. TLS validation
        if !req.is_secure() && self.config.tls_config.require_https {
            return Err(NetworkSecurityError::HttpsRequired);
        }
        
        Ok(())
    }
    
    async fn detect_ddos_pattern(&self, ip: &IpAddr, req: &Request) -> Result<bool, NetworkSecurityError> {
        let patterns = [
            self.detect_high_frequency_requests(ip).await?,
            self.detect_large_payload_flood(ip, req).await?,
            self.detect_connection_exhaustion(ip).await?,
            self.detect_malformed_requests(ip, req).await?,
        ];
        
        // If multiple patterns detected, likely DDoS
        Ok(patterns.iter().filter(|&&p| p).count() >= 2)
    }
}
```

#### Firewall Configuration

```bash
#!/bin/bash
# firewall_rules.sh - Configure iptables for production

set -euo pipefail

# Flush existing rules
iptables -F
iptables -X
iptables -t nat -F
iptables -t nat -X
iptables -t mangle -F
iptables -t mangle -X

# Default policies (deny all, allow only what's needed)
iptables -P INPUT DROP
iptables -P FORWARD DROP
iptables -P OUTPUT ACCEPT

# Allow loopback
iptables -A INPUT -i lo -j ACCEPT
iptables -A OUTPUT -o lo -j ACCEPT

# Allow established connections
iptables -A INPUT -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT

# Allow SSH (limit connections)
iptables -A INPUT -p tcp --dport 22 -m conntrack --ctstate NEW -m limit --limit 3/min --limit-burst 3 -j ACCEPT

# Allow HTTPS (production API)
iptables -A INPUT -p tcp --dport 443 -j ACCEPT

# Allow HTTP (redirect to HTTPS)
iptables -A INPUT -p tcp --dport 80 -j ACCEPT

# PostgreSQL (only from application subnet)
iptables -A INPUT -p tcp -s 10.0.0.0/24 --dport 5432 -j ACCEPT

# Redis (only from application subnet)
iptables -A INPUT -p tcp -s 10.0.0.0/24 --dport 6379 -j ACCEPT

# DDoS protection rules
iptables -A INPUT -p tcp --dport 80 -m limit --limit 25/minute --limit-burst 100 -j ACCEPT
iptables -A INPUT -p tcp --dport 443 -m limit --limit 25/minute --limit-burst 100 -j ACCEPT

# Block common attack ports
iptables -A INPUT -p tcp --dport 23 -j DROP    # Telnet
iptables -A INPUT -p tcp --dport 135 -j DROP   # Windows RPC
iptables -A INPUT -p tcp --dport 139 -j DROP   # NetBIOS
iptables -A INPUT -p tcp --dport 445 -j DROP   # SMB

# Log dropped packets (rate limited)
iptables -A INPUT -m limit --limit 5/min --limit-burst 10 -j LOG --log-prefix "iptables-dropped: "

# Save rules
iptables-save > /etc/iptables/rules.v4

echo "Firewall configuration completed"
```

This comprehensive security and performance guideline provides specific thresholds, tools, and measurements for production deployment of the Rust SDR trunk transcriber. The guidelines enforce zero-unsafe-code policies, implement robust authentication and authorization, define precise performance targets, establish mandatory audit schedules, and ensure secure operational practices.
