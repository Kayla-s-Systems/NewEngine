use crate::providers::{common, AudioMetaV1, AudioProviderV1, ProviderEntry};

pub struct AacProvider;

impl AudioProviderV1 for AacProvider {
    fn container(&self) -> &'static str {
        "aac"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["aac"]
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        if bytes.len() >= 4 && &bytes[0..4] == b"ADIF" {
            return true;
        }
        if bytes.len() >= 2 {
            let b0 = bytes[0];
            let b1 = bytes[1];
            if b0 == 0xFF && (b1 & 0xF0) == 0xF0 {
                return true;
            }
        }
        false
    }

    fn probe_meta(&self, bytes: &[u8]) -> Result<AudioMetaV1, String> {
        common::probe_symphonia(bytes, Some("aac"), "aac")
    }

    fn describe_json(&self) -> &'static str {
        r#"{"container":"aac","extensions":["aac"],"sniff":"ADIF/ADTS","method":"import_audio_v1"}"#
    }
}

static PROVIDER: AacProvider = AacProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
