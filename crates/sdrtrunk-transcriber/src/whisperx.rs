//! `WhisperX` transcription service implementation

use crate::error::{TranscriptionError, TranscriptionResult};
use crate::service::{AudioValidation, ServiceCapabilities, ServiceHealth, TranscriptionService};
use crate::types::{
    SpeakerSegment, TranscriptionConfig, TranscriptionRequest, TranscriptionResponse,
    TranscriptionSegment, TranscriptionStats, TranscriptionStatus, WordSegment,
};
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tokio::time::{Duration, sleep};
use tracing::{error, info};
use uuid::Uuid;

/// Python service request format
#[derive(Debug, Serialize)]
struct PythonRequest {
    id: Uuid,
    call_id: Uuid,
    audio_path: String,
    requested_at: String,
    options: PythonOptions,
    retry_count: u32,
    priority: i32,
    callback_url: Option<String>,
}

#[derive(Debug, Serialize)]
struct PythonOptions {
    language: Option<String>,
    diarize: bool,
    min_speakers: Option<usize>,
    max_speakers: Option<usize>,
    vad: bool,
    word_timestamps: bool,
    return_confidence: bool,
    max_duration: Option<f64>,
}

/// Python service response format
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct PythonResponse {
    request_id: Uuid,
    call_id: Uuid,
    status: String,
    text: Option<String>,
    language: Option<String>,
    confidence: Option<f32>,
    processing_time_ms: u64,
    segments: Vec<PythonSegment>,
    speaker_segments: Vec<PythonSpeakerSegment>,
    speaker_count: Option<usize>,
    words: Vec<PythonWord>,
    error: Option<String>,
    completed_at: String,
}

#[derive(Debug, Deserialize)]
struct PythonSegment {
    id: usize,
    start: f64,
    end: f64,
    text: String,
    confidence: Option<f32>,
    speaker: Option<String>,
    words: Option<Vec<PythonWord>>,
}

#[derive(Debug, Deserialize)]
struct PythonSpeakerSegment {
    speaker: String,
    start: f64,
    end: f64,
    confidence: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct PythonWord {
    word: String,
    start: f64,
    end: f64,
    confidence: Option<f32>,
    speaker: Option<String>,
}

/// `WhisperX` transcription service
///
/// This service integrates with the Python `WhisperX` service for
/// high-quality transcription with speaker diarization.
pub struct WhisperXService {
    /// Configuration
    config: TranscriptionConfig,

    /// Service URL
    service_url: String,

    /// HTTP client
    client: reqwest::Client,

    /// Python subprocess handle
    python_process: Arc<RwLock<Option<Child>>>,

    /// Whether service is initialized
    initialized: Arc<RwLock<bool>>,

    /// Request tracking
    active_requests: Arc<RwLock<HashMap<Uuid, TranscriptionStatus>>>,
}

impl WhisperXService {
    /// Create a new `WhisperX` service
    pub fn new(config: TranscriptionConfig) -> Self {
        let service_port = config
            .service_port
            .expect("service_port must be configured in transcription config");
        let service_url = format!("http://localhost:{}", service_port);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30)) // Total request timeout
            .connect_timeout(Duration::from_secs(5)) // Connection timeout
            .pool_idle_timeout(Duration::from_secs(90)) // Keep connections alive for 90 seconds
            .pool_max_idle_per_host(10) // Keep up to 10 idle connections
            .build()
            .unwrap();

        Self {
            config,
            service_url,
            client,
            python_process: Arc::new(RwLock::new(None)),
            initialized: Arc::new(RwLock::new(false)),
            active_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the Python service subprocess
    async fn start_python_service(&self) -> TranscriptionResult<()> {
        let python_path = self
            .config
            .python_path
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("python/whisperx_service"));

        info!("Starting Python WhisperX service at {:?}", python_path);

        // Python service will read its configuration from config.toml directly
        // We only need to pass the port if not already configured there

        // Start Python process with multiple workers for better concurrency
        // Note: Workers share the same model in memory, so this is memory-efficient
        let child = Command::new("python")
            .arg("-m")
            .arg("uvicorn")
            .arg("service:app")
            .arg("--host")
            .arg("0.0.0.0")
            .arg("--port")
            .arg(
                self.config
                    .service_port
                    .expect("service_port must be configured")
                    .to_string(),
            )
            .current_dir(&python_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                TranscriptionError::subprocess(format!("Failed to start Python service: {}", e))
            })?;

        // Store process handle
        let mut process = self.python_process.write().await;
        *process = Some(child);

        // Wait for service to be ready
        self.wait_for_service().await?;

        Ok(())
    }

    /// Wait for the Python service to be ready
    async fn wait_for_service(&self) -> TranscriptionResult<()> {
        let mut attempts = 0;
        let max_attempts = 30;

        while attempts < max_attempts {
            match self
                .client
                .get(&format!("{}/health", self.service_url))
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        info!("Python WhisperX service is ready");
                        return Ok(());
                    }
                }
                Err(_) => {
                    // Service not ready yet
                }
            }

            attempts += 1;
            sleep(Duration::from_secs(2)).await;
        }

        Err(TranscriptionError::service_unavailable(
            "Python service failed to start",
        ))
    }

    /// Convert Python response to Rust response
    fn convert_response(
        &self,
        py_response: PythonResponse,
        request_id: Uuid,
    ) -> TranscriptionResponse {
        let segments: Vec<TranscriptionSegment> = py_response
            .segments
            .into_iter()
            .map(|seg| TranscriptionSegment {
                id: seg.id,
                start: seg.start,
                end: seg.end,
                text: seg.text,
                confidence: seg.confidence,
                speaker: seg.speaker,
                words: seg.words.map(|words| {
                    words
                        .into_iter()
                        .map(|w| WordSegment {
                            word: w.word,
                            start: w.start,
                            end: w.end,
                            confidence: w.confidence,
                            speaker: w.speaker,
                        })
                        .collect()
                }),
            })
            .collect();

        let speaker_segments: Vec<SpeakerSegment> = py_response
            .speaker_segments
            .into_iter()
            .map(|seg| SpeakerSegment {
                speaker: seg.speaker,
                start: seg.start,
                end: seg.end,
                confidence: seg.confidence,
            })
            .collect();

        let words: Vec<WordSegment> = py_response
            .words
            .into_iter()
            .map(|w| WordSegment {
                word: w.word,
                start: w.start,
                end: w.end,
                confidence: w.confidence,
                speaker: w.speaker,
            })
            .collect();

        TranscriptionResponse {
            request_id,
            call_id: py_response.call_id,
            status: match py_response.status.as_str() {
                "completed" => TranscriptionStatus::Completed,
                "failed" => TranscriptionStatus::Failed,
                _ => TranscriptionStatus::Processing,
            },
            text: py_response.text,
            language: py_response.language,
            confidence: py_response.confidence,
            processing_time_ms: py_response.processing_time_ms,
            segments,
            speaker_segments,
            speaker_count: py_response.speaker_count,
            words,
            error: py_response.error,
            completed_at: Utc::now(),
        }
    }
}

// Implementation moved to whisperx_impl.rs to keep the file organized
include!("whisperx_impl.rs");
