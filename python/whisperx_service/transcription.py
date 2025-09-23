"""WhisperX transcription service implementation."""

import asyncio
import gc
import logging
from pathlib import Path
from typing import Dict, List, Optional, Tuple

import torch
import whisperx
from tenacity import retry, stop_after_attempt, wait_exponential

from config import config

logger = logging.getLogger(__name__)


class TranscriptionService:
    """WhisperX-based transcription service."""

    def __init__(self):
        """Initialize transcription service."""
        self.model = None
        self.align_model = None
        self.align_metadata = None
        self.diarize_model = None
        self.device = None
        self.compute_type = None
        self._initialized = False

    @retry(
        stop=stop_after_attempt(3),
        wait=wait_exponential(multiplier=1, min=2, max=10)
    )
    async def initialize(self):
        """Initialize WhisperX models."""
        try:
            # Setup device
            if config.device == "cuda" and torch.cuda.is_available():
                self.device = "cuda"
                logger.info("Using CUDA device for transcription")
            elif config.device == "mps" and torch.backends.mps.is_available():
                self.device = "mps"
                logger.info("Using MPS device for transcription")
            else:
                self.device = "cpu"
                logger.info("Using CPU device for transcription")

            # Setup compute type
            if self.device == "cpu":
                self.compute_type = "int8"
            else:
                self.compute_type = config.compute_type

            logger.info(f"Loading WhisperX model {config.model_size} on {self.device}")

            # Load transcription model
            self.model = whisperx.load_model(
                config.model_size,
                self.device,
                compute_type=self.compute_type,
                download_root=str(config.model_cache_dir),
                language=config.language if config.language else None,
            )

            # Load alignment model if language is specified
            if config.language:
                logger.info(f"Loading alignment model for {config.language}")
                self.align_model, self.align_metadata = whisperx.load_align_model(
                    language_code=config.language,
                    device=self.device,
                )

            # Load diarization pipeline if enabled
            if config.diarize and config.hf_token:
                logger.info("Loading speaker diarization pipeline")
                try:
                    # Import from the correct module
                    from whisperx.diarize import DiarizationPipeline

                    self.diarize_model = DiarizationPipeline(
                        use_auth_token=config.hf_token,
                        device=self.device,
                    )
                    logger.info("Diarization pipeline loaded successfully")
                except ImportError as e:
                    logger.warning(f"Could not import DiarizationPipeline: {e}")
                    logger.warning("Diarization will be disabled")
                    self.diarize_model = None
                except Exception as e:
                    logger.warning(f"Could not initialize diarization: {e}")
                    logger.warning("Diarization will be disabled")
                    self.diarize_model = None
            elif config.diarize and not config.hf_token:
                logger.warning("Diarization requested but no HF token provided")

            self._initialized = True
            logger.info("WhisperX models loaded successfully")

        except Exception as e:
            logger.error(f"Failed to initialize models: {e}")
            raise

    async def transcribe(
        self,
        audio_path: str,
        options: Optional[Dict] = None
    ) -> Dict:
        """Transcribe audio file using WhisperX.

        Args:
            audio_path: Path to audio file
            options: Transcription options

        Returns:
            Transcription result with segments and metadata
        """
        if not self._initialized:
            await self.initialize()

        options = options or {}

        try:
            # Load audio
            audio = whisperx.load_audio(audio_path)

            # Transcribe with batching
            logger.info(f"Transcribing {audio_path}")
            result = self.model.transcribe(
                audio,
                batch_size=config.batch_size,
                language=options.get("language", config.language),
                print_progress=False,
            )

            # Align output for better timestamps if we have the language
            if result.get("language") and (self.align_model or not self.align_metadata):
                logger.info("Performing alignment for accurate timestamps")

                # Load alignment model for detected language if not already loaded
                if not self.align_model or (
                    self.align_metadata and
                    self.align_metadata.get("language", "") != result["language"]
                ):
                    # Clean up old model
                    if self.align_model:
                        del self.align_model
                        gc.collect()
                        if self.device == "cuda":
                            torch.cuda.empty_cache()

                    # Load new model
                    self.align_model, self.align_metadata = whisperx.load_align_model(
                        language_code=result["language"],
                        device=self.device,
                    )

                # Perform alignment
                result = whisperx.align(
                    result["segments"],
                    self.align_model,
                    self.align_metadata,
                    audio,
                    self.device,
                    return_char_alignments=options.get("return_char_alignments", False),
                )

            # Perform diarization if enabled
            if options.get("diarize", config.diarize) and self.diarize_model:
                logger.info("Performing speaker diarization")

                diarize_segments = self.diarize_model(
                    audio,
                    min_speakers=options.get("min_speakers"),
                    max_speakers=options.get("max_speakers"),
                )

                # Assign speakers to words
                result = whisperx.assign_word_speakers(diarize_segments, result)
                result["speaker_segments"] = diarize_segments

            # Ensure text field exists (combine all segments if needed)
            if "text" not in result and "segments" in result:
                result["text"] = " ".join(seg.get("text", "") for seg in result["segments"])
            elif "text" not in result:
                result["text"] = ""

            # Ensure all segments have IDs
            if "segments" in result:
                for i, seg in enumerate(result["segments"]):
                    if "id" not in seg:
                        seg["id"] = i

            # Add metadata
            result["audio_path"] = audio_path
            result["model_size"] = config.model_size
            result["device"] = self.device

            # Ensure required fields exist
            result.setdefault("language", None)
            result.setdefault("confidence", None)
            result.setdefault("segments", [])
            result.setdefault("speaker_segments", [])
            result.setdefault("speaker_count", None)
            result.setdefault("words", [])
            result.setdefault("processing_time_ms", 0)

            return result

        except Exception as e:
            logger.error(f"Transcription failed: {e}")
            raise

    def cleanup(self):
        """Clean up models and free memory."""
        logger.info("Cleaning up transcription models")

        if self.model:
            del self.model
        if self.align_model:
            del self.align_model
        if self.diarize_model:
            del self.diarize_model

        gc.collect()
        if self.device == "cuda":
            torch.cuda.empty_cache()

        self._initialized = False


# Global service instance
transcription_service = TranscriptionService()