//! Native NorthStar audio clip body stored inside a NEF8/ListFile envelope.
//!
//! This is the new runtime audio asset path. YSNCD remains a separate legacy
//! sound-cue dictionary format and is intentionally not reused for new imports.

pub const AUDIO_CLIP_BINARY_MAGIC: [u8; 4] = *b"AUDP";
pub const AUDIO_CLIP_BINARY_SCHEMA_VERSION: u16 = 1;
pub const AUDIO_CLIP_ENCODING_PCM_F32_LE: u16 = 1;

const HEADER_LEN: usize = 64;
const FLAG_LOOP: u32 = 1 << 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioClipLoopRegion {
    pub start_frame: u64,
    pub end_frame: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioClipBinary {
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
    pub loop_region: Option<AudioClipLoopRegion>,
}

impl AudioClipBinary {
    pub fn frame_count(&self) -> Result<u64, String> {
        validate_format(self.sample_rate_hz, self.channels)?;
        if self.samples.len() % usize::from(self.channels) != 0 {
            return Err(format!(
                "NEF8 audio clip interleaved sample count {} is not divisible by channels {}",
                self.samples.len(),
                self.channels
            ));
        }
        u64::try_from(self.samples.len() / usize::from(self.channels))
            .map_err(|_| "NEF8 audio clip frame count exceeds u64".to_owned())
    }

    pub fn validate(&self) -> Result<(), String> {
        let frames = self.frame_count()?;
        if self.samples.iter().any(|sample| !sample.is_finite()) {
            return Err("NEF8 audio clip samples contain non-finite values".to_owned());
        }
        if let Some(loop_region) = self.loop_region {
            if loop_region.start_frame >= loop_region.end_frame || loop_region.end_frame > frames {
                return Err(format!(
                    "NEF8 audio clip loop range invalid start={} end={} frames={frames}",
                    loop_region.start_frame, loop_region.end_frame
                ));
            }
        }
        Ok(())
    }
}

pub fn encode_audio_clip_binary_body(clip: &AudioClipBinary) -> Result<Vec<u8>, String> {
    clip.validate()?;
    let frame_count = clip.frame_count()?;
    let sample_bytes_len = clip
        .samples
        .len()
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| "NEF8 audio clip sample byte length overflow".to_owned())?;
    let total_len = HEADER_LEN
        .checked_add(sample_bytes_len)
        .ok_or_else(|| "NEF8 audio clip output length overflow".to_owned())?;
    let mut out = vec![0_u8; total_len];
    out[0..4].copy_from_slice(&AUDIO_CLIP_BINARY_MAGIC);
    write_u16(&mut out, 4, AUDIO_CLIP_BINARY_SCHEMA_VERSION)?;
    write_u16(&mut out, 6, AUDIO_CLIP_ENCODING_PCM_F32_LE)?;
    write_u16(&mut out, 8, clip.channels)?;
    write_u16(&mut out, 10, 0)?;
    write_u32(&mut out, 12, clip.sample_rate_hz)?;
    write_u64(&mut out, 16, frame_count)?;
    let (flags, loop_start, loop_end) = match clip.loop_region {
        Some(loop_region) => (FLAG_LOOP, loop_region.start_frame, loop_region.end_frame),
        None => (0, 0, 0),
    };
    write_u32(&mut out, 24, flags)?;
    write_u32(&mut out, 28, HEADER_LEN as u32)?;
    write_u64(&mut out, 32, loop_start)?;
    write_u64(&mut out, 40, loop_end)?;
    write_u64(&mut out, 48, sample_bytes_len as u64)?;
    write_u64(&mut out, 56, 0)?;

    let mut cursor = HEADER_LEN;
    for sample in &clip.samples {
        out[cursor..cursor + 4].copy_from_slice(&sample.to_le_bytes());
        cursor += 4;
    }
    Ok(out)
}

