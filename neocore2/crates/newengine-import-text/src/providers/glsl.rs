use crate::providers::{ProviderEntry, TextProviderV1};

pub struct GlslProvider;

impl TextProviderV1 for GlslProvider {
    fn service_id(&self) -> &'static str {
        "kalitech.import.glsl.v1"
    }

    fn container(&self) -> &'static str {
        "glsl"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["glsl"]
    }

    fn mime(&self) -> &'static str {
        "text/plain"
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        // Look for "#version" in first bytes.
        let end = bytes.len().min(256);
        let low = bytes[..end]
            .iter()
            .map(|c| c.to_ascii_lowercase())
            .collect::<Vec<u8>>();
        low.windows(8).any(|w| w == b"#version") || true
    }

    fn describe_json(&self) -> &'static str {
        r#"{"service_id":"kalitech.import.glsl.v1","container":"glsl","extensions":["glsl"],"mime":"text/plain","method":"import_text_v1"}"#
    }
}

static PROVIDER: GlslProvider = GlslProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
