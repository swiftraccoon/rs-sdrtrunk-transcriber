//! Audio utility functions

use tracing::{debug, warn};

/// MP3 header parsing for duration calculation
#[derive(Debug)]
struct Mp3Header {
    version: u8,      // MPEG version
    layer: u8,        // Layer (1, 2, or 3)
    bitrate: u32,     // Bitrate in kbps
    sample_rate: u32, // Sample rate in Hz
    #[allow(dead_code)]
    padding: bool, // Padding bit
    frame_size: usize, // Frame size in bytes
}

impl Mp3Header {
    /// Parse MP3 header from 4 bytes
    fn parse(header_bytes: &[u8]) -> Option<Self> {
        if header_bytes.len() < 4 {
            return None;
        }

        // Check sync bits (11 bits set to 1)
        if header_bytes[0] != 0xFF || (header_bytes[1] & 0xE0) != 0xE0 {
            return None;
        }

        // Extract version (bits 19-20)
        let version_bits = (header_bytes[1] >> 3) & 0x03;
        let version = match version_bits {
            0x00 => 2, // MPEG 2.5
            0x02 => 2, // MPEG 2
            0x03 => 1, // MPEG 1
            _ => return None,
        };

        // Extract layer (bits 17-18)
        let layer_bits = (header_bytes[1] >> 1) & 0x03;
        let layer = match layer_bits {
            0x01 => 3, // Layer III
            0x02 => 2, // Layer II
            0x03 => 1, // Layer I
            _ => return None,
        };

        // Extract bitrate index (bits 12-15)
        let bitrate_index = (header_bytes[2] >> 4) & 0x0F;

        // Bitrate tables for MPEG1/2 Layer III
        let bitrate = if version == 1 && layer == 3 {
            // MPEG1 Layer III
            match bitrate_index {
                0x01 => 32,
                0x02 => 40,
                0x03 => 48,
                0x04 => 56,
                0x05 => 64,
                0x06 => 80,
                0x07 => 96,
                0x08 => 112,
                0x09 => 128,
                0x0A => 160,
                0x0B => 192,
                0x0C => 224,
                0x0D => 256,
                0x0E => 320,
                _ => return None,
            }
        } else if version == 2 && layer == 3 {
            // MPEG2/2.5 Layer III
            match bitrate_index {
                0x01 => 8,
                0x02 => 16,
                0x03 => 24,
                0x04 => 32,
                0x05 => 40,
                0x06 => 48,
                0x07 => 56,
                0x08 => 64,
                0x09 => 80,
                0x0A => 96,
                0x0B => 112,
                0x0C => 128,
                0x0D => 144,
                0x0E => 160,
                _ => return None,
            }
        } else {
            return None; // Unsupported combination
        };

        // Extract sample rate index (bits 10-11)
        let sample_rate_index = (header_bytes[2] >> 2) & 0x03;
        let sample_rate = match (version, sample_rate_index) {
            (1, 0x00) => 44100,
            (1, 0x01) => 48000,
            (1, 0x02) => 32000,
            (2, 0x00) => 22050,
            (2, 0x01) => 24000,
            (2, 0x02) => 16000,
            _ => return None,
        };

        // Padding bit (bit 9)
        let padding = (header_bytes[2] & 0x02) != 0;

        // Calculate frame size
        let frame_size = if layer == 3 {
            // Layer III frame size calculation
            let samples_per_frame = if version == 1 { 1152 } else { 576 };
            let frame_size = (samples_per_frame * bitrate * 125) / sample_rate;
            if padding { frame_size + 1 } else { frame_size }
        } else {
            return None; // Only support Layer III for now
        };

        Some(Mp3Header {
            version,
            layer,
            bitrate,
            sample_rate,
            padding,
            frame_size: frame_size as usize,
        })
    }
}