pub fn decode_audio_clip_binary_body(body: &[u8]) -> Result<AudioClipBinary, String> {
    if body.len() < HEADER_LEN {
        return Err(format!(
            "NEF8 audio clip body too small bytes={} expected>={HEADER_LEN}",
            body.len()
        ));
    }
    if body.get(0..4) != Some(&AUDIO_CLIP_BINARY_MAGIC[..]) {
        return Err("NEF8 audio clip body magic mismatch".to_owned());
    }
    let schema = read_u16(body, 4)?;
    if schema != AUDIO_CLIP_BINARY_SCHEMA_VERSION {
        return Err(format!(
            "unsupported NEF8 audio clip body schema {schema}; expected {AUDIO_CLIP_BINARY_SCHEMA_VERSION}"
        ));
    }
    let encoding = read_u16(body, 6)?;
    if encoding != AUDIO_CLIP_ENCODING_PCM_F32_LE {
        return Err(format!("unsupported NEF8 audio clip encoding {encoding}"));
    }
    let channels = read_u16(body, 8)?;
    let sample_rate_hz = read_u32(body, 12)?;
    validate_format(sample_rate_hz, channels)?;
    let frame_count = read_u64(body, 16)?;
    let flags = read_u32(body, 24)?;
    let sample_data_offset = usize::try_from(read_u32(body, 28)?)
        .map_err(|_| "NEF8 audio clip sample data offset exceeds usize".to_owned())?;
    let loop_start = read_u64(body, 32)?;
    let loop_end = read_u64(body, 40)?;
    let sample_data_len = usize::try_from(read_u64(body, 48)?)
        .map_err(|_| "NEF8 audio clip sample data length exceeds usize".to_owned())?;

    if sample_data_offset < HEADER_LEN {
        return Err(format!(
            "NEF8 audio clip sample data overlaps header offset={sample_data_offset} header={HEADER_LEN}"
        ));
    }
    let sample_data_end = sample_data_offset
        .checked_add(sample_data_len)
        .ok_or_else(|| "NEF8 audio clip sample data range overflow".to_owned())?;
    let sample_data = body.get(sample_data_offset..sample_data_end).ok_or_else(|| {
        format!(
            "NEF8 audio clip sample data truncated offset={sample_data_offset} len={sample_data_len} body={}",
            body.len()
        )
    })?;
    if sample_data_len % 4 != 0 {
        return Err(format!(
            "NEF8 audio clip PCM F32 byte count is not 4-byte aligned: {sample_data_len}"
        ));
    }
    let expected_samples = usize::try_from(frame_count)
        .ok()
        .and_then(|frames| frames.checked_mul(usize::from(channels)))
        .ok_or_else(|| "NEF8 audio clip expected sample count overflow".to_owned())?;
    if sample_data_len / 4 != expected_samples {
        return Err(format!(
            "NEF8 audio clip sample count mismatch frames={frame_count} channels={channels} expected={expected_samples} actual={}",
            sample_data_len / 4
        ));
    }

    let mut samples = Vec::with_capacity(expected_samples);
    for bytes in sample_data.chunks_exact(4) {
        let sample = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        if !sample.is_finite() {
            return Err("NEF8 audio clip samples contain non-finite values".to_owned());
        }
        samples.push(sample);
    }

    let loop_region = if flags & FLAG_LOOP != 0 {
        Some(AudioClipLoopRegion {
            start_frame: loop_start,
            end_frame: loop_end,
        })
    } else {
        None
    };
    let clip = AudioClipBinary {
        sample_rate_hz,
        channels,
        samples,
        loop_region,
    };
    clip.validate()?;
    Ok(clip)
}

pub fn decode_audio_clip_nef8(
    source: &[u8],
    logical_path: &str,
    expected_content_kind: u32,
    expected_schema_version: u16,
) -> Result<AudioClipBinary, String> {
    let envelope = newengine_assets_api::decode_list_file_envelope(
        source,
        expected_content_kind,
        logical_path,
    )?;
    if envelope.header.content_schema_version != expected_schema_version {
        return Err(format!(
            "NEF8 audio clip content schema mismatch path='{logical_path}' expected={} actual={}",
            expected_schema_version, envelope.header.content_schema_version
        ));
    }
    decode_audio_clip_binary_body(&envelope.body)
        .map_err(|error| format!("NEF8 audio clip decode failed path='{logical_path}': {error}"))
}

fn validate_format(sample_rate_hz: u32, channels: u16) -> Result<(), String> {
    if channels == 0 || channels > 32 {
        return Err(format!("NEF8 audio clip channels out of range: {channels}"));
    }
    if !(8_000..=384_000).contains(&sample_rate_hz) {
        return Err(format!(
            "NEF8 audio clip sample rate out of range: {sample_rate_hz} Hz"
        ));
    }
    Ok(())
}

#[inline]
fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| format!("NEF8 audio clip truncated at u16 offset {offset}"))?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

#[inline]
fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("NEF8 audio clip truncated at u32 offset {offset}"))?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

#[inline]
fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let value = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| format!("NEF8 audio clip truncated at u64 offset {offset}"))?;
    Ok(u64::from_le_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]))
}

#[inline]
fn write_u16(bytes: &mut [u8], offset: usize, value: u16) -> Result<(), String> {
    bytes
        .get_mut(offset..offset + 2)
        .ok_or_else(|| format!("NEF8 audio clip output truncated at u16 offset {offset}"))?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[inline]
fn write_u32(bytes: &mut [u8], offset: usize, value: u32) -> Result<(), String> {
    bytes
        .get_mut(offset..offset + 4)
        .ok_or_else(|| format!("NEF8 audio clip output truncated at u32 offset {offset}"))?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[inline]
fn write_u64(bytes: &mut [u8], offset: usize, value: u64) -> Result<(), String> {
    bytes
        .get_mut(offset..offset + 8)
        .ok_or_else(|| format!("NEF8 audio clip output truncated at u64 offset {offset}"))?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_round_trip_preserves_pcm_and_loop() {
        let clip = AudioClipBinary {
            sample_rate_hz: 48_000,
            channels: 2,
            samples: vec![0.0, 0.5, -0.5, 1.0, -1.0, 0.25],
            loop_region: Some(AudioClipLoopRegion {
                start_frame: 1,
                end_frame: 3,
            }),
        };
        let bytes = encode_audio_clip_binary_body(&clip).expect("encode");
        let decoded = decode_audio_clip_binary_body(&bytes).expect("decode");
        assert_eq!(decoded, clip);
        assert_eq!(decoded.frame_count().unwrap(), 3);
    }

    #[test]
    fn rejects_non_interleaved_shape_and_non_finite_samples() {
        let bad_shape = AudioClipBinary {
            sample_rate_hz: 48_000,
            channels: 2,
            samples: vec![0.0, 1.0, 0.5],
            loop_region: None,
        };
        assert!(bad_shape.validate().is_err());

        let bad_sample = AudioClipBinary {
            sample_rate_hz: 48_000,
            channels: 1,
            samples: vec![f32::NAN],
            loop_region: None,
        };
        assert!(bad_sample.validate().is_err());
    }
}
