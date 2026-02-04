use crate::providers::{ProviderEntry, TextProviderV1};

pub struct ShaderStageProvider;

impl TextProviderV1 for ShaderStageProvider {
    fn service_id(&self) -> &'static str {
        "kalitech.import.shader.v1"
    }

    fn container(&self) -> &'static str {
        "shader"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["vert", "frag", "comp"]
    }

    fn mime(&self) -> &'static str {
        "text/plain"
    }

    fn describe_json(&self) -> &'static str {
        r#"{"service_id":"kalitech.import.shader.v1","container":"shader","extensions":["vert","frag","comp"],"mime":"text/plain","method":"import_text_v1"}"#
    }
}

static PROVIDER: ShaderStageProvider = ShaderStageProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
