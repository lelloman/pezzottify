//! Audio file conversion using ffmpeg/ffprobe.

use serde::Deserialize;
use std::path::Path;
use std::process::Stdio;
use thiserror::Error;
use tokio::process::Command;

/// Errors that can occur during audio conversion.
#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("ffprobe failed: {0}")]
    ProbeFailed(String),

    #[error("ffmpeg failed: {0}")]
    ConversionFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid output: {0}")]
    InvalidOutput(String),

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),
}

/// Audio metadata extracted from ffprobe.
#[derive(Debug, Clone)]
pub struct AudioMetadata {
    /// Duration in milliseconds.
    pub duration_ms: i64,
    /// Audio codec name.
    pub codec: String,
    /// Bitrate in kbps.
    pub bitrate: Option<i32>,
    /// Sample rate in Hz.
    pub sample_rate: Option<i32>,
    /// Number of channels.
    pub channels: Option<i32>,
    /// Format name (e.g., "mp3", "flac").
    pub format: String,
}

/// ffprobe JSON output structure.
#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    format: FfprobeFormat,
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    format_name: String,
    duration: Option<String>,
    bit_rate: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_type: String,
    codec_name: Option<String>,
    sample_rate: Option<String>,
    channels: Option<i32>,
    bit_rate: Option<String>,
}

/// Probe an audio file to extract metadata.
pub async fn probe_audio_file(path: &Path) -> Result<AudioMetadata, ConversionError> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ConversionError::ProbeFailed(stderr.to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let probe: FfprobeOutput = serde_json::from_str(&stdout)
        .map_err(|e| ConversionError::InvalidOutput(format!("JSON parse error: {}", e)))?;

    // Find the audio stream
    let audio_stream = probe
        .streams
        .iter()
        .find(|s| s.codec_type == "audio")
        .ok_or_else(|| ConversionError::InvalidOutput("No audio stream found".to_string()))?;

    // Parse duration (in seconds from ffprobe)
    let duration_secs: f64 = probe
        .format
        .duration
        .as_ref()
        .and_then(|d| d.parse().ok())
        .unwrap_or(0.0);
    let duration_ms = (duration_secs * 1000.0) as i64;

    // Parse bitrate (prefer stream bitrate, fall back to format bitrate)
    let bitrate_str = audio_stream
        .bit_rate
        .as_ref()
        .or(probe.format.bit_rate.as_ref());
    let bitrate = bitrate_str
        .and_then(|b| b.parse::<i64>().ok())
        .map(|b| (b / 1000) as i32);

    // Parse sample rate
    let sample_rate = audio_stream
        .sample_rate
        .as_ref()
        .and_then(|sr| sr.parse().ok());

    Ok(AudioMetadata {
        duration_ms,
        codec: audio_stream
            .codec_name
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        bitrate,
        sample_rate,
        channels: audio_stream.channels,
        format: probe.format.format_name,
    })
}

/// Convert an audio file to OGG Vorbis format.
///
/// # Arguments
/// * `input_path` - Path to the input audio file
/// * `output_path` - Path for the output OGG file
/// * `bitrate_kbps` - Target bitrate in kbps (e.g., 320)
pub async fn convert_to_ogg(
    input_path: &Path,
    output_path: &Path,
    bitrate_kbps: u32,
) -> Result<(), ConversionError> {
    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let output = Command::new("ffmpeg")
        .args([
            "-i",
            input_path.to_str().unwrap_or(""),
            "-c:a",
            "libvorbis",
            "-b:a",
            &format!("{}k", bitrate_kbps),
            "-vn", // No video
            "-y",  // Overwrite output
        ])
        .arg(output_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ConversionError::ConversionFailed(stderr.to_string()));
    }

    Ok(())
}

/// Check if ffmpeg and ffprobe are available.
#[allow(dead_code)]
pub async fn check_ffmpeg_available() -> Result<(), ConversionError> {
    // Check ffprobe
    let ffprobe_result = Command::new("ffprobe")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;

    if ffprobe_result.is_err() || !ffprobe_result.unwrap().success() {
        return Err(ConversionError::ProbeFailed(
            "ffprobe not found or not working".to_string(),
        ));
    }

    // Check ffmpeg
    let ffmpeg_result = Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;

    if ffmpeg_result.is_err() || !ffmpeg_result.unwrap().success() {
        return Err(ConversionError::ConversionFailed(
            "ffmpeg not found or not working".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_metadata_fields() {
        let metadata = AudioMetadata {
            duration_ms: 180000,
            codec: "mp3".to_string(),
            bitrate: Some(320),
            sample_rate: Some(44100),
            channels: Some(2),
            format: "mp3".to_string(),
        };

        assert_eq!(metadata.duration_ms, 180000);
        assert_eq!(metadata.codec, "mp3");
        assert_eq!(metadata.bitrate, Some(320));
    }
}
