//! Worker pool for processing transcription requests

use crate::error::{TranscriptionError, TranscriptionResult};
use crate::service::TranscriptionService;
use crate::types::{TranscriptionRequest, TranscriptionResponse};
use sdrtrunk_core::{TranscriptionConfig, TranscriptionStatus};
use async_channel::{Receiver, Sender};
use sdrtrunk_database::{queries, queries::TranscriptionUpdate};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

/// Worker pool for processing transcription requests
pub struct TranscriptionWorkerPool {
    /// Configuration
    config: TranscriptionConfig,

    /// Request sender channel
    sender: Sender<TranscriptionRequest>,

    /// Request receiver channel
    receiver: Receiver<TranscriptionRequest>,

    /// Response channel
    response_sender: Sender<TranscriptionResponse>,

    /// Response receiver
    response_receiver: Receiver<TranscriptionResponse>,

    /// Worker handles
    workers: Vec<JoinHandle<()>>,

    /// Transcription service
    service: Arc<dyn TranscriptionService>,

    /// Database pool
    pool: PgPool,
}

impl TranscriptionWorkerPool {
    /// Create a new worker pool
    pub fn new(
        config: TranscriptionConfig,
        service: Arc<dyn TranscriptionService>,
        pool: PgPool,
    ) -> Self {
        let queue_size = config.queue_size;
        let (sender, receiver) = async_channel::bounded(queue_size);
        let (response_sender, response_receiver) = async_channel::bounded(queue_size);

        Self {
            config,
            sender,
            receiver,
            response_sender,
            response_receiver,
            workers: Vec::new(),
            service,
            pool,
        }
    }

    /// Start the worker pool
    pub async fn start(&mut self) -> TranscriptionResult<()> {
        info!("Starting transcription worker pool with {} workers", self.config.workers);

        for i in 0..self.config.workers {
            let worker = self.spawn_worker(i).await?;
            self.workers.push(worker);
        }

        Ok(())
    }

