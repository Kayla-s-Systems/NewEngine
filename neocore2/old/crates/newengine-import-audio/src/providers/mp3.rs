use crate::providers::{common, AudioMetaV1, AudioProviderV1, ProviderEntry};

pub struct Mp3Provider;

impl AudioProviderV1 for Mp3Provider {
    fn container(&self) -> &'static str {
        "mp3"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["mp3"]
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        if bytes.len() >= 3 && &bytes[0..3] == b"ID3" {
            return true;
        }
        if bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0 {
            return true;
        }
        false
    }

    fn probe_meta(&self, bytes: &[u8]) -> Result<AudioMetaV1, String> {
        common::probe_symphonia(bytes, Some("mp3"), "mp3")
    }

    fn describe_json(&self) -> &'static str {
        r#"{"container":"mp3","extensions":["mp3"],"sniff":"ID3 or frame sync","method":"import_audio_v1"}"#
    }
}

static PROVIDER: Mp3Provider = Mp3Provider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
