"""FastAPI service for WhisperX transcription."""

import asyncio
import logging
import time
from contextlib import asynccontextmanager
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, Optional
from uuid import UUID

import httpx
from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from pydantic import ValidationError
from pythonjsonlogger import jsonlogger

from config import config
from models import (
    ServiceHealth,
    ServiceStats,
    TranscriptionRequest,
    TranscriptionResponse,
    TranscriptionStatus,
)
from transcription import transcription_service

# Configure logging
logHandler = logging.StreamHandler()
if config.log_format == "json":
    formatter = jsonlogger.JsonFormatter()
    logHandler.setFormatter(formatter)

logging.basicConfig(
    level=getattr(logging, config.log_level),
    handlers=[logHandler],
)

logger = logging.getLogger(__name__)

# Request tracking
active_requests: Dict[UUID, TranscriptionStatus] = {}
service_stats = ServiceStats()
service_start_time = time.time()

# Job queue for async processing
job_queue: asyncio.Queue = asyncio.Queue(maxsize=1000)
job_results: Dict[UUID, Optional[TranscriptionResponse]] = {}
processing_task = None

# HTTP client for callbacks
callback_client = httpx.AsyncClient(timeout=30.0)


async def send_webhook_callback(callback_url: str, response: TranscriptionResponse):
    """Send transcription result to webhook callback URL."""
    try:
        logger.info(f"Sending webhook callback to {callback_url} for request {response.request_id}")

        # Convert response to dict for JSON serialization
        payload = response.model_dump(mode='json')

        # Send POST request with transcription result
        async with httpx.AsyncClient(timeout=10.0) as client:
            result = await client.post(callback_url, json=payload)

            if result.status_code == 200:
                logger.info(f"Webhook callback successful for request {response.request_id}")
            else:
                logger.warning(f"Webhook callback returned status {result.status_code} for request {response.request_id}")

    except Exception as e:
        logger.error(f"Failed to send webhook callback for request {response.request_id}: {e}")


async def process_queue_worker():
    """Background worker that processes transcription queue one at a time."""
    logger.info("Queue worker started")

    while True:
        try:
            # Get next request from queue
            request = await job_queue.get()

            # Track processing time from the beginning (before any operations that might fail)
            start_time = time.time()

            # Update status
            active_requests[request.id] = TranscriptionStatus.PROCESSING
            logger.info(f"Processing request {request.id}")

            try:
                # Run transcription (synchronously, one at a time)
                result = transcription_service.transcribe_sync(
                    str(request.audio_path),
                    request.options.model_dump() if request.options else {}
                )
                # Calculate actual processing time in milliseconds
                processing_time_ms = int((time.time() - start_time) * 1000)

                # Create response
                response = TranscriptionResponse(
                    request_id=request.id,
                    call_id=request.call_id,
                    status=TranscriptionStatus.COMPLETED,
                    text=result.get("text"),
                    language=result.get("language"),
                    confidence=result.get("confidence"),
                    processing_time_ms=processing_time_ms,  # Use our calculated time
                    segments=result.get("segments", []),
                    speaker_segments=result.get("speaker_segments", []),
                    speaker_count=result.get("speaker_count"),
                    words=result.get("words", []),
                    completed_at=datetime.now(timezone.utc),
                )

                # Store result
                job_results[request.id] = response
                active_requests[request.id] = TranscriptionStatus.COMPLETED
                service_stats.successful += 1

                logger.info(f"Request {request.id} completed in {processing_time_ms:.2f}ms")

                # Send webhook callback if URL provided
                if request.callback_url:
                    await send_webhook_callback(request.callback_url, response)

            except Exception as e:
                logger.error(f"Request {request.id} failed: {e}")
                active_requests[request.id] = TranscriptionStatus.FAILED
                job_results[request.id] = TranscriptionResponse(
                    request_id=request.id,
                    call_id=request.call_id,
                    status=TranscriptionStatus.FAILED,
                    error=str(e),
                    processing_time_ms=int((time.time() - start_time) * 1000),
                    completed_at=datetime.now(timezone.utc),
                )
                service_stats.failed += 1

                # Send failure webhook callback if URL provided
                if request.callback_url:
                    await send_webhook_callback(request.callback_url, job_results[request.id])

        except asyncio.CancelledError:
            logger.info("Queue worker cancelled")
            break
        except Exception as e:
            logger.error(f"Queue worker error: {e}")
            await asyncio.sleep(1)  # Brief pause before retrying

