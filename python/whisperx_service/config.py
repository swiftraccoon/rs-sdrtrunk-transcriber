"""Configuration for WhisperX service."""

import os
from pathlib import Path
from typing import Optional
import tomllib

from pydantic_settings import BaseSettings


class ServiceConfig(BaseSettings):
    """WhisperX service configuration."""

    # Server settings
    host: str = "0.0.0.0"
    port: int = 8001  # Default, override with WHISPERX_PORT env var
    workers: int = 1

    # Model settings
    model_size: str = "large-v3"
    device: str = "cpu"  # "cuda", "cpu", or "mps"
    compute_type: str = "float32"  # "float16", "int8", "float32"
    batch_size: int = 16

    # Transcription settings
    language: Optional[str] = None  # Auto-detect if None
    task: str = "transcribe"  # or "translate"

    # Diarization settings
    diarize: bool = True
    min_speakers: Optional[int] = None
    max_speakers: Optional[int] = None

    # VAD settings
    vad_onset: float = 0.500
    vad_offset: float = 0.363

    # Processing settings
    chunk_length: Optional[int] = 30  # seconds

    # Paths
    model_cache_dir: Path = Path.home() / ".cache" / "whisperx"
    temp_dir: Path = Path("/tmp/whisperx")

    # Logging
    log_level: str = "INFO"
    log_format: str = "json"

    # Performance
    num_workers: int = 2
    max_queue_size: int = 100
    request_timeout: int = 300  # seconds

    # Auth (optional)
    hf_token: Optional[str] = None  # Hugging Face token for pyannote models

    class Config:
        """Pydantic config."""
        env_prefix = "WHISPERX_"
        env_file = ".env"
        env_file_encoding = "utf-8"


# Load port from config.toml if it exists
def load_config_from_toml():
    config_path = Path(__file__).parent.parent.parent / "config.toml"
    if config_path.exists():
        with open(config_path, "rb") as f:
            toml_config = tomllib.load(f)
            if "transcription" in toml_config:
                transcription = toml_config["transcription"]
                overrides = {}
                if "service_port" in transcription:
                    overrides["port"] = transcription["service_port"]
                if "model_size" in transcription:
                    overrides["model_size"] = transcription["model_size"]
                if "device" in transcription:
                    overrides["device"] = transcription["device"]
                if "batch_size" in transcription:
                    overrides["batch_size"] = transcription["batch_size"]
                if "language" in transcription:
                    overrides["language"] = transcription["language"]
                return overrides
    return {}

# Global config instance - load from TOML first, then env vars can override
toml_overrides = load_config_from_toml()
config = ServiceConfig(**toml_overrides)