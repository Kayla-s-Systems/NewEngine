use crate::providers::{ProviderEntry, TextProviderV1};

pub struct WgslProvider;

impl TextProviderV1 for WgslProvider {
    fn service_id(&self) -> &'static str {
        "kalitech.import.wgsl.v1"
    }

    fn container(&self) -> &'static str {
        "wgsl"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["wgsl"]
    }

    fn mime(&self) -> &'static str {
        "text/plain"
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        // Heuristic: look for "@group" or "fn main" patterns.
        let end = bytes.len().min(512);
        let low = bytes[..end]
            .iter()
            .map(|c| c.to_ascii_lowercase())
            .collect::<Vec<u8>>();
        low.windows(6).any(|w| w == b"@group")
            || low.windows(6).any(|w| w == b"@stage")
            || low.windows(2).any(|w| w == b"fn")
            || true
    }

    fn describe_json(&self) -> &'static str {
        r#"{"service_id":"kalitech.import.wgsl.v1","container":"wgsl","extensions":["wgsl"],"mime":"text/plain","method":"import_text_v1"}"#
    }
}

static PROVIDER: WgslProvider = WgslProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