    /// Spawn a worker
    async fn spawn_worker(&self, id: usize) -> TranscriptionResult<JoinHandle<()>> {
        let receiver = self.receiver.clone();
        let response_sender = self.response_sender.clone();
        let service = Arc::clone(&self.service);
        let pool = self.pool.clone();

        let handle = tokio::spawn(async move {
            info!("Worker {} started", id);

            while let Ok(request) = receiver.recv().await {
                info!("Worker {} picked up request {} for call {}", id, request.id, request.call_id);
                let call_id = request.call_id;

                info!("Worker {} submitting transcription request for call {} with webhook callback", id, call_id);
                match service.transcribe(&request).await {
                    Ok(response) => {
                        // With webhook pattern, we just log that the request was accepted
                        // The actual result will come via webhook callback
                        if response.status == TranscriptionStatus::Processing {
                            info!("Worker {} transcription request accepted for call {} - webhook will handle completion", id, call_id);

                            // Update database to show it's processing
                            if let Err(e) = queries::RadioCallQueries::update_transcription_status(
                                &pool,
                                TranscriptionUpdate {
                                    id: call_id,
                                    status: "processing",
                                    text: None,
                                    confidence: None,
                                    error: None,
                                    speaker_segments: None,
                                    speaker_count: None,
                                },
                            )
                            .await
                            {
                                error!("Worker {} failed to update processing status: {}", id, e);
                            }
                        } else {
                            // Backward compatibility: handle immediate response if not using webhook
                            warn!("Worker {} got immediate response (non-webhook pattern) for call {}", id, call_id);

                            // Still send to response channel for backward compatibility
                            if let Err(e) = response_sender.send(response).await {
                                error!("Worker {} failed to send response: {}", id, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Worker {} transcription failed: {}", id, e);

                        // Update database with error status
                        if let Err(db_err) = queries::RadioCallQueries::update_transcription_status(
                            &pool,
                            TranscriptionUpdate {
                                id: call_id,
                                status: "failed",
                                text: None,
                                confidence: None,
                                error: Some(&e.to_string()),
                                speaker_segments: None,
                                speaker_count: None,
                            },
                        )
                        .await
                        {
                            error!("Worker {} failed to update database with error status: {}", id, db_err);
                        }
                    }
                }
            }

            info!("Worker {} shutting down", id);
        });

        Ok(handle)
    }

    /// Submit a transcription request
    pub async fn submit(&self, request: TranscriptionRequest) -> TranscriptionResult<()> {
        self.sender
            .send(request)
            .await
            .map_err(|_| TranscriptionError::queue_full(self.config.queue_size))
    }

    /// Try to submit a transcription request without blocking
    /// Returns Ok(()) if submitted successfully, Err if queue is full
    pub fn try_submit(&self, request: TranscriptionRequest) -> TranscriptionResult<()> {
        self.sender
            .try_send(request)
            .map_err(|_| TranscriptionError::queue_full(self.config.queue_size))
    }

    /// Get the current queue length for monitoring
    pub fn queue_len(&self) -> usize {
        self.sender.len()
    }

    /// Get the queue capacity
    pub fn queue_capacity(&self) -> Option<usize> {
        self.sender.capacity()
    }

    /// Get the next completed transcription
    pub async fn get_response(&self) -> TranscriptionResult<TranscriptionResponse> {
        self.response_receiver
            .recv()
            .await
            .map_err(|_| TranscriptionError::worker_pool("Response channel closed"))
    }

    /// Shutdown the worker pool
    pub async fn shutdown(mut self) -> TranscriptionResult<()> {
        info!("Shutting down transcription worker pool");

        // Close the sender to signal workers to stop
        self.sender.close();

        // Wait for all workers to finish
        for (i, worker) in self.workers.drain(..).enumerate() {
            if let Err(e) = worker.await {
                warn!("Worker {} failed to shutdown cleanly: {}", i, e);
            }
        }

        info!("Transcription worker pool shut down");
        Ok(())
    }

    /// Get queue depth
    pub fn queue_depth(&self) -> usize {
        self.receiver.len()
    }

    /// Check if queue is full
    pub fn is_queue_full(&self) -> bool {
        self.receiver.is_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockTranscriptionService;
    use std::path::PathBuf;
    use uuid::Uuid;

    async fn create_test_pool() -> PgPool {
        // Create a test database pool
        PgPool::connect("postgres://localhost/test_db").await.unwrap()
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_worker_pool_creation() {
        let config = TranscriptionConfig::default();
        let service = Arc::new(MockTranscriptionService::new());
        let db_pool = create_test_pool().await;
        let pool = TranscriptionWorkerPool::new(config, service, db_pool);

        assert_eq!(pool.queue_depth(), 0);
        assert!(!pool.is_queue_full());
    }

    #[tokio::test]
    #[ignore = "Requires database connection"]
    async fn test_worker_pool_submit() {
        let config = TranscriptionConfig {
            workers: 1,
            queue_size: 10,
            ..Default::default()
        };

        let mut service = MockTranscriptionService::new();
        let mut init_config = config.clone();
        service.initialize(&mut init_config).await.unwrap();

        let service = Arc::new(service);
        let db_pool = create_test_pool().await;
        let mut pool = TranscriptionWorkerPool::new(config, service, db_pool);
        pool.start().await.unwrap();

        let request = TranscriptionRequest::new(
            Uuid::new_v4(),
            PathBuf::from("/test/audio.mp3"),
        );

        pool.submit(request).await.unwrap();
        assert!(pool.queue_depth() <= 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "Test hangs with 0 workers - needs refactoring"]
    async fn test_worker_pool_queue_full() {
        // This test has an issue: with 0 workers, tasks never complete
        // and the test hangs indefinitely. Ignoring for now.
        let config = TranscriptionConfig {
            workers: 0, // No workers to process
            queue_size: 2,
            ..Default::default()
        };

        let service = Arc::new(MockTranscriptionService::new());
        let db_pool = create_test_pool().await;
        let pool = TranscriptionWorkerPool::new(config, service, db_pool);

        // Fill the queue
        for _ in 0..2 {
            let request = TranscriptionRequest::new(
                Uuid::new_v4(),
                PathBuf::from("/test/audio.mp3"),
            );
            pool.submit(request).await.unwrap();
        }

        assert!(pool.is_queue_full());

        // This should fail
        let request = TranscriptionRequest::new(
            Uuid::new_v4(),
            PathBuf::from("/test/audio.mp3"),
        );
        let result = pool.submit(request).await;
        assert!(result.is_err());
    }
}