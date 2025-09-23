"""FastAPI service for WhisperX transcription."""

import asyncio
import logging
import time
from contextlib import asynccontextmanager
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict
from uuid import UUID

from fastapi import FastAPI, HTTPException, BackgroundTasks
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


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application lifespan handler."""
    logger.info("Starting WhisperX service")

    # Initialize transcription service
    try:
        await transcription_service.initialize()
    except Exception as e:
        logger.error(f"Failed to initialize transcription service: {e}")
        raise

    # Create temp directory
    config.temp_dir.mkdir(parents=True, exist_ok=True)

    yield

    # Cleanup
    logger.info("Shutting down WhisperX service")
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


@app.post("/transcribe", response_model=TranscriptionResponse)
async def transcribe(request: TranscriptionRequest, background_tasks: BackgroundTasks):
    """Process a transcription request."""
    # Check if file exists
    if not request.audio_path.exists():
        raise HTTPException(
            status_code=404,
            detail=f"Audio file not found: {request.audio_path}",
        )

    # Check queue size
    if len(active_requests) >= config.max_queue_size:
        raise HTTPException(
            status_code=503,
            detail="Service queue is full",
        )

    # Track request
    active_requests[request.id] = TranscriptionStatus.PROCESSING
    service_stats.total_requests += 1

    try:
        # Run transcription
        start_time = time.time()

        result = await asyncio.wait_for(
            transcription_service.transcribe(
                request.audio_path,
                request.options.model_dump() if request.options else {},
            ),
            timeout=config.request_timeout,
        )

        # Update stats
        service_stats.successful += 1
        processing_time = (time.time() - start_time) * 1000

        # Update average processing time
        if service_stats.successful == 1:
            service_stats.avg_processing_time_ms = processing_time
        else:
            service_stats.avg_processing_time_ms = (
                service_stats.avg_processing_time_ms * (service_stats.successful - 1)
                + processing_time
            ) / service_stats.successful

        # Log the transcription text
        transcription_text = result.get("text", "")
        if transcription_text:
            logger.info(f"Transcription: {transcription_text[:200]}{'...' if len(transcription_text) > 200 else ''}")
        else:
            logger.info("Transcription: (empty/no speech detected)")

        # Create response
        response = TranscriptionResponse(
            request_id=request.id,
            call_id=request.call_id,
            status=TranscriptionStatus.COMPLETED,
            text=result["text"],
            language=result["language"],
            confidence=result["confidence"],
            processing_time_ms=result["processing_time_ms"],
            segments=result["segments"],
            speaker_segments=result["speaker_segments"],
            speaker_count=result["speaker_count"],
            words=result["words"],
            completed_at=datetime.now(timezone.utc),
        )

        active_requests[request.id] = TranscriptionStatus.COMPLETED
        return response

    except asyncio.TimeoutError:
        service_stats.failed += 1
        active_requests[request.id] = TranscriptionStatus.FAILED

        raise HTTPException(
            status_code=504,
            detail=f"Transcription timeout after {config.request_timeout} seconds",
        )

    except Exception as e:
        logger.error(f"Transcription failed: {e}")
        service_stats.failed += 1
        active_requests[request.id] = TranscriptionStatus.FAILED

        return TranscriptionResponse(
            request_id=request.id,
            call_id=request.call_id,
            status=TranscriptionStatus.FAILED,
            error=str(e),
            processing_time_ms=int((time.time() - start_time) * 1000),
            completed_at=datetime.now(timezone.utc),
        )

    finally:
        # Clean up old requests
        background_tasks.add_task(cleanup_old_requests)


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