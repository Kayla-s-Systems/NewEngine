use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use super::AudioMetaV1;

#[inline]
pub fn probe_symphonia(
    bytes: &[u8],
    ext_hint: Option<&str>,
    container: &'static str,
) -> Result<AudioMetaV1, String> {
    let mut hint = Hint::new();
    if let Some(ext) = ext_hint {
        let e = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        if !e.is_empty() {
            hint.with_extension(&e);
        }
    }

    let cursor = std::io::Cursor::new(bytes.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), MediaSourceStreamOptions::default());

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("probe failed: {e}"))?;

    let format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| "no default track".to_string())?;

    let params = &track.codec_params;

    let sample_rate = params.sample_rate.unwrap_or(0);
    let channels = params.channels.map(|c| c.count() as u16).unwrap_or(0);
    let bits_per_sample = params.bits_per_sample.unwrap_or(0) as u16;
    let frames = params.n_frames.unwrap_or(0);

    let duration_sec = match (params.n_frames, params.time_base) {
        (Some(nf), Some(tb)) if tb.denom > 0 => (nf as f64) * (tb.numer as f64) / (tb.denom as f64),
        _ => 0.0,
    };

    let codec = format!("{:?}", params.codec);

    Ok(AudioMetaV1 {
        container,
        codec,
        sample_rate,
        channels,
        bits_per_sample,
        frames,
        duration_sec,
    })
}
