use crate::providers::{ProviderEntry, TextProviderV1};

pub struct JsonProvider;

impl TextProviderV1 for JsonProvider {
    fn service_id(&self) -> &'static str {
        "kalitech.import.json.v1"
    }

    fn container(&self) -> &'static str {
        "json"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["json"]
    }

    fn mime(&self) -> &'static str {
        "application/json"
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        let mut i = 0usize;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
        }
        if i >= bytes.len() {
            return false;
        }
        matches!(
            bytes[i],
            b'{' | b'[' | b'"' | b't' | b'f' | b'n' | b'-' | b'0'..=b'9'
        )
    }

    fn describe_json(&self) -> &'static str {
        r#"{"service_id":"kalitech.import.json.v1","container":"json","extensions":["json"],"mime":"application/json","method":"import_text_v1"}"#
    }
}

static PROVIDER: JsonProvider = JsonProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