/// Calculate the duration of an MP3 file in seconds
pub fn calculate_mp3_duration(audio_data: &[u8]) -> Option<f64> {
    if audio_data.len() < 4 {
        return None;
    }

    let mut total_frames = 0;
    let mut cursor = 0;
    let mut first_header: Option<Mp3Header> = None;

    // Scan through the file looking for MP3 frames
    while cursor < audio_data.len() - 4 {
        // Try to parse header at current position
        if let Some(header) = Mp3Header::parse(&audio_data[cursor..cursor + 4]) {
            if first_header.is_none() {
                debug!(
                    "Found MP3 header: MPEG{} Layer {}, {}kbps, {}Hz",
                    header.version, header.layer, header.bitrate, header.sample_rate
                );
                first_header = Some(header);
            }
            total_frames += 1;

            // Jump to next frame (skip current frame)
            if let Some(ref hdr) = first_header {
                cursor += hdr.frame_size;
                continue;
            }
        }
        cursor += 1;
    }

    if let Some(header) = first_header {
        if total_frames > 0 {
            // Calculate duration based on actual frame count
            let samples_per_frame = if header.version == 1 { 1152.0 } else { 576.0 };
            let duration = (total_frames as f64 * samples_per_frame) / header.sample_rate as f64;

            debug!(
                "Calculated MP3 duration: {:.2}s ({} frames, {}kbps, {}Hz)",
                duration, total_frames, header.bitrate, header.sample_rate
            );

            return Some(duration);
        }

        // Fallback: estimate based on file size and detected bitrate
        let file_size_bits = audio_data.len() as f64 * 8.0;
        let duration = file_size_bits / (header.bitrate as f64 * 1000.0);

        debug!(
            "Estimated MP3 duration from bitrate: {:.2}s ({}kbps)",
            duration, header.bitrate
        );

        return Some(duration);
    }

    // Last resort: rough estimate assuming 16kbps (common for voice)
    let file_size_kb = audio_data.len() as f64 / 1024.0;
    let estimated_duration = file_size_kb / 2.0; // 16kbps = 2 KB/s

    warn!(
        "Could not parse MP3 headers, using fallback estimate: {:.2}s for {} bytes",
        estimated_duration,
        audio_data.len()
    );
    Some(estimated_duration)
}

