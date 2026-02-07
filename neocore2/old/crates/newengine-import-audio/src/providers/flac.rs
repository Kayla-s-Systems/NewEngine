use crate::providers::{common, AudioMetaV1, AudioProviderV1, ProviderEntry};

pub struct FlacProvider;

impl AudioProviderV1 for FlacProvider {
    fn container(&self) -> &'static str {
        "flac"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["flac"]
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        bytes.len() >= 4 && &bytes[0..4] == b"fLaC"
    }

    fn probe_meta(&self, bytes: &[u8]) -> Result<AudioMetaV1, String> {
        common::probe_symphonia(bytes, Some("flac"), "flac")
    }

    fn describe_json(&self) -> &'static str {
        r#"{"container":"flac","extensions":["flac"],"sniff":"fLaC","method":"import_audio_v1"}"#
    }
}

static PROVIDER: FlacProvider = FlacProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
