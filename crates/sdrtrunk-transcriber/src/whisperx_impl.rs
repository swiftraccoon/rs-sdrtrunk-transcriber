/// Acceptance response from the Python service when using webhook callback pattern
#[derive(Deserialize)]
struct AcceptResponse {
    /// Request ID assigned by the service
    request_id: String,
    /// Current status of the request
    status: String,
}

/// Status query response from the Python service
#[derive(Deserialize)]
struct StatusResponse {
    /// Current status string
    status: String,
}

#[async_trait]
impl TranscriptionService for WhisperXService {
    async fn initialize(&mut self, config: &TranscriptionConfig) -> TranscriptionResult<()> {
        self.config = config.clone();

        // Start Python service if local path configured, otherwise connect to external
        if self.config.python_path.is_some() {
            info!("Starting local WhisperX Python process");
            self.start_python_service().await?;
        } else {
            info!(
                "Connecting to external WhisperX service at {}",
                self.service_url
            );
            self.wait_for_service().await?;
        }

        let mut initialized = self.initialized.write().await;
        *initialized = true;
        drop(initialized);

        Ok(())
    }

    async fn shutdown(&mut self) -> TranscriptionResult<()> {
        info!("Shutting down WhisperX service");

        // Stop Python subprocess if we started it
        let mut process = self.python_process.write().await;
        if let Some(mut child) = process.take() {
            let _ = child.kill().await;
        }
        drop(process);

        let mut initialized = self.initialized.write().await;
        *initialized = false;
        drop(initialized);

        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    async fn transcribe(
        &self,
        request: &TranscriptionRequest,
    ) -> TranscriptionResult<TranscriptionResponse> {
        const MAX_RETRIES: u32 = 3;
        let initialized = self.initialized.read().await;
        if !*initialized {
            return Err(TranscriptionError::service_unavailable("WhisperX"));
        }
        drop(initialized);

        // Track request
        {
            let mut requests = self.active_requests.write().await;
            let _ = requests.insert(request.id, TranscriptionStatus::Processing);
        }

        // Build Python request with callback URL
        let callback_url = "http://localhost:8080/api/v1/transcription/callback".to_string();
        let py_request = PythonRequest {
            id: request.id,
            call_id: request.call_id,
            audio_path: request.audio_path.to_string_lossy().to_string(),
            requested_at: request.requested_at.to_rfc3339(),
            options: PythonOptions {
                language: request.options.language.clone(),
                diarize: request.options.diarize,
                min_speakers: request.options.min_speakers,
                max_speakers: request.options.max_speakers,
                vad: request.options.vad,
                word_timestamps: request.options.word_timestamps,
                return_confidence: request.options.return_confidence,
                max_duration: request.options.max_duration,
            },
            retry_count: request.retry_count,
            priority: request.priority,
            callback_url: Some(callback_url),
        };

        // Send request to Python service with retry logic
        let url = format!("{}/transcribe", self.service_url);

        // Retry with exponential backoff: 1s, 2s, 4s
        let mut retry_count: u32 = 0;

        let response = loop {
            info!(
                "Attempting to connect to WhisperX service at {url} (attempt {}/{})",
                retry_count + 1,
                MAX_RETRIES
            );

            match self.client.post(&url).json(&py_request).send().await {
                Ok(resp) => {
                    info!("Successfully connected to WhisperX, awaiting response");
                    break resp;
                }
                Err(e) => {
                    retry_count += 1;

                    if retry_count >= MAX_RETRIES {
                        error!(
                            "Failed to connect to WhisperX service after {MAX_RETRIES} attempts: {e}"
                        );
                        return Err(TranscriptionError::service_communication(format!(
                            "HTTP request failed after {MAX_RETRIES} retries: {e}"
                        )));
                    }

                    // Exponential backoff: 1s, 2s, 4s
                    let delay = Duration::from_secs(1 << (retry_count - 1));
                    info!("Connection failed (attempt {retry_count}), retrying in {delay:?}: {e}");
                    sleep(delay).await;
                }
            }
        };

        // Check if we got 202 Accepted (webhook callback pattern)
        if response.status() == 202 {
            info!("Request accepted with callback URL, returning immediately");

            let accept_resp: AcceptResponse = response.json().await.map_err(|e| {
                TranscriptionError::service_communication(format!(
                    "Failed to parse acceptance: {e}"
                ))
            })?;

            info!(
                "Transcription request {} accepted by WhisperX with status: {}",
                accept_resp.request_id, accept_resp.status
            );

            // Update tracking to show it's processing
            {
                let mut requests = self.active_requests.write().await;
                let _ = requests.insert(request.id, TranscriptionStatus::Processing);
            }

            // Return a processing response - the webhook will handle the actual result
            return Ok(TranscriptionResponse {
                request_id: request.id,
                call_id: request.call_id,
                status: TranscriptionStatus::Processing,
                text: None,
                language: None,
                confidence: None,
                processing_time_ms: 0,
                segments: vec![],
                speaker_segments: vec![],
                speaker_count: None,
                words: vec![],
                error: None,
                completed_at: Utc::now(),
            });
        }

        // Old synchronous pattern (backward compatibility)
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(TranscriptionError::processing_failed(format!(
                "Service returned {status}: {error_text}"
            )));
        }

        // Parse response
        let py_response: PythonResponse = response.json().await.map_err(|e| {
            TranscriptionError::service_communication(format!("Failed to parse response: {e}"))
        })?;

        // Convert to Rust response
        let transcription_response = Self::convert_response(py_response, request.id);

        // Update tracking
        {
            let mut requests = self.active_requests.write().await;
            let _ = requests.insert(request.id, transcription_response.status);
        }

        Ok(transcription_response)
    }