/// Calculate audio duration based on file extension
pub fn calculate_audio_duration(audio_data: &[u8], filename: Option<&str>) -> Option<f64> {
    // Determine file type from extension if available
    if let Some(name) = filename
        && name.to_lowercase().ends_with(".mp3")
    {
        return calculate_mp3_duration(audio_data);
    }
    // Add other formats as needed (WAV, FLAC, etc.)

    // Try to detect MP3 by magic bytes if no filename
    if audio_data.len() > 2 && audio_data[0] == 0xFF && (audio_data[1] & 0xE0) == 0xE0 {
        return calculate_mp3_duration(audio_data);
    }

    None
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
mod tests {
    use super::*;

    /// Create a valid MP3 header with specified parameters
    fn create_mp3_header(
        version: u8,
        layer: u8,
        bitrate_index: u8,
        sample_rate_index: u8,
        padding: bool,
    ) -> Vec<u8> {
        let mut header = vec![0xFF]; // Sync byte 1

        // Byte 2: sync (3 bits) | version (2 bits) | layer (2 bits) | protection (1 bit)
        let version_bits = match version {
            1 => 0b11, // MPEG 1
            2 => 0b10, // MPEG 2
            _ => 0b00, // MPEG 2.5
        };
        let layer_bits = match layer {
            1 => 0b11, // Layer I
            2 => 0b10, // Layer II
            3 => 0b01, // Layer III
            _ => 0b00, // Reserved
        };
        header.push(0xE0 | (version_bits << 3) | (layer_bits << 1) | 0x01);

        // Byte 3: bitrate index (4 bits) | sample rate (2 bits) | padding (1 bit) | private (1 bit)
        let padding_bit = if padding { 0x02 } else { 0x00 };
        header.push((bitrate_index << 4) | (sample_rate_index << 2) | padding_bit);

        // Byte 4: channel mode (2 bits) | mode extension (2 bits) | copyright (1 bit) | original (1 bit) | emphasis (2 bits)
        header.push(0xC0); // Stereo, no extensions

        header
    }

    #[test]
    fn test_mp3_header_parse_mpeg1_layer3_128kbps_44100hz() {
        let header = create_mp3_header(1, 3, 0x09, 0x00, false);
        let parsed = Mp3Header::parse(&header).expect("Failed to parse header");

        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.layer, 3);
        assert_eq!(parsed.bitrate, 128);
        assert_eq!(parsed.sample_rate, 44100);
        assert!(!parsed.padding);
    }

    #[test]
    fn test_mp3_header_parse_mpeg2_layer3_16kbps_22050hz() {
        let header = create_mp3_header(2, 3, 0x02, 0x00, false);
        let parsed = Mp3Header::parse(&header).expect("Failed to parse header");

        assert_eq!(parsed.version, 2);
        assert_eq!(parsed.layer, 3);
        assert_eq!(parsed.bitrate, 16);
        assert_eq!(parsed.sample_rate, 22050);
    }

    #[test]
    fn test_mp3_header_parse_invalid_sync() {
        let header = vec![0xFE, 0xE0, 0x90, 0xC0]; // Invalid sync
        assert!(Mp3Header::parse(&header).is_none());
    }

    #[test]
    fn test_mp3_header_parse_too_short() {
        let header = vec![0xFF, 0xFB]; // Too short
        assert!(Mp3Header::parse(&header).is_none());
    }

    #[test]
    fn test_calculate_mp3_duration_single_frame() {
        // Create a minimal MP3 with one frame
        // MPEG1 Layer III, 128kbps, 44100Hz = 418 bytes per frame
        let mut data = create_mp3_header(1, 3, 0x09, 0x00, false);
        data.extend(vec![0x00; 414]); // Pad to frame size

        let duration = calculate_mp3_duration(&data);
        assert!(duration.is_some());

        // 1152 samples at 44100Hz = ~0.026 seconds
        let expected = 1152.0 / 44100.0;
        assert!((duration.unwrap() - expected).abs() < 0.001);
    }

    #[test]
    fn test_calculate_mp3_duration_multiple_frames() {
        // MPEG2 Layer III, 16kbps, 22050Hz
        let mut data = Vec::new();

        // Add 10 frames
        for _ in 0..10 {
            data.extend(create_mp3_header(2, 3, 0x02, 0x00, false));
            data.extend(vec![0x00; 68]); // Frame size for 16kbps at 22050Hz
        }

        let duration = calculate_mp3_duration(&data);
        assert!(duration.is_some());

        // 10 frames * 576 samples at 22050Hz = ~0.261 seconds
        let expected = (10.0 * 576.0) / 22050.0;
        assert!((duration.unwrap() - expected).abs() < 0.01);
    }

    #[test]
    fn test_calculate_mp3_duration_empty_data() {
        let data = vec![];
        assert!(calculate_mp3_duration(&data).is_none());
    }

    #[test]
    fn test_calculate_mp3_duration_invalid_data() {
        let data = vec![0x00, 0x00, 0x00, 0x00];
        let duration = calculate_mp3_duration(&data);
        // Should return fallback estimate
        assert!(duration.is_some());
    }

    #[test]
    fn test_calculate_audio_duration_with_mp3_extension() {
        let header = create_mp3_header(1, 3, 0x09, 0x00, false);
        let mut data = header.clone();
        data.extend(vec![0x00; 414]);

        let duration = calculate_audio_duration(&data, Some("test.mp3"));
        assert!(duration.is_some());
    }

    #[test]
    fn test_calculate_audio_duration_with_mp3_magic_bytes() {
        let header = create_mp3_header(1, 3, 0x09, 0x00, false);
        let mut data = header.clone();
        data.extend(vec![0x00; 414]);

        let duration = calculate_audio_duration(&data, None);
        assert!(duration.is_some());
    }

    #[test]
    fn test_calculate_audio_duration_non_mp3() {
        let data = vec![0x52, 0x49, 0x46, 0x46]; // WAV header
        let duration = calculate_audio_duration(&data, Some("test.wav"));
        assert!(duration.is_none());
    }
}
