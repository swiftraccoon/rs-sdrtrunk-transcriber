"""Configuration for WhisperX service."""

import os
from pathlib import Path
from typing import Optional, Union
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

    # Core decoding parameters
    beam_size: int = 5
    best_of: int = 5
    patience: float = 1.0
    length_penalty: float = 1.0
    repetition_penalty: float = 1.0
    no_repeat_ngram_size: int = 0

    # Temperature and fallback
    temperature: Union[float, list[float]] = 0.0
    temperature_increment_on_fallback: float = 0.2
    prompt_reset_on_temperature: float = 0.5

    # Threshold parameters
    compression_ratio_threshold: float = 2.4
    logprob_threshold: float = -1.0
    no_speech_threshold: float = 0.6
    hallucination_silence_threshold: Optional[float] = None

    # Prompt and context
    initial_prompt: Optional[str] = None
    prefix: Optional[str] = None
    condition_on_previous_text: bool = False
    hotwords: Optional[str] = None

    # Token control
    suppress_tokens: str = "-1"
    suppress_blank: bool = True
    suppress_numerals: bool = False
    without_timestamps: bool = True
    max_initial_timestamp: float = 0.0

    # Diarization settings
    diarize: bool = True
    min_speakers: Optional[int] = None
    max_speakers: Optional[int] = None
    word_timestamps: bool = True

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

    # Punctuation handling (faster-whisper compatible)
    prepend_punctuations: Optional[str] = None
    append_punctuations: Optional[str] = None

    # Additional VAD parameters (faster-whisper compatible - may not be used by WhisperX)
    min_speech_duration_ms: Optional[int] = None
    max_speech_duration_s: Optional[float] = None
    min_silence_duration_ms: Optional[int] = None
    speech_pad_ms: Optional[int] = None

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

                # Load all new WhisperX parameters
                param_mapping = {
                    "beam_size": "beam_size",
                    "best_of": "best_of",
                    "patience": "patience",
                    "length_penalty": "length_penalty",
                    "repetition_penalty": "repetition_penalty",
                    "no_repeat_ngram_size": "no_repeat_ngram_size",
                    "temperature": "temperature",
                    "temperature_increment_on_fallback": "temperature_increment_on_fallback",
                    "prompt_reset_on_temperature": "prompt_reset_on_temperature",
                    "compression_ratio_threshold": "compression_ratio_threshold",
                    "logprob_threshold": "logprob_threshold",
                    "no_speech_threshold": "no_speech_threshold",
                    "hallucination_silence_threshold": "hallucination_silence_threshold",
                    "initial_prompt": "initial_prompt",
                    "prefix": "prefix",
                    "condition_on_previous_text": "condition_on_previous_text",
                    "hotwords": "hotwords",
                    "suppress_tokens": "suppress_tokens",
                    "suppress_blank": "suppress_blank",
                    "suppress_numerals": "suppress_numerals",
                    "without_timestamps": "without_timestamps",
                    "max_initial_timestamp": "max_initial_timestamp",
                    "vad_onset": "vad_onset",
                    "vad_offset": "vad_offset",
                    "chunk_length": "chunk_length",
                    "diarization": "diarize",
                    "min_speakers": "min_speakers",
                    "max_speakers": "max_speakers",
                    "word_timestamps": "word_timestamps",
                    "hf_token": "hf_token",
                    "prepend_punctuations": "prepend_punctuations",
                    "append_punctuations": "append_punctuations",
                    "min_speech_duration_ms": "min_speech_duration_ms",
                    "max_speech_duration_s": "max_speech_duration_s",
                    "min_silence_duration_ms": "min_silence_duration_ms",
                    "speech_pad_ms": "speech_pad_ms"
                }

                for toml_key, config_key in param_mapping.items():
                    if toml_key in transcription:
                        value = transcription[toml_key]
                        # Handle empty strings as None for optional fields
                        if value == "" and config_key in ["initial_prompt", "prefix", "hotwords", "hallucination_silence_threshold"]:
                            value = None
                        # Handle 0.0 as None for hallucination_silence_threshold
                        elif config_key == "hallucination_silence_threshold" and value == 0.0:
                            value = None
                        overrides[config_key] = value

                return overrides
    return {}

# Global config instance - load from TOML first, then env vars can override
toml_overrides = load_config_from_toml()
config = ServiceConfig(**toml_overrides)