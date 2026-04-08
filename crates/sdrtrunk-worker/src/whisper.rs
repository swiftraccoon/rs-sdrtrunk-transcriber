//! Whisper Large v3 transcription via whisper.cpp (CPU-optimized).
//!
//! Loads the model once at startup and transcribes audio files on demand.
//! Audio is converted from MP3 to 16kHz mono WAV via ffmpeg before inference.

use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Whisper transcription engine. Loads model once, transcribes many files.
#[allow(clippy::redundant_pub_crate)]
pub(crate) struct WhisperEngine {
    ctx: WhisperContext,
    beam_size: i32,
}

/// Result of a transcription.
#[derive(Debug)]
#[allow(clippy::redundant_pub_crate)]
pub(crate) struct TranscriptionResult {
    /// Full transcribed text
    pub(crate) text: String,
    /// Per-segment results with timestamps
    #[allow(dead_code)]
    pub(crate) segments: Vec<Segment>,
}

/// A single transcription segment with timestamps.
#[derive(Debug)]
#[allow(dead_code, clippy::redundant_pub_crate)]
pub(crate) struct Segment {
    /// Start time in milliseconds
    pub(crate) start_ms: i64,
    /// End time in milliseconds
    pub(crate) end_ms: i64,
    /// Transcribed text for this segment
    pub(crate) text: String,
}

impl WhisperEngine {
    /// Load a Whisper model from a GGML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the model file cannot be loaded.
    #[allow(clippy::redundant_pub_crate)]
    pub(crate) fn load(model_path: &Path) -> Result<Self> {
        info!("Loading Whisper model from {}", model_path.display());
        let ctx = WhisperContext::new_with_params(
            model_path
                .to_str()
                .ok_or_else(|| anyhow!("Invalid model path"))?,
            WhisperContextParameters::default(),
        )
        .map_err(|e| anyhow!("Failed to load Whisper model: {e}"))?;
        info!("Whisper model loaded successfully");

        Ok(Self { ctx, beam_size: 5 })
    }

    /// Transcribe an audio file (MP3, WAV, or any ffmpeg-supported format).
    ///
    /// Converts to 16kHz mono WAV internally if needed, then runs Whisper inference.
    ///
    /// # Errors
    ///
    /// Returns an error if audio conversion or transcription fails.
    #[allow(clippy::redundant_pub_crate)]
    pub(crate) fn transcribe(&self, audio_path: &Path) -> Result<TranscriptionResult> {
        // Convert to 16kHz mono WAV
        let wav_path = convert_to_wav(audio_path)?;

        // Read WAV samples
        let samples = read_wav_samples(&wav_path)?;

        // Clean up temp WAV
        let _ = std::fs::remove_file(&wav_path);

        if samples.is_empty() {
            return Ok(TranscriptionResult {
                text: String::new(),
                segments: vec![],
            });
        }

        // Run Whisper inference
        let mut params = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size: self.beam_size,
            patience: -1.0,
        });
        params.set_language(Some("en"));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);

        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| anyhow!("Failed to create Whisper state: {e}"))?;

        state
            .full(params, &samples)
            .map_err(|e| anyhow!("Whisper inference failed: {e}"))?;

        // Extract results
        let num_segments = state.full_n_segments();
        #[allow(clippy::cast_sign_loss)]
        let mut segments = Vec::with_capacity(num_segments as usize);
        let mut full_text = String::new();

        for i in 0..num_segments {
            let Some(seg) = state.get_segment(i) else {
                continue;
            };
            let text = seg.to_str_lossy().unwrap_or_default().into_owned();
            let start = seg.start_timestamp();
            let end = seg.end_timestamp();

            if !text.trim().is_empty() {
                if !full_text.is_empty() {
                    full_text.push(' ');
                }
                full_text.push_str(text.trim());

                segments.push(Segment {
                    start_ms: start * 10, // whisper.cpp uses centiseconds
                    end_ms: end * 10,
                    text: text.trim().to_string(),
                });
            }
        }

        Ok(TranscriptionResult {
            text: full_text,
            segments,
        })
    }
}

/// Convert any audio file to 16kHz mono WAV using ffmpeg.
///
/// # Errors
///
/// Returns an error if ffmpeg is not available or the conversion fails.
fn convert_to_wav(input: &Path) -> Result<PathBuf> {
    let wav_path = input.with_extension("whisper.wav");

    let output = Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            input.to_str().unwrap_or(""),
            "-ar",
            "16000",
            "-ac",
            "1",
            "-c:a",
            "pcm_s16le",
            wav_path.to_str().unwrap_or(""),
        ])
        .output()
        .context("Failed to run ffmpeg")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("ffmpeg conversion failed: {stderr}"));
    }

    Ok(wav_path)
}

/// Read WAV file samples as f32 normalized to [-1, 1].
///
/// # Errors
///
/// Returns an error if the WAV file cannot be opened or read.
fn read_wav_samples(path: &Path) -> Result<Vec<f32>> {
    let reader = hound::WavReader::open(path).context("Failed to open WAV file")?;
    let spec = reader.spec();

    if spec.channels != 1 {
        warn!("WAV has {} channels, expected mono", spec.channels);
    }

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => reader
            .into_samples::<i16>()
            .filter_map(std::result::Result::ok)
            .map(|s| f32::from(s) / f32::from(i16::MAX))
            .collect(),
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(std::result::Result::ok)
            .collect(),
    };

    Ok(samples)
}
