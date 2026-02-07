use crate::providers::{common, AudioMetaV1, AudioProviderV1, ProviderEntry};

pub struct WavProvider;

impl AudioProviderV1 for WavProvider {
    fn container(&self) -> &'static str {
        "wav"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["wav"]
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE"
    }

    fn probe_meta(&self, bytes: &[u8]) -> Result<AudioMetaV1, String> {
        common::probe_symphonia(bytes, Some("wav"), "wav")
    }

    fn describe_json(&self) -> &'static str {
        r#"{"container":"wav","extensions":["wav"],"sniff":"RIFF....WAVE","method":"import_audio_v1"}"#
    }
}

static PROVIDER: WavProvider = WavProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
