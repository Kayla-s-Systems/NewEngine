use crate::providers::{ProviderEntry, TextProviderV1};

pub struct YamlProvider;

impl TextProviderV1 for YamlProvider {
    fn service_id(&self) -> &'static str {
        "kalitech.import.yaml.v1"
    }

    fn container(&self) -> &'static str {
        "yaml"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["yaml", "yml"]
    }

    fn mime(&self) -> &'static str {
        "application/yaml"
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        // Heuristic: YAML often starts with '---' or key-value patterns.
        let mut i = 0usize;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
        }
        if i + 3 <= bytes.len() && &bytes[i..i + 3] == b"---" {
            return true;
        }
        // Look for "key:" in first ~64 bytes.
        let end = (i + 128).min(bytes.len());
        let chunk = &bytes[i..end];
        for w in chunk.windows(2) {
            if w[0].is_ascii_alphabetic() && w[1] == b':' {
                return true;
            }
        }
        true
    }

    fn describe_json(&self) -> &'static str {
        r#"{"service_id":"kalitech.import.yaml.v1","container":"yaml","extensions":["yaml","yml"],"mime":"application/yaml","method":"import_text_v1"}"#
    }
}

static PROVIDER: YamlProvider = YamlProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
