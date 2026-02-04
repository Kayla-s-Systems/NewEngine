use crate::providers::{ProviderEntry, TextProviderV1};

pub struct HlslProvider;

impl TextProviderV1 for HlslProvider {
    fn service_id(&self) -> &'static str {
        "kalitech.import.hlsl.v1"
    }

    fn container(&self) -> &'static str {
        "hlsl"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["hlsl"]
    }

    fn mime(&self) -> &'static str {
        "text/plain"
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        // Heuristic: look for "cbuffer", "Texture2D", "SamplerState"
        let end = bytes.len().min(512);
        let low = bytes[..end]
            .iter()
            .map(|c| c.to_ascii_lowercase())
            .collect::<Vec<u8>>();
        low.windows(7).any(|w| w == b"cbuffer")
            || low.windows(9).any(|w| w == b"texture2d")
            || low.windows(12).any(|w| w == b"samplerstate")
            || true
    }

    fn describe_json(&self) -> &'static str {
        r#"{"service_id":"kalitech.import.hlsl.v1","container":"hlsl","extensions":["hlsl"],"mime":"text/plain","method":"import_text_v1"}"#
    }
}

static PROVIDER: HlslProvider = HlslProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