@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application lifespan handler."""
    global processing_task

    logger.info("Starting WhisperX service")

    # Initialize transcription service
    try:
        await transcription_service.initialize()
    except Exception as e:
        logger.error(f"Failed to initialize transcription service: {e}")
        raise

    # Create temp directory
    config.temp_dir.mkdir(parents=True, exist_ok=True)

    # Start background queue processor
    processing_task = asyncio.create_task(process_queue_worker())
    logger.info("Background queue processor started")

    yield

    # Cleanup
    logger.info("Shutting down WhisperX service")
    if processing_task:
        processing_task.cancel()
        await asyncio.gather(processing_task, return_exceptions=True)

    await transcription_service.shutdown()


# Create FastAPI app
app = FastAPI(
    title="WhisperX Transcription Service",
    version="1.0.0",
    lifespan=lifespan,
)

# Add CORS middleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.get("/health", response_model=ServiceHealth)
async def health_check():
    """Check service health."""
    try:
        # Check if models are loaded
        model_loaded = transcription_service._initialized

        # Check GPU availability
        gpu_available = None
        if transcription_service.device in ["cuda", "mps"]:
            gpu_available = True

        # Get memory info if available
        available_memory = None
        if transcription_service.device == "cuda":
            import torch
            if torch.cuda.is_available():
                available_memory = torch.cuda.get_device_properties(0).total_memory

        return ServiceHealth(
            healthy=model_loaded,
            status="Operational" if model_loaded else "Not initialized",
            model_loaded=model_loaded,
            available_memory=available_memory,
            gpu_available=gpu_available,
            queue_depth=len(active_requests),
            active_workers=sum(1 for s in active_requests.values() if s == TranscriptionStatus.PROCESSING),
            checked_at=datetime.now(timezone.utc),
        )
    except Exception as e:
        logger.error(f"Health check failed: {e}")
        return ServiceHealth(
            healthy=False,
            status=f"Error: {str(e)}",
            model_loaded=False,
            checked_at=datetime.now(timezone.utc),
        )


@app.get("/stats", response_model=ServiceStats)
async def get_stats():
    """Get service statistics."""
    service_stats.uptime_seconds = time.time() - service_start_time
    service_stats.queue_depth = len(active_requests)
    service_stats.processing = sum(
        1 for s in active_requests.values() if s == TranscriptionStatus.PROCESSING
    )
    return service_stats


@app.post("/transcribe", status_code=202)
async def transcribe(request: TranscriptionRequest):
    """Accept a transcription request and queue for processing."""
    # Check if file exists
    if not request.audio_path.exists():
        raise HTTPException(
            status_code=404,
            detail=f"Audio file not found: {request.audio_path}",
        )

    # Check queue size
    if job_queue.full():
        raise HTTPException(
            status_code=503,
            detail="Service queue is full",
        )

    # Add to queue
    try:
        await job_queue.put(request)
        active_requests[request.id] = TranscriptionStatus.PENDING
        service_stats.total_requests += 1

        logger.info(f"Request {request.id} accepted and queued (queue depth: {job_queue.qsize()})")

        # Return immediately with accepted status
        return {
            "status": "accepted",
            "request_id": str(request.id),
            "call_id": str(request.call_id),
            "queue_position": job_queue.qsize()
        }
    except Exception as e:
        logger.error(f"Failed to queue request: {e}")
        raise HTTPException(
            status_code=500,
            detail="Failed to queue request"
        )


@app.get("/status/{request_id}")
async def get_status(request_id: UUID):
    """Get status of a transcription request."""
    status = active_requests.get(request_id)
    if status is None:
        raise HTTPException(
            status_code=404,
            detail="Request not found",
        )

    return {"request_id": request_id, "status": status}


@app.get("/result/{request_id}", response_model=TranscriptionResponse)
async def get_result(request_id: UUID):
    """Get the result of a completed transcription."""
    # Check if request exists
    if request_id not in active_requests:
        raise HTTPException(
            status_code=404,
            detail="Request not found",
        )

    # Check if completed
    status = active_requests.get(request_id)
    if status == TranscriptionStatus.PENDING:
        raise HTTPException(
            status_code=202,
            detail="Request is still pending in queue"
        )
    elif status == TranscriptionStatus.PROCESSING:
        raise HTTPException(
            status_code=202,
            detail="Request is currently being processed"
        )

    # Get result
    result = job_results.get(request_id)
    if result is None:
        raise HTTPException(
            status_code=404,
            detail="Result not found (may have been cleaned up)"
        )

    return result


@app.delete("/cancel/{request_id}")
async def cancel_request(request_id: UUID):
    """Cancel a transcription request."""
    if request_id not in active_requests:
        raise HTTPException(
            status_code=404,
            detail="Request not found",
        )

    active_requests[request_id] = TranscriptionStatus.CANCELLED
    return {"request_id": request_id, "status": "cancelled"}


async def cleanup_old_requests():
    """Clean up completed/failed requests older than 1 hour."""
    current_time = time.time()
    to_remove = []

    for request_id, status in active_requests.items():
        if status in [TranscriptionStatus.COMPLETED, TranscriptionStatus.FAILED, TranscriptionStatus.CANCELLED]:
            # Remove after 1 hour
            to_remove.append(request_id)

    for request_id in to_remove[:10]:  # Remove max 10 at a time
        del active_requests[request_id]


@app.exception_handler(ValidationError)
async def validation_exception_handler(request, exc):
    """Handle validation errors."""
    return JSONResponse(
        status_code=422,
        content={"detail": str(exc)},
    )


@app.exception_handler(Exception)
async def general_exception_handler(request, exc):
    """Handle general exceptions."""
    logger.error(f"Unhandled exception: {exc}")
    return JSONResponse(
        status_code=500,
        content={"detail": "Internal server error"},
    )


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(
        app,
        host=config.host,
        port=config.port,
        workers=config.workers,
        log_level=config.log_level.lower(),
    )