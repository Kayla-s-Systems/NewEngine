#![forbid(unsafe_op_in_unsafe_fn)]

//! Sony XVAG container support used by NorthStar audio content and runtime.
//!
//! The container and codec are deliberately separate. NorthStar currently writes
//! XVAG with codec `0x06` (Sony PS-ADPCM) and can identify MPEG/ATRAC9 XVAGs for
//! future decoder adapters.

pub const XVAG_MAGIC: [u8; 4] = *b"XVAG";
pub const XVAG_CODEC_PS_ADPCM: u32 = 0x06;
pub const XVAG_CODEC_PS_ADPCM_EXTENDED: u32 = 0x07;
pub const XVAG_CODEC_MPEG: u32 = 0x08;
pub const XVAG_CODEC_ATRAC9: u32 = 0x09;

const XVAG_FIRST_CHUNK_OFFSET: usize = 0x20;
const XVAG_FMAT_SIZE_V61: usize = 0x24;
const XVAG_WRITER_DATA_OFFSET: usize = 0x60;
const PS_ADPCM_FRAME_BYTES: usize = 16;
const PS_ADPCM_SAMPLES_PER_FRAME: usize = 28;

const PS_FILTERS: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XvagCodec {
    PsAdpcm,
    PsAdpcmExtended,
    Mpeg,
    Atrac9,
    Unknown(u32),
}

impl XvagCodec {
    pub const fn from_tag(tag: u32) -> Self {
        match tag {
            XVAG_CODEC_PS_ADPCM => Self::PsAdpcm,
            XVAG_CODEC_PS_ADPCM_EXTENDED => Self::PsAdpcmExtended,
            XVAG_CODEC_MPEG => Self::Mpeg,
            XVAG_CODEC_ATRAC9 => Self::Atrac9,
            other => Self::Unknown(other),
        }
    }

