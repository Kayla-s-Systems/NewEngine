use crate::providers::{common, AudioMetaV1, AudioProviderV1, ProviderEntry};

pub struct M4aProvider;

impl AudioProviderV1 for M4aProvider {
    fn container(&self) -> &'static str {
        "m4a"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["m4a", "mp4"]
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        if bytes.len() < 12 {
            return false;
        }
        if &bytes[4..8] != b"ftyp" {
            return false;
        }
        true
    }

    fn probe_meta(&self, bytes: &[u8]) -> Result<AudioMetaV1, String> {
        common::probe_symphonia(bytes, Some("m4a"), "m4a")
    }

    fn describe_json(&self) -> &'static str {
        r#"{"container":"m4a","extensions":["m4a","mp4"],"sniff":"ftyp","method":"import_audio_v1"}"#
    }
}

static PROVIDER: M4aProvider = M4aProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