    async fn health_check(&self) -> TranscriptionResult<ServiceHealth> {
        let initialized = self.initialized.read().await;
        if !*initialized {
            return Ok(ServiceHealth::unhealthy("Service not initialized"));
        }
        drop(initialized);

        // Query Python service health
        match self
            .client
            .get(format!("{}/health", self.service_url))
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    #[derive(Deserialize)]
                    #[allow(dead_code)]
                    struct HealthResponse {
                        healthy: bool,
                        status: String,
                        model_loaded: bool,
                        gpu_available: Option<bool>,
                        queue_depth: usize,
                        active_workers: usize,
                    }

                    if let Ok(health) = response.json::<HealthResponse>().await {
                        let mut service_health = ServiceHealth::healthy(health.status);
                        service_health.model_loaded = health.model_loaded;
                        service_health.gpu_available = health.gpu_available;
                        service_health.queue_depth = health.queue_depth;
                        service_health.active_workers = health.active_workers;
                        return Ok(service_health);
                    }
                }
            }
            Err(e) => {
                return Ok(ServiceHealth::unhealthy(format!(
                    "Service unreachable: {e}"
                )));
            }
        }

        Ok(ServiceHealth::unhealthy("Health check failed"))
    }

    async fn get_stats(&self) -> TranscriptionResult<TranscriptionStats> {
        // Query Python service stats
        if let Ok(response) = self
            .client
            .get(format!("{}/stats", self.service_url))
            .send()
            .await
            && response.status().is_success()
            && let Ok(stats) = response.json::<TranscriptionStats>().await
        {
            return Ok(stats);
        }

        Ok(TranscriptionStats::default())
    }

    async fn get_status(&self, request_id: Uuid) -> TranscriptionResult<TranscriptionStatus> {
        // Check local tracking first
        {
            let requests = self.active_requests.read().await;
            if let Some(status) = requests.get(&request_id) {
                return Ok(*status);
            }
        }

        // Query Python service
        if let Ok(response) = self
            .client
            .get(format!("{}/status/{request_id}", self.service_url))
            .send()
            .await
            && response.status().is_success()
            && let Ok(status_resp) = response.json::<StatusResponse>().await
        {
            return Ok(match status_resp.status.as_str() {
                "processing" => TranscriptionStatus::Processing,
                "completed" => TranscriptionStatus::Completed,
                "failed" => TranscriptionStatus::Failed,
                "cancelled" => TranscriptionStatus::Cancelled,
                _ => TranscriptionStatus::Pending,
            });
        }

        Ok(TranscriptionStatus::Pending)
    }

    async fn cancel(&self, request_id: Uuid) -> TranscriptionResult<()> {
        // Send cancel request to Python service
        let response = self
            .client
            .delete(format!("{}/cancel/{request_id}", self.service_url))
            .send()
            .await
            .map_err(|e| {
                TranscriptionError::service_communication(format!("Cancel request failed: {e}"))
            })?;

        if !response.status().is_success() {
            return Err(TranscriptionError::processing_failed(
                "Failed to cancel request",
            ));
        }

        // Update local tracking
        {
            let mut requests = self.active_requests.write().await;
            let _ = requests.insert(request_id, TranscriptionStatus::Cancelled);
        }

        Ok(())
    }

    async fn validate_audio(&self, path: &Path) -> TranscriptionResult<AudioValidation> {
        if !path.exists() {
            return Ok(AudioValidation::invalid("File does not exist", 0));
        }

        let metadata = tokio::fs::metadata(path).await?;
        let file_size = metadata.len();

        // Check extension
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !self.capabilities().supports_format(&extension) {
            return Ok(AudioValidation::invalid(
                format!("Unsupported format: {extension}"),
                file_size,
            ));
        }

        // Query Python service for thorough validation
        let initialized = self.initialized.read().await;
        if !*initialized {
            // Service not initialized, fallback to basic validation
            return Ok(AudioValidation::valid(extension, 0.0, file_size));
        }
        drop(initialized);

        self.validate_audio_via_python(&extension, file_size, path)
            .await
    }

    fn capabilities(&self) -> ServiceCapabilities {
        ServiceCapabilities::whisperx()
    }

    fn name(&self) -> &'static str {
        "whisperx"
    }
}