    pub const fn tag(self) -> u32 {
        match self {
            Self::PsAdpcm => XVAG_CODEC_PS_ADPCM,
            Self::PsAdpcmExtended => XVAG_CODEC_PS_ADPCM_EXTENDED,
            Self::Mpeg => XVAG_CODEC_MPEG,
            Self::Atrac9 => XVAG_CODEC_ATRAC9,
            Self::Unknown(tag) => tag,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XvagHeader {
    pub big_endian: bool,
    pub version_flag: u8,
    pub start_offset: u32,
    pub channels: u32,
    pub codec: XvagCodec,
    pub num_samples: u32,
    pub interleave_factor: u32,
    pub sample_rate_hz: u32,
    pub data_size: u32,
    pub subsongs: u32,
    pub layers: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedPcmF32 {
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XvagAtrac9Info {
    pub frame_size: u32,
    pub samples_per_frame: u32,
    /// ATRAC9 config words are stored big-endian even in little-endian XVAGs.
    pub config_data_be: u32,
    pub fact_num_samples: u32,
    pub decoder_delay: u32,
    pub encoder_delay: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XvagMpegInfo {
    pub mpeg_version: u32,
    pub layer: u32,
    pub bit_rate: u32,
    pub sample_rate_hz: u32,
    pub stream_version: u32,
    pub channels_per_stream: u32,
    pub channels_total_or_stream: u32,
    pub fixed_frame_size: u32,
    pub encoder_delay: u32,
    pub num_samples: u32,
    pub data_size: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XvagCodecInfo {
    PsAdpcm,
    PsAdpcmExtended,
    Mpeg(XvagMpegInfo),
    Atrac9(XvagAtrac9Info),
    Unknown { tag: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XvagDemuxDescriptor {
    pub codec: XvagCodec,
    pub data_offset: u32,
    pub data_size: u32,
    pub codec_frame_bytes: u32,
    pub interleave_block_bytes: u32,
    pub stream_count: u32,
    pub selected_stream_index: u32,
    pub target_subsong: u32,
    pub target_layer: u32,
    pub channels_per_layer: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XvagInspection {
    pub header: XvagHeader,
    pub codec_info: XvagCodecInfo,
}

pub fn inspect_xvag(bytes: &[u8]) -> Result<XvagInspection, String> {
    let header = parse_xvag_header(bytes)?;
    let codec_info = parse_xvag_codec_info(bytes, &header)?;
    Ok(XvagInspection { header, codec_info })
}

pub fn parse_xvag_codec_info(bytes: &[u8], header: &XvagHeader) -> Result<XvagCodecInfo, String> {
    match header.codec {
        XvagCodec::PsAdpcm => Ok(XvagCodecInfo::PsAdpcm),
        XvagCodec::PsAdpcmExtended => Ok(XvagCodecInfo::PsAdpcmExtended),
        XvagCodec::Mpeg => {
            let (offset, size) =
                find_chunk(bytes, XVAG_FIRST_CHUNK_OFFSET, b"mpin", header.big_endian)
                    .ok_or_else(|| "XVAG MPEG codec requires mpin chunk".to_owned())?;
            if size < 0x38 {
                return Err(format!("XVAG mpin chunk too small: 0x{size:x}"));
            }
            let info = XvagMpegInfo {
                mpeg_version: read_u32(bytes, offset, header.big_endian)?,
                layer: read_u32(bytes, offset + 0x04, header.big_endian)?,
                bit_rate: read_u32(bytes, offset + 0x08, header.big_endian)?,
                sample_rate_hz: read_u32(bytes, offset + 0x0c, header.big_endian)?,
                stream_version: read_u32(bytes, offset + 0x10, header.big_endian)?,
                channels_per_stream: read_u32(bytes, offset + 0x14, header.big_endian)?,
                channels_total_or_stream: read_u32(bytes, offset + 0x18, header.big_endian)?,
                fixed_frame_size: read_u32(bytes, offset + 0x1c, header.big_endian)?,
                encoder_delay: read_u32(bytes, offset + 0x20, header.big_endian)?,
                num_samples: read_u32(bytes, offset + 0x24, header.big_endian)?,
                data_size: read_u32(bytes, offset + 0x34, header.big_endian)?,
            };
            if info.fixed_frame_size == 0 {
                return Err("XVAG mpin fixed frame size must be non-zero".to_owned());
            }
            Ok(XvagCodecInfo::Mpeg(info))
        }
        XvagCodec::Atrac9 => {
            let (offset, size) =
                find_chunk(bytes, XVAG_FIRST_CHUNK_OFFSET, b"a9in", header.big_endian)
                    .ok_or_else(|| "XVAG ATRAC9 codec requires a9in chunk".to_owned())?;
            if size < 0x18 {
                return Err(format!("XVAG a9in chunk too small: 0x{size:x}"));
            }
            let config = bytes
                .get(offset + 0x08..offset + 0x0c)
                .ok_or_else(|| "XVAG a9in config data truncated".to_owned())?;
            let info = XvagAtrac9Info {
                frame_size: read_u32(bytes, offset, header.big_endian)?,
                samples_per_frame: read_u32(bytes, offset + 0x04, header.big_endian)?,
                config_data_be: u32::from_be_bytes([config[0], config[1], config[2], config[3]]),
                fact_num_samples: read_u32(bytes, offset + 0x0c, header.big_endian)?,
                decoder_delay: read_u32(bytes, offset + 0x10, header.big_endian)?,
                encoder_delay: read_u32(bytes, offset + 0x14, header.big_endian)?,
            };
            if info.frame_size == 0 || info.samples_per_frame == 0 {
                return Err(format!(
                    "XVAG a9in invalid frame contract: frame_size={} samples_per_frame={}",
                    info.frame_size, info.samples_per_frame
                ));
            }
            Ok(XvagCodecInfo::Atrac9(info))
        }
        XvagCodec::Unknown(tag) => Ok(XvagCodecInfo::Unknown { tag }),
    }
}

pub fn xvag_demux_descriptor(
    bytes: &[u8],
    target_subsong: u32,
    target_layer: u32,
) -> Result<XvagDemuxDescriptor, String> {
    let inspection = inspect_xvag(bytes)?;
    let header = &inspection.header;
    if target_subsong == 0 || target_subsong > header.subsongs {
        return Err(format!(
            "XVAG target subsong out of range: {target_subsong} / {}",
            header.subsongs
        ));
    }
    if target_layer == 0 || target_layer > header.layers {
        return Err(format!(
            "XVAG target layer out of range: {target_layer} / {}",
            header.layers
        ));
    }
    if header.channels % header.layers != 0 {
        return Err(format!(
            "XVAG channels/layers are not evenly divisible: channels={} layers={}",
            header.channels, header.layers
        ));
    }
    if matches!(
        header.codec,
        XvagCodec::PsAdpcm | XvagCodec::PsAdpcmExtended
    ) {
        if header.subsongs > 1 && header.layers > 1 {
            return Err("XVAG PS-ADPCM cannot combine multiple subsongs and layers".to_owned());
        }
        if header.subsongs > 1 && header.channels > 1 {
            return Err("XVAG PS-ADPCM multi-subsong layout requires mono streams".to_owned());
        }
        if header.layers > 1 && header.layers != header.channels {
            return Err(format!(
                "XVAG PS-ADPCM layered layout requires layers == channels ({} != {})",
                header.layers, header.channels
            ));
        }
    }

    let codec_frame_bytes = match inspection.codec_info {
        XvagCodecInfo::PsAdpcm | XvagCodecInfo::PsAdpcmExtended => PS_ADPCM_FRAME_BYTES as u32,
        XvagCodecInfo::Mpeg(info) => info.fixed_frame_size,
        XvagCodecInfo::Atrac9(info) => info.frame_size,
        XvagCodecInfo::Unknown { tag } => {
            return Err(format!(
                "XVAG codec 0x{tag:02x} has no demux frame contract"
            ));
        }
    };
    let interleave_block_bytes = codec_frame_bytes
        .checked_mul(header.interleave_factor)
        .ok_or_else(|| "XVAG demux interleave size overflow".to_owned())?;
    let stream_count = header
        .subsongs
        .checked_mul(header.layers)
        .ok_or_else(|| "XVAG stream count overflow".to_owned())?;
    let selected_stream_index = (target_subsong - 1)
        .checked_mul(header.layers)
        .and_then(|base| base.checked_add(target_layer - 1))
        .ok_or_else(|| "XVAG selected stream index overflow".to_owned())?;

    Ok(XvagDemuxDescriptor {
        codec: header.codec,
        data_offset: header.start_offset,
        data_size: header.data_size,
        codec_frame_bytes,
        interleave_block_bytes,
        stream_count,
        selected_stream_index,
        target_subsong,
        target_layer,
        channels_per_layer: header.channels / header.layers,
    })
}

pub fn parse_xvag_header(bytes: &[u8]) -> Result<XvagHeader, String> {
    if bytes.get(0..4) != Some(&XVAG_MAGIC) {
        return Err("XVAG magic mismatch".to_owned());
    }
    if bytes.len() < XVAG_FIRST_CHUNK_OFFSET {
        return Err(format!(
            "XVAG header truncated: {} bytes, expected at least {}",
            bytes.len(),
            XVAG_FIRST_CHUNK_OFFSET
        ));
    }

    let big_endian = bytes[0x08] & 0x01 != 0;
    let start_offset = read_u32(bytes, 0x04, big_endian)?;
    let version_flag = bytes[0x0b];
    let (fmat_offset, fmat_size) = find_chunk(bytes, XVAG_FIRST_CHUNK_OFFSET, b"fmat", big_endian)
        .ok_or_else(|| "XVAG fmat chunk not found".to_owned())?;
    if fmat_size < 0x1c {
        return Err(format!("XVAG fmat chunk too small: 0x{fmat_size:x}"));
    }

    let channels = read_u32(bytes, fmat_offset, big_endian)?;
    let codec = XvagCodec::from_tag(read_u32(bytes, fmat_offset + 0x04, big_endian)?);
    let num_samples = read_u32(bytes, fmat_offset + 0x08, big_endian)?;
    let repeated_samples = read_u32(bytes, fmat_offset + 0x0c, big_endian)?;
    if repeated_samples != num_samples {
        return Err(format!(
            "XVAG fmat sample count mismatch: {num_samples} != {repeated_samples}"
        ));
    }
    let interleave_factor = read_u32(bytes, fmat_offset + 0x10, big_endian)?;
    let sample_rate_hz = read_u32(bytes, fmat_offset + 0x14, big_endian)?;
    let data_size = read_u32(bytes, fmat_offset + 0x18, big_endian)?;
    let (subsongs, layers) = if fmat_size >= 0x24 {
        (
            read_u32(bytes, fmat_offset + 0x1c, big_endian)?,
            read_u32(bytes, fmat_offset + 0x20, big_endian)?,
        )
    } else {
        (1, 1)
    };

    if channels == 0 || channels > 32 {
        return Err(format!("XVAG channel count out of range: {channels}"));
    }
    if !(8_000..=384_000).contains(&sample_rate_hz) {
        return Err(format!("XVAG sample rate out of range: {sample_rate_hz}"));
    }
    if interleave_factor == 0 {
        return Err("XVAG interleave factor must be non-zero".to_owned());
    }
    if subsongs == 0 || layers == 0 {
        return Err(format!(
            "XVAG invalid subsong/layer counts: subsongs={subsongs} layers={layers}"
        ));
    }
    let data_end = usize::try_from(start_offset)
        .ok()
        .and_then(|start| start.checked_add(usize::try_from(data_size).ok()?))
        .ok_or_else(|| "XVAG data range overflow".to_owned())?;
    if data_end > bytes.len() {
        return Err(format!(
            "XVAG data truncated: end=0x{data_end:x} file=0x{:x}",
            bytes.len()
        ));
    }

    Ok(XvagHeader {
        big_endian,
        version_flag,
        start_offset,
        channels,
        codec,
        num_samples,
        interleave_factor,
        sample_rate_hz,
        data_size,
        subsongs,
        layers,
    })
}

pub fn decode_xvag_ps_adpcm(bytes: &[u8]) -> Result<DecodedPcmF32, String> {
    let header = parse_xvag_header(bytes)?;
    if header.codec != XvagCodec::PsAdpcm {
        return Err(format!(
            "XVAG codec 0x{:02x} is not supported by the native PS-ADPCM decoder",
            header.codec.tag()
        ));
    }
    if header.subsongs != 1 || header.layers != 1 {
        return Err(format!(
            "XVAG PS-ADPCM decoder currently requires one subsong/layer, got subsongs={} layers={}",
            header.subsongs, header.layers
        ));
    }

    let channels = usize::try_from(header.channels).map_err(|_| "XVAG channels exceed usize")?;
    let target_frames =
        usize::try_from(header.num_samples).map_err(|_| "XVAG sample count exceeds usize")?;
    let factor =
        usize::try_from(header.interleave_factor).map_err(|_| "XVAG factor exceeds usize")?;
    let data_start =
        usize::try_from(header.start_offset).map_err(|_| "XVAG data offset exceeds usize")?;
    let data_end = data_start
        .checked_add(usize::try_from(header.data_size).map_err(|_| "XVAG data size exceeds usize")?)
        .ok_or_else(|| "XVAG data range overflow".to_owned())?;
    let data = bytes
        .get(data_start..data_end)
        .ok_or_else(|| "XVAG data range is outside source".to_owned())?;

    let mut channel_pcm = vec![Vec::<i16>::with_capacity(target_frames); channels];
    let mut histories = vec![(0_i32, 0_i32); channels];
    let block_bytes = PS_ADPCM_FRAME_BYTES
        .checked_mul(factor)
        .ok_or_else(|| "XVAG interleave block size overflow".to_owned())?;
    let round_bytes = block_bytes
        .checked_mul(channels)
        .ok_or_else(|| "XVAG interleave round size overflow".to_owned())?;
    let mut round_offset = 0usize;

    while channel_pcm
        .iter()
        .any(|channel| channel.len() < target_frames)
    {
        let round_end = round_offset
            .checked_add(round_bytes)
            .ok_or_else(|| "XVAG interleave cursor overflow".to_owned())?;
        if round_end > data.len() {
            return Err(format!(
                "XVAG PS-ADPCM ended before declared sample count: decoded={} target={target_frames}",
                channel_pcm.iter().map(Vec::len).min().unwrap_or(0)
            ));
        }
        for channel_index in 0..channels {
            let channel_block = round_offset + channel_index * block_bytes;
            for frame_index in 0..factor {
                if channel_pcm[channel_index].len() >= target_frames {
                    break;
                }
                let frame_offset = channel_block + frame_index * PS_ADPCM_FRAME_BYTES;
                let frame = &data[frame_offset..frame_offset + PS_ADPCM_FRAME_BYTES];
                let decoded = decode_ps_adpcm_frame(frame, &mut histories[channel_index])?;
                let remaining = target_frames - channel_pcm[channel_index].len();
                channel_pcm[channel_index]
                    .extend_from_slice(&decoded[..remaining.min(PS_ADPCM_SAMPLES_PER_FRAME)]);
            }
        }
        round_offset = round_end;
    }

    let sample_capacity = target_frames
        .checked_mul(channels)
        .ok_or_else(|| "XVAG interleaved output size overflow".to_owned())?;
    let mut samples = Vec::with_capacity(sample_capacity);
    for frame in 0..target_frames {
        for channel in &channel_pcm {
            samples.push(f32::from(channel[frame]) / 32768.0);
        }
    }

    Ok(DecodedPcmF32 {
        sample_rate_hz: header.sample_rate_hz,
        channels: u16::try_from(header.channels).map_err(|_| "XVAG channel count exceeds u16")?,
        samples,
    })
}

pub fn encode_xvag_ps_adpcm(
    sample_rate_hz: u32,
    channels: u16,
    samples: &[f32],
) -> Result<Vec<u8>, String> {
    if channels == 0 || channels > 32 {
        return Err(format!("XVAG channel count out of range: {channels}"));
    }
    if !(8_000..=384_000).contains(&sample_rate_hz) {
        return Err(format!("XVAG sample rate out of range: {sample_rate_hz}"));
    }
    let channel_count = usize::from(channels);
    if samples.len() % channel_count != 0 {
        return Err(format!(
            "XVAG interleaved sample count {} is not divisible by channels {channels}",
            samples.len()
        ));
    }
    if samples.iter().any(|sample| !sample.is_finite()) {
        return Err("XVAG source PCM contains non-finite samples".to_owned());
    }

    let frames_per_channel = samples.len() / channel_count;
    let frame_count = frames_per_channel.div_ceil(PS_ADPCM_SAMPLES_PER_FRAME);
    let encoded_frames_per_channel = frame_count
        .checked_add(1)
        .ok_or_else(|| "XVAG frame count overflow".to_owned())?; // final silent frame avoids false loop heuristics
    let data_size = encoded_frames_per_channel
        .checked_mul(channel_count)
        .and_then(|count| count.checked_mul(PS_ADPCM_FRAME_BYTES))
        .ok_or_else(|| "XVAG data size overflow".to_owned())?;
    let data_size_u32 = u32::try_from(data_size).map_err(|_| "XVAG data exceeds u32".to_owned())?;
    let num_samples_u32 = u32::try_from(frames_per_channel)
        .map_err(|_| "XVAG sample count exceeds u32".to_owned())?;

    let mut out = vec![0_u8; XVAG_WRITER_DATA_OFFSET + data_size];
    out[0..4].copy_from_slice(&XVAG_MAGIC);
    write_u32(&mut out, 0x04, XVAG_WRITER_DATA_OFFSET as u32, false)?;
    out[0x08] = 0; // little-endian XVAG
    out[0x0b] = 0x61;

    out[0x20..0x24].copy_from_slice(b"fmat");
    write_u32(&mut out, 0x24, XVAG_FMAT_SIZE_V61 as u32, false)?;
    let fmat = 0x28;
    write_u32(&mut out, fmat, u32::from(channels), false)?;
    write_u32(&mut out, fmat + 0x04, XVAG_CODEC_PS_ADPCM, false)?;
    write_u32(&mut out, fmat + 0x08, num_samples_u32, false)?;
    write_u32(&mut out, fmat + 0x0c, num_samples_u32, false)?;
    write_u32(&mut out, fmat + 0x10, 1, false)?;
    write_u32(&mut out, fmat + 0x14, sample_rate_hz, false)?;
    write_u32(&mut out, fmat + 0x18, data_size_u32, false)?;
    write_u32(&mut out, fmat + 0x1c, 1, false)?;
    write_u32(&mut out, fmat + 0x20, 1, false)?;
    out[0x4c..0x50].copy_from_slice(b"0000");

    let mut histories = vec![(0_i32, 0_i32); channel_count];
    let mut cursor = XVAG_WRITER_DATA_OFFSET;
    for encoded_frame in 0..encoded_frames_per_channel {
        for channel in 0..channel_count {
            if encoded_frame == frame_count {
                out[cursor..cursor + PS_ADPCM_FRAME_BYTES].fill(0);
            } else {
                let mut pcm = [0_i16; PS_ADPCM_SAMPLES_PER_FRAME];
                for (sample_in_frame, target) in pcm.iter_mut().enumerate() {
                    let source_frame = encoded_frame * PS_ADPCM_SAMPLES_PER_FRAME + sample_in_frame;
                    if source_frame < frames_per_channel {
                        let sample =
                            samples[source_frame * channel_count + channel].clamp(-1.0, 1.0);
                        *target = if sample >= 1.0 {
                            i16::MAX
                        } else if sample <= -1.0 {
                            i16::MIN
                        } else {
                            (sample * 32768.0).round() as i16
                        };
                    }
                }
                let frame = encode_ps_adpcm_frame(&pcm, &mut histories[channel]);
                out[cursor..cursor + PS_ADPCM_FRAME_BYTES].copy_from_slice(&frame);
            }
            cursor += PS_ADPCM_FRAME_BYTES;
        }
    }

    Ok(out)
}

fn decode_ps_adpcm_frame(frame: &[u8], history: &mut (i32, i32)) -> Result<[i16; 28], String> {
    if frame.len() != PS_ADPCM_FRAME_BYTES {
        return Err(format!(
            "PS-ADPCM frame size is {}, expected 16",
            frame.len()
        ));
    }
    let predictor = usize::from(frame[0] >> 4);
    let shift = u32::from(frame[0] & 0x0f);
    let Some(&(coef0, coef1)) = PS_FILTERS.get(predictor) else {
        return Err(format!("PS-ADPCM predictor out of range: {predictor}"));
    };
    if shift > 12 {
        return Err(format!("PS-ADPCM shift out of range: {shift}"));
    }

    let mut output = [0_i16; PS_ADPCM_SAMPLES_PER_FRAME];
    for sample_index in 0..PS_ADPCM_SAMPLES_PER_FRAME {
        let packed = frame[2 + sample_index / 2];
        let nibble_u8 = if sample_index & 1 == 0 {
            packed & 0x0f
        } else {
            packed >> 4
        };
        let nibble = if nibble_u8 & 0x08 != 0 {
            i32::from(nibble_u8) - 16
        } else {
            i32::from(nibble_u8)
        };
        let residual = (nibble << 12) >> shift;
        let predicted = (history.0 * coef0 + history.1 * coef1 + 32) >> 6;
        let decoded = (residual + predicted).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
        output[sample_index] = decoded as i16;
        history.1 = history.0;
        history.0 = decoded;
    }
    Ok(output)
}

fn encode_ps_adpcm_frame(pcm: &[i16; 28], history: &mut (i32, i32)) -> [u8; 16] {
    let mut best_error = f64::INFINITY;
    let mut best_predictor = 0usize;
    let mut best_shift = 0u32;
    let mut best_nibbles = [0_i8; PS_ADPCM_SAMPLES_PER_FRAME];
    let mut best_history = *history;

    for (predictor, &(coef0, coef1)) in PS_FILTERS.iter().enumerate() {
        for shift in 0..=12u32 {
            let scale = 1_i32 << (12 - shift);
            let mut candidate_history = *history;
            let mut candidate_nibbles = [0_i8; PS_ADPCM_SAMPLES_PER_FRAME];
            let mut error = 0.0_f64;

            for (index, &target_i16) in pcm.iter().enumerate() {
                let target = i32::from(target_i16);
                let predicted =
                    (candidate_history.0 * coef0 + candidate_history.1 * coef1 + 32) >> 6;
                let residual = target - predicted;
                let nibble = ((f64::from(residual) / f64::from(scale)).round() as i32).clamp(-8, 7);
                let decoded =
                    (predicted + nibble * scale).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
                let delta = f64::from(target - decoded);
                error += delta * delta;
                candidate_nibbles[index] = nibble as i8;
                candidate_history.1 = candidate_history.0;
                candidate_history.0 = decoded;
            }

            if error < best_error {
                best_error = error;
                best_predictor = predictor;
                best_shift = shift;
                best_nibbles = candidate_nibbles;
                best_history = candidate_history;
            }
        }
    }

    *history = best_history;
    let mut frame = [0_u8; PS_ADPCM_FRAME_BYTES];
    frame[0] = ((best_predictor as u8) << 4) | best_shift as u8;
    frame[1] = 0;
    for pair in 0..14 {
        let lo = (i32::from(best_nibbles[pair * 2]) & 0x0f) as u8;
        let hi = (i32::from(best_nibbles[pair * 2 + 1]) & 0x0f) as u8;
        frame[2 + pair] = lo | (hi << 4);
    }
    frame
}

fn find_chunk(
    bytes: &[u8],
    start_offset: usize,
    wanted: &[u8; 4],
    big_endian_size: bool,
) -> Option<(usize, usize)> {
    let mut cursor = start_offset;
    while cursor.checked_add(8)? <= bytes.len() {
        let chunk_id = bytes.get(cursor..cursor + 4)?;
        let chunk_size =
            usize::try_from(read_u32(bytes, cursor + 4, big_endian_size).ok()?).ok()?;
        let payload = cursor.checked_add(8)?;
        let end = payload.checked_add(chunk_size)?;
        if end > bytes.len() {
            return None;
        }
        if chunk_id == wanted {
            return Some((payload, chunk_size));
        }
        if chunk_size == 0 {
            return None;
        }
        cursor = end.checked_add(chunk_size & 1)?;
    }
    None
}

fn read_u32(bytes: &[u8], offset: usize, big_endian: bool) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset.checked_add(4).ok_or("XVAG offset overflow")?)
        .ok_or_else(|| format!("XVAG truncated at u32 offset 0x{offset:x}"))?;
    Ok(if big_endian {
        u32::from_be_bytes([value[0], value[1], value[2], value[3]])
    } else {
        u32::from_le_bytes([value[0], value[1], value[2], value[3]])
    })
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32, big_endian: bool) -> Result<(), String> {
    let target = bytes
        .get_mut(offset..offset.checked_add(4).ok_or("XVAG output offset overflow")?)
        .ok_or_else(|| format!("XVAG output truncated at u32 offset 0x{offset:x}"))?;
    let encoded = if big_endian {
        value.to_be_bytes()
    } else {
        value.to_le_bytes()
    };
    target.copy_from_slice(&encoded);
    Ok(())
}

#[cfg(test)]
mod tests {
    include!("tests/tests.rs");
}
