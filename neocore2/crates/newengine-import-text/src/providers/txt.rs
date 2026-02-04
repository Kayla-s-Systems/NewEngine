use crate::providers::{ProviderEntry, TextProviderV1};

pub struct TxtProvider;

impl TextProviderV1 for TxtProvider {
    fn service_id(&self) -> &'static str {
        "kalitech.import.txt.v1"
    }

    fn container(&self) -> &'static str {
        "txt"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["txt", "md"]
    }

    fn mime(&self) -> &'static str {
        "text/plain"
    }

    fn describe_json(&self) -> &'static str {
        r#"{"service_id":"kalitech.import.txt.v1","container":"txt","extensions":["txt","md"],"mime":"text/plain","method":"import_text_v1"}"#
    }
}

static PROVIDER: TxtProvider = TxtProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
