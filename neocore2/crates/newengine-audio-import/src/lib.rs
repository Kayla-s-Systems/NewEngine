#![forbid(unsafe_op_in_unsafe_fn)]

//! Research/import-side audio contracts.
//!
//! The core invariant is intentionally explicit:
//!
//! `container != codec`
//!
//! A source container may carry different codecs across games, platform revisions,
//! banks, or individual tracks. Runtime playback therefore never infers codec
//! identity from a filename extension alone.

mod pcm;
mod xvag;

pub use pcm::{CanonicalPcmBuffer, CanonicalPcmLoopRegion};

use serde::{Deserialize, Serialize};

pub const NORTHSTAR_AUDIO_IMPORT_CONTRACT: &str = "northstar.audio.import.v1";
pub const NORTHSTAR_AUDIO_CANONICAL_PCM: &str = "northstar.audio.pcm.f32.interleaved.v1";
pub const NORTHSTAR_AUDIO_NATIVE_TARGET: &str = "nef8.audio_clip.v1";
pub const YSNCD_LEGACY_POLICY: &str = "legacy_read_compat_only";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportedAudioContainer {
    Xvag,
    Vag,
    SonyBnk,
    Bink,
    Wwise,
    Riff,
    Raw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportedAudioCodec {
    Atrac9,
    PsAdpcm,
    Hevag,
    Mpeg,
    BinkRdft,
    BinkDct,
    Vorbis,
    Opus,
    Pcm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeConfidence {
    Magic,
    MagicAndHeader,
    ExtensionHint,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedAudioStream {
    pub track_index: u32,
    pub codec: Option<ImportedAudioCodec>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u16>,
}

impl ImportedAudioStream {
    pub const fn unknown(track_index: u32) -> Self {
        Self {
            track_index,
            codec: None,
            sample_rate_hz: None,
            channels: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedAudioProbe {
    pub container: ImportedAudioContainer,
    pub revision: Option<String>,
    pub streams: Vec<ImportedAudioStream>,
    pub confidence: ProbeConfidence,
}

impl ImportedAudioProbe {
    #[inline]
    pub fn primary_codec(&self) -> Option<ImportedAudioCodec> {
        self.streams.first().and_then(|stream| stream.codec)
    }
}

/// Probe an imported audio or multimedia source without conflating its container
/// with the codec carried by one of its streams.
///
/// This function is intentionally conservative. When the outer container is
/// known but its codec cannot be proven from the bounded header, `codec` remains
/// `None` rather than being guessed from the extension.
pub fn probe_imported_audio(source_name: Option<&str>, bytes: &[u8]) -> Option<ImportedAudioProbe> {
    if let Some(probe) = probe_bink(bytes) {
        return Some(probe);
    }
    if let Some(probe) = xvag::probe_xvag(bytes) {
        return Some(probe);
    }
    if bytes.starts_with(b"VAGp") {
        return Some(ImportedAudioProbe {
            container: ImportedAudioContainer::Vag,
            revision: None,
            // VAG containers are observed with both classic PS ADPCM and HEVAG.
            // Do not guess which one is present from magic alone.
            streams: vec![ImportedAudioStream::unknown(0)],
            confidence: ProbeConfidence::Magic,
        });
    }
    if bytes.starts_with(b"BKHD") {
        return Some(ImportedAudioProbe {
            container: ImportedAudioContainer::Wwise,
            revision: None,
            // A Wwise soundbank is a bank/container. Individual media streams are
            // resolved/demuxed separately and may use different codecs.
            streams: Vec::new(),
            confidence: ProbeConfidence::Magic,
        });
    }
    if let Some(probe) = probe_riff(source_name, bytes) {
        return Some(probe);
    }
    if let Some(codec) = probe_ogg_codec(bytes) {
        return Some(ImportedAudioProbe {
            container: ImportedAudioContainer::Raw,
            revision: None,
            streams: vec![ImportedAudioStream {
                track_index: 0,
                codec: Some(codec),
                sample_rate_hz: None,
                channels: None,
            }],
            confidence: ProbeConfidence::Magic,
        });
    }

    match extension(source_name).as_deref() {
        Some("pcm") => Some(ImportedAudioProbe {
            container: ImportedAudioContainer::Raw,
            revision: None,
            streams: vec![ImportedAudioStream {
                track_index: 0,
                codec: Some(ImportedAudioCodec::Pcm),
                sample_rate_hz: None,
                channels: None,
            }],
            confidence: ProbeConfidence::ExtensionHint,
        }),
        Some("raw") => Some(ImportedAudioProbe {
            container: ImportedAudioContainer::Raw,
            revision: None,
            streams: vec![ImportedAudioStream::unknown(0)],
            confidence: ProbeConfidence::ExtensionHint,
        }),
        _ => None,
    }
}

fn probe_bink(bytes: &[u8]) -> Option<ImportedAudioProbe> {
    // Standalone Bink Audio container used by FFmpeg's `binka` demuxer.
    if bytes.get(0..4) == Some(b"1FCB") && matches!(bytes.get(4), Some(1 | 2)) {
        let channels = bytes
            .get(5)
            .copied()
            .map(u16::from)
            .filter(|value| *value != 0);
        let sample_rate_hz = read_u16_le(bytes, 6)
            .map(u32::from)
            .filter(|value| *value != 0);
        return Some(ImportedAudioProbe {
            container: ImportedAudioContainer::Bink,
            revision: bytes.get(4).map(|value| format!("binka-v{value}")),
            streams: vec![ImportedAudioStream {
                track_index: 0,
                codec: Some(ImportedAudioCodec::BinkDct),
                sample_rate_hz,
                channels,
            }],
            confidence: ProbeConfidence::MagicAndHeader,
        });
    }

    if !is_bink_video_signature(bytes) {
        return None;
    }

    let revision = std::str::from_utf8(bytes.get(0..4)?)
        .ok()
        .map(ToOwned::to_owned);
    let num_audio_tracks = read_u32_le(bytes, 40)?;
    if num_audio_tracks > 256 {
        return None;
    }

    let signature = bytes.get(0..3)?;
    let revision_byte = *bytes.get(3)?;
    let has_new_field = (signature == b"BIK" && revision_byte == b'k')
        || (signature == b"KB2" && matches!(revision_byte, b'i' | b'j' | b'k'));

    let mut cursor = 44usize;
    if has_new_field {
        cursor = cursor.checked_add(4)?;
    }
    cursor = cursor.checked_add(usize::try_from(num_audio_tracks).ok()?.checked_mul(4)?)?;

    let mut streams = Vec::with_capacity(usize::try_from(num_audio_tracks).ok()?);
    for track_index in 0..num_audio_tracks {
        let sample_rate_hz = read_u16_le(bytes, cursor)
            .map(u32::from)
            .filter(|value| *value != 0);
        let flags = read_u16_le(bytes, cursor.checked_add(2)?)?;
        let codec = if flags & 0x1000 != 0 {
            ImportedAudioCodec::BinkDct
        } else {
            ImportedAudioCodec::BinkRdft
        };
        let channels = Some(if flags & 0x2000 != 0 { 2 } else { 1 });
        streams.push(ImportedAudioStream {
            track_index,
            codec: Some(codec),
            sample_rate_hz,
            channels,
        });
        cursor = cursor.checked_add(4)?;
    }

    Some(ImportedAudioProbe {
        container: ImportedAudioContainer::Bink,
        revision,
        streams,
        confidence: ProbeConfidence::MagicAndHeader,
    })
}

fn is_bink_video_signature(bytes: &[u8]) -> bool {
    let Some(tag) = bytes.get(0..4) else {
        return false;
    };
    let revision = tag[3];
    (tag.get(0..3) == Some(b"BIK") && matches!(revision, b'b' | b'f' | b'g' | b'h' | b'i' | b'k'))
        || (tag.get(0..3) == Some(b"KB2")
            && matches!(
                revision,
                b'a' | b'd' | b'f' | b'g' | b'h' | b'i' | b'j' | b'k'
            ))
}

fn probe_riff(source_name: Option<&str>, bytes: &[u8]) -> Option<ImportedAudioProbe> {
    if bytes.get(0..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return None;
    }

    let container = if extension(source_name).as_deref() == Some("wem") {
        ImportedAudioContainer::Wwise
    } else {
        ImportedAudioContainer::Riff
    };
    let mut codec = None;
    let mut channels = None;
    let mut sample_rate_hz = None;

    let mut cursor = 12usize;
    while cursor.checked_add(8)? <= bytes.len() {
        let chunk_id = bytes.get(cursor..cursor + 4)?;
        let chunk_len = usize::try_from(read_u32_le(bytes, cursor + 4)?).ok()?;
        let payload = cursor.checked_add(8)?;
        let end = payload.checked_add(chunk_len)?;
        if end > bytes.len() {
            break;
        }
        if chunk_id == b"fmt " && chunk_len >= 16 {
            let format_tag = read_u16_le(bytes, payload)?;
            channels = read_u16_le(bytes, payload + 2).filter(|value| *value != 0);
            sample_rate_hz = read_u32_le(bytes, payload + 4).filter(|value| *value != 0);
            codec = match format_tag {
                0x0001 | 0x0003 => Some(ImportedAudioCodec::Pcm),
                _ => None,
            };
            break;
        }
        cursor = end.checked_add(chunk_len & 1)?;
    }

    Some(ImportedAudioProbe {
        container,
        revision: None,
        streams: vec![ImportedAudioStream {
            track_index: 0,
            codec,
            sample_rate_hz,
            channels,
        }],
        confidence: ProbeConfidence::MagicAndHeader,
    })
}

fn probe_ogg_codec(bytes: &[u8]) -> Option<ImportedAudioCodec> {
    if bytes.get(0..4) != Some(b"OggS") {
        return None;
    }
    let probe_window = bytes.get(..bytes.len().min(256))?;
    if probe_window
        .windows(b"OpusHead".len())
        .any(|window| window == b"OpusHead")
    {
        return Some(ImportedAudioCodec::Opus);
    }
    if probe_window
        .windows(b"\x01vorbis".len())
        .any(|window| window == b"\x01vorbis")
    {
        return Some(ImportedAudioCodec::Vorbis);
    }
    None
}

fn extension(source_name: Option<&str>) -> Option<String> {
    let source_name = source_name?.trim();
    let (_, extension) = source_name.rsplit_once('.')?;
    let extension = extension.trim().to_ascii_lowercase();
    (!extension.is_empty()).then_some(extension)
}

#[inline]
fn read_u16_le(bytes: &[u8], offset: usize) -> Option<u16> {
    let value = bytes.get(offset..offset.checked_add(2)?)?;
    Some(u16::from_le_bytes([value[0], value[1]]))
}

#[inline]
fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let value = bytes.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xvag_probe_does_not_guess_codec_from_container() {
        let probe = probe_imported_audio(Some("voice.xvag"), b"XVAG\0\0\0\0").expect("xvag");
        assert_eq!(probe.container, ImportedAudioContainer::Xvag);
        assert_eq!(probe.primary_codec(), None);
    }

    #[test]
    fn wwise_bank_probe_keeps_media_codec_unresolved() {
        let probe = probe_imported_audio(Some("Init.bnk"), b"BKHD\x08\0\0\0").expect("wwise");
        assert_eq!(probe.container, ImportedAudioContainer::Wwise);
        assert!(probe.streams.is_empty());
    }

    #[test]
    fn riff_pcm_probe_extracts_codec_and_format() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&36_u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&48_000_u32.to_le_bytes());
        bytes.extend_from_slice(&192_000_u32.to_le_bytes());
        bytes.extend_from_slice(&4_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&0_u32.to_le_bytes());

        let probe = probe_imported_audio(Some("ambience.wav"), &bytes).expect("riff");
        assert_eq!(probe.container, ImportedAudioContainer::Riff);
        assert_eq!(probe.primary_codec(), Some(ImportedAudioCodec::Pcm));
        assert_eq!(probe.streams[0].channels, Some(2));
        assert_eq!(probe.streams[0].sample_rate_hz, Some(48_000));
    }

    #[test]
    fn wem_is_wwise_container_even_when_riff_wrapped() {
        let mut bytes = vec![0_u8; 44];
        bytes[0..4].copy_from_slice(b"RIFF");
        bytes[8..12].copy_from_slice(b"WAVE");
        bytes[12..16].copy_from_slice(b"fmt ");
        bytes[16..20].copy_from_slice(&16_u32.to_le_bytes());
        bytes[20..22].copy_from_slice(&1_u16.to_le_bytes());
        bytes[22..24].copy_from_slice(&1_u16.to_le_bytes());
        bytes[24..28].copy_from_slice(&44_100_u32.to_le_bytes());
        let probe = probe_imported_audio(Some("12345.wem"), &bytes).expect("wem");
        assert_eq!(probe.container, ImportedAudioContainer::Wwise);
        assert_eq!(probe.primary_codec(), Some(ImportedAudioCodec::Pcm));
    }

    #[test]
    fn ogg_probe_distinguishes_opus_from_vorbis() {
        let mut opus = b"OggS".to_vec();
        opus.extend_from_slice(&[0; 24]);
        opus.extend_from_slice(b"OpusHead");
        assert_eq!(
            probe_imported_audio(Some("dialogue.opus"), &opus)
                .and_then(|probe| probe.primary_codec()),
            Some(ImportedAudioCodec::Opus)
        );

        let mut vorbis = b"OggS".to_vec();
        vorbis.extend_from_slice(&[0; 24]);
        vorbis.extend_from_slice(b"\x01vorbis");
        assert_eq!(
            probe_imported_audio(Some("music.ogg"), &vorbis)
                .and_then(|probe| probe.primary_codec()),
            Some(ImportedAudioCodec::Vorbis)
        );
    }

    #[test]
    fn bink_video_audio_tracks_report_rdft_and_dct_independently() {
        let mut bytes = vec![0_u8; 64];
        bytes[0..4].copy_from_slice(b"BIKi");
        bytes[4..8].copy_from_slice(&56_u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&1_u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&1920_u32.to_le_bytes());
        bytes[24..28].copy_from_slice(&1080_u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&30_u32.to_le_bytes());
        bytes[32..36].copy_from_slice(&1_u32.to_le_bytes());
        bytes[40..44].copy_from_slice(&2_u32.to_le_bytes());
        // max decoded sizes for the two tracks
        bytes[44..48].copy_from_slice(&4096_u32.to_le_bytes());
        bytes[48..52].copy_from_slice(&4096_u32.to_le_bytes());
        // track 0: 48 kHz stereo RDFT
        bytes[52..54].copy_from_slice(&48_000_u16.to_le_bytes());
        bytes[54..56].copy_from_slice(&0x2000_u16.to_le_bytes());
        // track 1: 44.1 kHz mono DCT
        bytes[56..58].copy_from_slice(&44_100_u16.to_le_bytes());
        bytes[58..60].copy_from_slice(&0x1000_u16.to_le_bytes());

        let probe = probe_imported_audio(Some("movie.bik"), &bytes).expect("bink");
        assert_eq!(probe.container, ImportedAudioContainer::Bink);
        assert_eq!(probe.streams.len(), 2);
        assert_eq!(probe.streams[0].codec, Some(ImportedAudioCodec::BinkRdft));
        assert_eq!(probe.streams[0].channels, Some(2));
        assert_eq!(probe.streams[1].codec, Some(ImportedAudioCodec::BinkDct));
        assert_eq!(probe.streams[1].channels, Some(1));
    }

    #[test]
    fn standalone_binka_is_dct_audio() {
        let mut bytes = vec![0_u8; 24];
        bytes[0..4].copy_from_slice(b"1FCB");
        bytes[4] = 1;
        bytes[5] = 2;
        bytes[6..8].copy_from_slice(&48_000_u16.to_le_bytes());
        let probe = probe_imported_audio(Some("sample.binka"), &bytes).expect("binka");
        assert_eq!(probe.container, ImportedAudioContainer::Bink);
        assert_eq!(probe.primary_codec(), Some(ImportedAudioCodec::BinkDct));
        assert_eq!(probe.streams[0].channels, Some(2));
        assert_eq!(probe.streams[0].sample_rate_hz, Some(48_000));
    }

    #[test]
    fn raw_extension_does_not_claim_a_codec() {
        let probe = probe_imported_audio(Some("mystery.raw"), &[1, 2, 3, 4]).expect("raw");
        assert_eq!(probe.container, ImportedAudioContainer::Raw);
        assert_eq!(probe.primary_codec(), None);
    }

    #[test]
    fn new_import_target_is_nef8_and_ysncd_is_legacy_only() {
        assert_eq!(NORTHSTAR_AUDIO_NATIVE_TARGET, "nef8.audio_clip.v1");
        assert_eq!(YSNCD_LEGACY_POLICY, "legacy_read_compat_only");
    }
}
