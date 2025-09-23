"""Data models for WhisperX service."""

from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import List, Optional
from uuid import UUID

from pydantic import BaseModel, Field


class TranscriptionStatus(str, Enum):
    """Transcription status enum."""
    PENDING = "pending"
    PROCESSING = "processing"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


class TranscriptionOptions(BaseModel):
    """Options for transcription processing."""
    language: Optional[str] = None
    diarize: bool = True
    min_speakers: Optional[int] = None
    max_speakers: Optional[int] = None
    vad: bool = True
    word_timestamps: bool = True
    return_confidence: bool = True
    max_duration: Optional[float] = 3600.0


class TranscriptionRequest(BaseModel):
    """Transcription request model."""
    id: UUID
    call_id: UUID
    audio_path: Path
    requested_at: datetime
    options: TranscriptionOptions = Field(default_factory=TranscriptionOptions)
    retry_count: int = 0
    priority: int = 0


class WordSegment(BaseModel):
    """Word-level segment with timing."""
    word: str
    start: float
    end: float
    confidence: Optional[float] = None
    speaker: Optional[str] = None


class TranscriptionSegment(BaseModel):
    """Transcription segment with timing."""
    id: int
    start: float
    end: float
    text: str
    confidence: Optional[float] = None
    speaker: Optional[str] = None
    words: Optional[List[WordSegment]] = None


class SpeakerSegment(BaseModel):
    """Speaker diarization segment."""
    speaker: str
    start: float
    end: float
    confidence: Optional[float] = None


class TranscriptionResponse(BaseModel):
    """Transcription response model."""
    request_id: UUID
    call_id: UUID
    status: TranscriptionStatus
    text: Optional[str] = None
    language: Optional[str] = None
    confidence: Optional[float] = None
    processing_time_ms: int
    segments: List[TranscriptionSegment] = Field(default_factory=list)
    speaker_segments: List[SpeakerSegment] = Field(default_factory=list)
    speaker_count: Optional[int] = None
    words: List[WordSegment] = Field(default_factory=list)
    error: Optional[str] = None
    completed_at: datetime


class ServiceHealth(BaseModel):
    """Service health status."""
    healthy: bool
    status: str
    model_loaded: bool
    available_memory: Optional[int] = None
    gpu_available: Optional[bool] = None
    queue_depth: int = 0
    active_workers: int = 0
    checked_at: datetime


class ServiceStats(BaseModel):
    """Service statistics."""
    total_requests: int = 0
    successful: int = 0
    failed: int = 0
    processing: int = 0
    queue_depth: int = 0
    avg_processing_time_ms: float = 0.0
    total_audio_duration: float = 0.0
    uptime_seconds: float = 0.0