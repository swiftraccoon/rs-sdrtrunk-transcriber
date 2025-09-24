#[async_trait]
impl TranscriptionService for WhisperXService {
    async fn initialize(&mut self, config: &TranscriptionConfig) -> TranscriptionResult<()> {
        self.config = config.clone();

        // Start Python service if configured
        if self.config.python_path.is_some() {
            self.start_python_service().await?;
        } else {
            // Assume service is already running externally
            self.wait_for_service().await?;
        }

        let mut initialized = self.initialized.write().await;
        *initialized = true;

        Ok(())
    }

    async fn shutdown(&mut self) -> TranscriptionResult<()> {
        info!("Shutting down WhisperX service");

        // Stop Python subprocess if we started it
        let mut process = self.python_process.write().await;
        if let Some(mut child) = process.take() {
            child.kill().await.ok();
        }

        let mut initialized = self.initialized.write().await;
        *initialized = false;

        Ok(())
    }

    async fn transcribe(
        &self,
        request: &TranscriptionRequest,
    ) -> TranscriptionResult<TranscriptionResponse> {
        let initialized = self.initialized.read().await;
        if !*initialized {
            return Err(TranscriptionError::service_unavailable("WhisperX"));
        }

        // Track request
        {
            let mut requests = self.active_requests.write().await;
            requests.insert(request.id, TranscriptionStatus::Processing);
        }

        // Build Python request with callback URL
        let callback_url = format!("http://localhost:8080/api/v1/transcription/callback");
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
        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 3;

        let response = loop {
            info!("Attempting to connect to WhisperX service at {} (attempt {}/{})",
                url, retry_count + 1, MAX_RETRIES);

            match self.client.post(&url).json(&py_request).send().await {
                Ok(resp) => {
                    info!("Successfully connected to WhisperX, awaiting response");
                    break resp;
                }
                Err(e) => {
                    retry_count += 1;

                    if retry_count >= MAX_RETRIES {
                        error!("Failed to connect to WhisperX service after {} attempts: {}",
                            MAX_RETRIES, e);
                        return Err(TranscriptionError::service_communication(
                            format!("HTTP request failed after {} retries: {}", MAX_RETRIES, e)
                        ));
                    }

                    // Exponential backoff: 1s, 2s, 4s
                    let delay = Duration::from_secs(1 << (retry_count - 1));
                    info!("Connection failed (attempt {}), retrying in {:?}: {}",
                        retry_count, delay, e);
                    sleep(delay).await;
                }
            }
        };

        // Check if we got 202 Accepted (webhook callback pattern)
        if response.status() == 202 {
            info!("Request accepted with callback URL, returning immediately");

            // Parse the acceptance response
            #[derive(Deserialize)]
            struct AcceptResponse {
                request_id: String,
                status: String,
            }

            let accept_resp: AcceptResponse = response
                .json()
                .await
                .map_err(|e| TranscriptionError::service_communication(format!("Failed to parse acceptance: {}", e)))?;

            info!("Transcription request {} accepted by WhisperX with status: {}",
                accept_resp.request_id, accept_resp.status);

            // Update tracking to show it's processing
            {
                let mut requests = self.active_requests.write().await;
                requests.insert(request.id, TranscriptionStatus::Processing);
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
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(TranscriptionError::processing_failed(format!(
                "Service returned {}: {}",
                status, error_text
            )));
        }

        // Parse response
        let py_response: PythonResponse = response
            .json()
            .await
            .map_err(|e| TranscriptionError::service_communication(format!("Failed to parse response: {}", e)))?;

        // Convert to Rust response
        let transcription_response = self.convert_response(py_response, request.id);

        // Update tracking
        {
            let mut requests = self.active_requests.write().await;
            requests.insert(request.id, transcription_response.status);
        }

        Ok(transcription_response)
    }

    async fn health_check(&self) -> TranscriptionResult<ServiceHealth> {
        let initialized = self.initialized.read().await;
        if !*initialized {
            return Ok(ServiceHealth::unhealthy("Service not initialized"));
        }

        // Query Python service health
        match self.client.get(&format!("{}/health", self.service_url)).send().await {
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
                return Ok(ServiceHealth::unhealthy(format!("Service unreachable: {}", e)));
            }
        }

        Ok(ServiceHealth::unhealthy("Health check failed"))
    }

    async fn get_stats(&self) -> TranscriptionResult<TranscriptionStats> {
        // Query Python service stats
        match self.client.get(&format!("{}/stats", self.service_url)).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    if let Ok(stats) = response.json::<TranscriptionStats>().await {
                        return Ok(stats);
                    }
                }
            }
            Err(_) => {}
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
        match self
            .client
            .get(&format!("{}/status/{}", self.service_url, request_id))
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    #[derive(Deserialize)]
                    struct StatusResponse {
                        status: String,
                    }

                    if let Ok(status_resp) = response.json::<StatusResponse>().await {
                        return Ok(match status_resp.status.as_str() {
                            "pending" => TranscriptionStatus::Pending,
                            "processing" => TranscriptionStatus::Processing,
                            "completed" => TranscriptionStatus::Completed,
                            "failed" => TranscriptionStatus::Failed,
                            "cancelled" => TranscriptionStatus::Cancelled,
                            _ => TranscriptionStatus::Pending,
                        });
                    }
                }
            }
            Err(_) => {}
        }

        Ok(TranscriptionStatus::Pending)
    }

    async fn cancel(&self, request_id: Uuid) -> TranscriptionResult<()> {
        // Send cancel request to Python service
        let response = self
            .client
            .delete(&format!("{}/cancel/{}", self.service_url, request_id))
            .send()
            .await
            .map_err(|e| TranscriptionError::service_communication(format!("Cancel request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(TranscriptionError::processing_failed("Failed to cancel request"));
        }

        // Update local tracking
        {
            let mut requests = self.active_requests.write().await;
            requests.insert(request_id, TranscriptionStatus::Cancelled);
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

        // TODO: Query Python service for more thorough validation
        Ok(AudioValidation::valid(extension, 0.0, file_size))
    }

    fn capabilities(&self) -> ServiceCapabilities {
        ServiceCapabilities::whisperx()
    }

    fn name(&self) -> &str {
        "whisperx"
    }
}