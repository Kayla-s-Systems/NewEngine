//! Strict binary decoder for NEF8 `.yscd` sound-cue dictionaries.
//!
//! `engine.assets` owns the NEF8 envelope. This module owns only the inflated
//! YSCD domain body and verifies each embedded encoded-audio payload hash.

use serde::{Deserialize, Serialize};

pub const YSCD_BINARY_MAGIC: [u8; 4] = *b"YSCD";
pub const YSCD_BINARY_SCHEMA_VERSION: u16 = 1;
const HEADER_LEN: usize = 64;
const CUE_RECORD_LEN: usize = 40;
const CLIP_RECORD_LEN: usize = 88;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct YscdAttenuation {
    pub min_distance: f32,
    pub max_distance: f32,
    pub curve: String,
    pub rolloff: f32,
    pub curve_points: Vec<[f32; 2]>,
}

impl Default for YscdAttenuation {
    fn default() -> Self {
        Self {
            min_distance: 1.0,
            max_distance: 100.0,
            curve: "inverse".to_owned(),
            rolloff: 1.0,
            curve_points: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct YscdLayerDescriptor {
    pub name: String,
    pub role: String,
    pub clip_names: Vec<String>,
    pub gain: f32,
    pub pitch: f32,
    pub attenuation: Option<YscdAttenuation>,
}

impl Default for YscdLayerDescriptor {
    fn default() -> Self {
        Self {
            name: "body".to_owned(),
            role: "body".to_owned(),
            clip_names: Vec::new(),
            gain: 1.0,
            pitch: 1.0,
            attenuation: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct YscdCueDescriptor {
    pub bus: String,
    pub looping: bool,
    pub concurrency_group: String,
    pub priority: i32,
    pub spatial_policy: String,
    pub gain_range: [f32; 2],
    pub pitch_range: [f32; 2],
    pub attenuation: Option<YscdAttenuation>,
    pub layers: Vec<YscdLayerDescriptor>,
}

impl Default for YscdCueDescriptor {
    fn default() -> Self {
        Self {
            bus: "sfx".to_owned(),
            looping: false,
            concurrency_group: String::new(),
            priority: 0,
            spatial_policy: "inherit".to_owned(),
            gain_range: [1.0, 1.0],
            pitch_range: [1.0, 1.0],
            attenuation: None,
            layers: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct YscdClip {
    pub name: String,
    pub source: String,
    pub codec: String,
    pub weight: f32,
    pub gain: f32,
    pub pitch: f32,
    pub bytes: Vec<u8>,
    pub payload_hash: [u8; 32],
}

#[derive(Clone, Debug, PartialEq)]
pub struct YscdCue {
    pub name: String,
    pub stable_hash: u64,
    pub descriptor: YscdCueDescriptor,
    pub clips: Vec<YscdClip>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct YscdDictionary {
    pub cues: Vec<YscdCue>,
}

impl YscdDictionary {
    #[inline]
    pub fn cue(&self, name: &str) -> Option<&YscdCue> {
        self.cues
            .iter()
            .find(|cue| cue.name.eq_ignore_ascii_case(name))
    }
}

pub fn decode_yscd_nef8(source: &[u8], logical_path: &str) -> Result<YscdDictionary, String> {
    let envelope = newengine_assets_api::decode_list_file_envelope(
        source,
        crate::yscd::CONTENT_KIND,
        logical_path,
    )?;
    if envelope.header.content_schema_version != crate::yscd::CONTENT_SCHEMA_VERSION {
        return Err(format!(
            "YSCD content schema mismatch path='{logical_path}' expected={} actual={}",
            crate::yscd::CONTENT_SCHEMA_VERSION,
            envelope.header.content_schema_version
        ));
    }
    decode_yscd_binary_body(&envelope.body)
        .map_err(|error| format!("YSCD decode failed path='{logical_path}': {error}"))
}

pub fn decode_yscd_binary_body(body: &[u8]) -> Result<YscdDictionary, String> {
    if body.len() < HEADER_LEN || body.get(0..4) != Some(&YSCD_BINARY_MAGIC) {
        return Err("YSCD body magic mismatch or truncated header".to_owned());
    }
    let schema = read_u16(body, 4)?;
    if schema != YSCD_BINARY_SCHEMA_VERSION {
        return Err(format!("unsupported YSCD body schema {schema}"));
    }
    let cue_count = read_u32(body, 8)? as usize;
    let clip_count = read_u32(body, 12)? as usize;
    let cue_table_offset = to_usize(read_u64(body, 16)?, "cue table offset")?;
    let clip_table_offset = to_usize(read_u64(body, 24)?, "clip table offset")?;
    let string_table_offset = to_usize(read_u64(body, 32)?, "string table offset")?;
    let string_table_len = to_usize(read_u64(body, 40)?, "string table len")?;
    let payload_offset = to_usize(read_u64(body, 48)?, "payload offset")?;
    let payload_len = to_usize(read_u64(body, 56)?, "payload len")?;

    checked_slice(
        body,
        cue_table_offset,
        cue_count
            .checked_mul(CUE_RECORD_LEN)
            .ok_or("cue table overflow")?,
        "cue table",
    )?;
    checked_slice(
        body,
        clip_table_offset,
        clip_count
            .checked_mul(CLIP_RECORD_LEN)
            .ok_or("clip table overflow")?,
        "clip table",
    )?;
    let strings = checked_slice(body, string_table_offset, string_table_len, "string table")?;
    checked_slice(body, payload_offset, payload_len, "payload region")?;

    #[derive(Clone)]
    struct ClipRow {
        name: String,
        source: String,
        codec: String,
        weight: f32,
        gain: f32,
        pitch: f32,
        bytes: Vec<u8>,
        hash: [u8; 32],
    }

    let mut clip_rows = Vec::with_capacity(clip_count);
    for index in 0..clip_count {
        let at = clip_table_offset + index * CLIP_RECORD_LEN;
        let name = read_cstr(strings, read_u32(body, at + 8)? as usize)?;
        let source = read_cstr(strings, read_u32(body, at + 12)? as usize)?;
        let codec = read_cstr(strings, read_u32(body, at + 16)? as usize)?;
        let payload_at = to_usize(read_u64(body, at + 40)?, "clip payload offset")?;
        let payload_len = to_usize(read_u64(body, at + 48)?, "clip payload len")?;
        let bytes = checked_slice(body, payload_at, payload_len, "clip payload")?.to_vec();
        let mut expected_hash = [0u8; 32];
        expected_hash.copy_from_slice(checked_slice(body, at + 56, 32, "clip hash")?);
        let actual_hash = *blake3::hash(&bytes).as_bytes();
        if actual_hash != expected_hash {
            return Err(format!("YSCD clip '{name}' BLAKE3 mismatch"));
        }
        clip_rows.push(ClipRow {
            name,
            source,
            codec,
            weight: read_f32(body, at + 24)?,
            gain: read_f32(body, at + 28)?,
            pitch: read_f32(body, at + 32)?,
            bytes,
            hash: expected_hash,
        });
    }

    let mut cues = Vec::with_capacity(cue_count);
    for index in 0..cue_count {
        let at = cue_table_offset + index * CUE_RECORD_LEN;
        let stable_hash = read_u64(body, at)?;
        let name = read_cstr(strings, read_u32(body, at + 8)? as usize)?;
        let descriptor = read_cstr(strings, read_u32(body, at + 12)? as usize)?;
        let descriptor_len = read_u32(body, at + 16)? as usize;
        if descriptor.len() != descriptor_len {
            return Err(format!("cue '{name}' descriptor length mismatch"));
        }
        let descriptor: YscdCueDescriptor = serde_json::from_str(&descriptor)
            .map_err(|error| format!("cue '{name}' descriptor JSON invalid: {error}"))?;
        let first = read_u32(body, at + 20)? as usize;
        let count = read_u32(body, at + 24)? as usize;
        let end = first.checked_add(count).ok_or("cue clip range overflow")?;
        if end > clip_rows.len() {
            return Err(format!("cue '{name}' clip range out of bounds"));
        }
        let clips = clip_rows[first..end]
            .iter()
            .map(|row| YscdClip {
                name: row.name.clone(),
                source: row.source.clone(),
                codec: row.codec.clone(),
                weight: row.weight,
                gain: row.gain,
                pitch: row.pitch,
                bytes: row.bytes.clone(),
                payload_hash: row.hash,
            })
            .collect();
        cues.push(YscdCue {
            name,
            stable_hash,
            descriptor,
            clips,
        });
    }
    Ok(YscdDictionary { cues })
}

fn checked_slice<'a>(
    bytes: &'a [u8],
    offset: usize,
    len: usize,
    label: &str,
) -> Result<&'a [u8], String> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| format!("{label} range overflow"))?;
    bytes.get(offset..end).ok_or_else(|| {
        format!(
            "{label} truncated offset={offset} len={len} bytes={}",
            bytes.len()
        )
    })
}

fn read_cstr(strings: &[u8], offset: usize) -> Result<String, String> {
    let tail = strings
        .get(offset..)
        .ok_or("YSCD string offset out of bounds")?;
    let len = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or("YSCD string is not NUL terminated")?;
    String::from_utf8(tail[..len].to_vec())
        .map_err(|error| format!("YSCD string is not UTF-8: {error}"))
}

fn to_usize(value: u64, label: &str) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| format!("{label} does not fit usize: {value}"))
}

fn read_u16(bytes: &[u8], at: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        checked_slice(bytes, at, 2, "u16")?
            .try_into()
            .map_err(|_| "invalid u16 slice")?,
    ))
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        checked_slice(bytes, at, 4, "u32")?
            .try_into()
            .map_err(|_| "invalid u32 slice")?,
    ))
}

fn read_u64(bytes: &[u8], at: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        checked_slice(bytes, at, 8, "u64")?
            .try_into()
            .map_err(|_| "invalid u64 slice")?,
    ))
}

fn read_f32(bytes: &[u8], at: usize) -> Result<f32, String> {
    Ok(f32::from_le_bytes(
        checked_slice(bytes, at, 4, "f32")?
            .try_into()
            .map_err(|_| "invalid f32 slice")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_yscd_body() {
        assert!(decode_yscd_binary_body(b"NOPE").is_err());
    }
}
