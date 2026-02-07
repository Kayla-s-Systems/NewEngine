#![forbid(unsafe_op_in_unsafe_fn)]

use crate::types::Asset;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Wav,
    Ogg,
    Mp3,
    Flac,
    Aac,
    M4a,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct AudioMeta {
    pub schema: String,
    pub container: String,
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub frames: u64,
    pub duration_sec: f64,
}

#[derive(Debug, Clone)]
pub struct AudioAsset {
    pub format: AudioFormat,
    pub meta: AudioMeta,
    pub payload: Vec<u8>,
}

impl Asset for AudioAsset {
    #[inline]
    fn type_name() -> &'static str {
        "AudioAsset"
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AudioReadError {
    #[error("wire: too short")]
    TooShort,
    #[error("wire: meta length out of bounds")]
    MetaOutOfBounds,
    #[error("wire: meta length too large ({0} bytes)")]
    MetaTooLarge(usize),
    #[error("utf8: {0}")]
    Utf8(String),
    #[error("meta json: {0}")]
    MetaJson(String),
}

pub struct AudioReader;

impl AudioReader {
    /// Hard cap to prevent pathological allocations / malformed assets.
    pub const MAX_META_BYTES: usize = 64 * 1024;

    /// Builds AudioAsset from split parts:
    /// - meta_json: blob.meta_json
    /// - payload: blob.payload (original bytes)
    pub fn from_blob_parts(meta_json: &str, payload: &[u8]) -> Result<AudioAsset, AudioReadError> {
        let meta = parse_meta_json(meta_json)?;
        let format = detect_format(&meta.container);
        Ok(AudioAsset {
            format,
            meta,
            payload: payload.to_vec(),
        })
    }

    /// Decodes importer wire:
    /// [4] meta_len_le (u32)
    /// [N] meta_json utf8
    /// [..] payload bytes (rest)
    pub fn read_wire(bytes: &[u8]) -> Result<AudioAsset, AudioReadError> {
        if bytes.len() < 4 {
            return Err(AudioReadError::TooShort);
        }

        let meta_len = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        if meta_len > Self::MAX_META_BYTES {
            return Err(AudioReadError::MetaTooLarge(meta_len));
        }

        let meta_start = 4usize;
        let meta_end = meta_start.saturating_add(meta_len);
        if meta_end > bytes.len() {
            return Err(AudioReadError::MetaOutOfBounds);
        }

        let meta_bytes = &bytes[meta_start..meta_end];
        let payload = &bytes[meta_end..];

        let meta_str =
            std::str::from_utf8(meta_bytes).map_err(|e| AudioReadError::Utf8(e.to_string()))?;

        Self::from_blob_parts(meta_str, payload)
    }
}

fn parse_meta_json(meta_json: &str) -> Result<AudioMeta, AudioReadError> {
    let v: JsonValue =
        serde_json::from_str(meta_json).map_err(|e| AudioReadError::MetaJson(e.to_string()))?;

    let schema = v.get("schema").and_then(|x| x.as_str()).unwrap_or("").to_owned();

    let container_raw = v
        .get("container")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_owned();

    let container = normalize_container(&container_raw);

    let codec = v.get("codec").and_then(|x| x.as_str()).unwrap_or("").to_owned();

    let sample_rate = v.get("sample_rate").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    let channels = v.get("channels").and_then(|x| x.as_u64()).unwrap_or(0) as u16;
    let bits_per_sample = v
        .get("bits_per_sample")
        .and_then(|x| x.as_u64())
        .unwrap_or(0) as u16;

    let frames = v.get("frames").and_then(|x| x.as_u64()).unwrap_or(0);
    let duration_sec = v.get("duration_sec").and_then(|x| x.as_f64()).unwrap_or(0.0);

    Ok(AudioMeta {
        schema,
        container,
        codec,
        sample_rate,
        channels,
        bits_per_sample,
        frames,
        duration_sec,
    })
}

#[inline]
fn normalize_container(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "wave" | "wav" => "wav".to_owned(),
        "ogg" | "oga" => "ogg".to_owned(),
        "mp3" => "mp3".to_owned(),
        "flac" => "flac".to_owned(),
        "aac" => "aac".to_owned(),
        "m4a" | "mp4" => "m4a".to_owned(),
        other => other.to_owned(),
    }
}

#[inline]
fn detect_format(container: &str) -> AudioFormat {
    match container {
        "wav" => AudioFormat::Wav,
        "ogg" => AudioFormat::Ogg,
        "mp3" => AudioFormat::Mp3,
        "flac" => AudioFormat::Flac,
        "aac" => AudioFormat::Aac,
        "m4a" => AudioFormat::M4a,
        _ => AudioFormat::Unknown,
    }
}