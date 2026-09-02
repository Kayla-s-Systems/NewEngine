use crate::{
    ImportedAudioCodec, ImportedAudioContainer, ImportedAudioProbe, ImportedAudioStream,
    ProbeConfidence,
};
use newengine_audio_xvag::{parse_xvag_header, XvagCodec};

pub(crate) fn probe_xvag(bytes: &[u8]) -> Option<ImportedAudioProbe> {
    if bytes.get(0..4) != Some(b"XVAG") {
        return None;
    }
    let Ok(header) = parse_xvag_header(bytes) else {
        return Some(ImportedAudioProbe {
            container: ImportedAudioContainer::Xvag,
            revision: bytes.get(0x0b).map(|value| format!("xvag-0x{value:02x}")),
            streams: vec![ImportedAudioStream::unknown(0)],
            confidence: ProbeConfidence::Magic,
        });
    };

    let codec = match header.codec {
        XvagCodec::PsAdpcm | XvagCodec::PsAdpcmExtended => Some(ImportedAudioCodec::PsAdpcm),
        XvagCodec::Mpeg => Some(ImportedAudioCodec::Mpeg),
        XvagCodec::Atrac9 => Some(ImportedAudioCodec::Atrac9),
        XvagCodec::Unknown(_) => None,
    };
    Some(ImportedAudioProbe {
        container: ImportedAudioContainer::Xvag,
        revision: Some(format!(
            "xvag-0x{:02x}-{}-subsongs{}-layers{}",
            header.version_flag,
            if header.big_endian { "be" } else { "le" },
            header.subsongs,
            header.layers
        )),
        streams: vec![ImportedAudioStream {
            track_index: 0,
            codec,
            sample_rate_hz: Some(header.sample_rate_hz),
            channels: u16::try_from(header.channels).ok(),
        }],
        confidence: ProbeConfidence::MagicAndHeader,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncated_probe_keeps_codec_unknown() {
        let probe = probe_xvag(b"XVAG\0\0\0\0").expect("xvag");
        assert_eq!(probe.primary_codec(), None);
        assert_eq!(probe.confidence, ProbeConfidence::Magic);
    }

    #[test]
    fn generated_ps_adpcm_xvag_is_recognized_by_import_probe() {
        let source = vec![0.0_f32; 56 * 2];
        let bytes = newengine_audio_xvag::encode_xvag_ps_adpcm(48_000, 2, &source).unwrap();
        let probe = probe_xvag(&bytes).expect("xvag");
        assert_eq!(probe.primary_codec(), Some(ImportedAudioCodec::PsAdpcm));
        assert_eq!(probe.streams[0].channels, Some(2));
        assert_eq!(probe.streams[0].sample_rate_hz, Some(48_000));
        assert_eq!(probe.confidence, ProbeConfidence::MagicAndHeader);
        assert!(probe
            .revision
            .as_deref()
            .unwrap()
            .contains("subsongs1-layers1"));
    }

    #[test]
    fn codec_tags_map_without_conflating_container_and_codec() {
        let source = vec![0.0_f32; 56];
        let mut bytes = newengine_audio_xvag::encode_xvag_ps_adpcm(44_100, 1, &source).unwrap();
        for (tag, expected) in [
            (0x06_u32, Some(ImportedAudioCodec::PsAdpcm)),
            (0x08_u32, Some(ImportedAudioCodec::Mpeg)),
            (0x09_u32, Some(ImportedAudioCodec::Atrac9)),
        ] {
            bytes[0x2c..0x30].copy_from_slice(&tag.to_le_bytes());
            let probe = probe_xvag(&bytes).expect("xvag");
            assert_eq!(probe.container, ImportedAudioContainer::Xvag);
            assert_eq!(probe.primary_codec(), expected);
        }
    }
}