impl WhisperXService {
    /// Perform audio validation by querying the Python service
    ///
    /// # Errors
    ///
    /// This method does not propagate errors directly; validation failures are
    /// returned as `AudioValidation` results with appropriate issue descriptions.
    async fn validate_audio_via_python(
        &self,
        extension: &str,
        file_size: u64,
        path: &Path,
    ) -> TranscriptionResult<AudioValidation> {
        let validation_request = PythonValidationRequest {
            audio_path: path.to_string_lossy().to_string(),
        };

        let url = format!("{}/validate", self.service_url);
        let fallback = || AudioValidation::valid(extension.to_string(), 0.0, file_size);

        let response = match self.client.post(&url).json(&validation_request).send().await {
            Ok(r) => r,
            Err(e) => {
                info!("Failed to connect to validation endpoint: {e}, using basic validation");
                return Ok(fallback());
            }
        };

        if !response.status().is_success() {
            info!(
                "Validation endpoint returned error {}, using basic validation",
                response.status()
            );
            return Ok(fallback());
        }

        match response.json::<PythonValidationResponse>().await {
            Ok(py_validation) => Ok(Self::build_validation_result(py_validation, extension)),
            Err(e) => {
                info!("Failed to parse validation response: {e}, using basic validation");
                Ok(fallback())
            }
        }
    }

    /// Build an `AudioValidation` result from the Python service response
    fn build_validation_result(
        py_validation: PythonValidationResponse,
        fallback_extension: &str,
    ) -> AudioValidation {
        let sample_rate = py_validation
            .sample_rate
            .and_then(|sr| u32::try_from(sr).ok());
        let channels = py_validation
            .channels
            .and_then(|ch| u16::try_from(ch).ok());

        if py_validation.valid {
            AudioValidation {
                valid: true,
                format: py_validation
                    .format
                    .or_else(|| Some(fallback_extension.to_string())),
                duration_seconds: py_validation.duration_seconds,
                sample_rate,
                channels,
                file_size: py_validation.file_size_bytes,
                issues: Vec::new(),
            }
        } else {
            let error_msg = py_validation
                .error_message
                .unwrap_or_else(|| "Validation failed".to_string());
            AudioValidation {
                valid: false,
                format: py_validation.format,
                duration_seconds: py_validation.duration_seconds,
                sample_rate,
                channels,
                file_size: py_validation.file_size_bytes,
                issues: vec![error_msg],
            }
        }
    }
}
