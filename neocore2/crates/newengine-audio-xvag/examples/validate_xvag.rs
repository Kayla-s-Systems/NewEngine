use std::{env, fs, path::PathBuf};

use newengine_audio_xvag::{XvagCodec, XvagCodecInfo};

fn main() {
    if let Err(error) = run() {
        eprintln!("validate_xvag failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| "usage: validate_xvag <file.xvag>".to_owned())?;
    let bytes =
        fs::read(&path).map_err(|error| format!("read '{}' failed: {error}", path.display()))?;
    let inspection = newengine_audio_xvag::inspect_xvag(&bytes)?;
    let header = &inspection.header;
    let demux = newengine_audio_xvag::xvag_demux_descriptor(&bytes, 1, 1)?;
    println!(
        "XVAG endian={} version=0x{:02x} codec=0x{:02x} channels={} rate={} samples={} data_offset=0x{:x} data_size={} factor={} subsongs={} layers={} frame_bytes={} interleave_bytes={}",
        if header.big_endian { "be" } else { "le" },
        header.version_flag,
        header.codec.tag(),
        header.channels,
        header.sample_rate_hz,
        header.num_samples,
        header.start_offset,
        header.data_size,
        header.interleave_factor,
        header.subsongs,
        header.layers,
        demux.codec_frame_bytes,
        demux.interleave_block_bytes,
    );
    match inspection.codec_info {
        XvagCodecInfo::Atrac9(info) => println!(
            "ATRAC9 frame_size={} samples_per_frame={} config_be=0x{:08x} fact_samples={} decoder_delay={} encoder_delay={}",
            info.frame_size,
            info.samples_per_frame,
            info.config_data_be,
            info.fact_num_samples,
            info.decoder_delay,
            info.encoder_delay,
        ),
        XvagCodecInfo::Mpeg(info) => println!(
            "MPEG version={} layer={} bitrate={} sample_rate={} frame_size={} encoder_delay={} samples={} data_size={}",
            info.mpeg_version,
            info.layer,
            info.bit_rate,
            info.sample_rate_hz,
            info.fixed_frame_size,
            info.encoder_delay,
            info.num_samples,
            info.data_size,
        ),
        XvagCodecInfo::PsAdpcm | XvagCodecInfo::PsAdpcmExtended => {}
        XvagCodecInfo::Unknown { tag } => println!("unknown codec tag=0x{tag:02x}"),
    }
    if header.codec == XvagCodec::PsAdpcm {
        let decoded = newengine_audio_xvag::decode_xvag_ps_adpcm(&bytes)?;
        println!("decoded_samples={}", decoded.samples.len());
    }
    Ok(())
}
