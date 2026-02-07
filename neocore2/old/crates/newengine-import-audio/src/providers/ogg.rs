use crate::providers::{common, AudioMetaV1, AudioProviderV1, ProviderEntry};

pub struct OggProvider;

impl AudioProviderV1 for OggProvider {
    fn container(&self) -> &'static str {
        "ogg"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["ogg", "opus"]
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        bytes.len() >= 4 && &bytes[0..4] == b"OggS"
    }

    fn probe_meta(&self, bytes: &[u8]) -> Result<AudioMetaV1, String> {
        common::probe_symphonia(bytes, Some("ogg"), "ogg")
    }

    fn describe_json(&self) -> &'static str {
        r#"{"container":"ogg","extensions":["ogg","opus"],"sniff":"OggS","method":"import_audio_v1"}"#
    }
}

static PROVIDER: OggProvider = OggProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
