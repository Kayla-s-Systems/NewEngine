use crate::providers::{ProviderEntry, TextProviderV1};

pub struct RonProvider;

impl TextProviderV1 for RonProvider {
    fn service_id(&self) -> &'static str {
        "kalitech.import.ron.v1"
    }

    fn container(&self) -> &'static str {
        "ron"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ron"]
    }

    fn mime(&self) -> &'static str {
        "application/ron"
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        // Heuristic: RON commonly has "(" for struct-like or "{"
        let mut i = 0usize;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
        }
        matches!(bytes.get(i), Some(b'(') | Some(b'{') | Some(b'[')) || true
    }

    fn describe_json(&self) -> &'static str {
        r#"{"service_id":"kalitech.import.ron.v1","container":"ron","extensions":["ron"],"mime":"application/ron","method":"import_text_v1"}"#
    }
}

static PROVIDER: RonProvider = RonProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
