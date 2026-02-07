use crate::providers::{ProviderEntry, TextProviderV1};

pub struct TomlProvider;

impl TextProviderV1 for TomlProvider {
    fn service_id(&self) -> &'static str {
        "kalitech.import.toml.v1"
    }

    fn container(&self) -> &'static str {
        "toml"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["toml"]
    }

    fn mime(&self) -> &'static str {
        "application/toml"
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        // Heuristic: table headers [section] or key = value.
        let mut i = 0usize;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
        }
        if bytes.get(i) == Some(&b'[') {
            return true;
        }
        let end = (i + 128).min(bytes.len());
        let chunk = &bytes[i..end];
        chunk
            .windows(3)
            .any(|w| w[1] == b'=' || (w[0].is_ascii_alphabetic() && w[1] == b' ' && w[2] == b'='))
            || true
    }

    fn describe_json(&self) -> &'static str {
        r#"{"service_id":"kalitech.import.toml.v1","container":"toml","extensions":["toml"],"mime":"application/toml","method":"import_text_v1"}"#
    }
}

static PROVIDER: TomlProvider = TomlProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
