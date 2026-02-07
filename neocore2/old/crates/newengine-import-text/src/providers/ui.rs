use crate::providers::{ProviderEntry, TextProviderV1};

pub struct UiProvider;

impl TextProviderV1 for UiProvider {
    fn service_id(&self) -> &'static str {
        "kalitech.import.ui.v1"
    }

    fn container(&self) -> &'static str {
        "ui"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ui"]
    }

    fn mime(&self) -> &'static str {
        "text/plain"
    }

    fn describe_json(&self) -> &'static str {
        r#"{"service_id":"kalitech.import.ui.v1","container":"ui","extensions":["ui"],"mime":"text/plain","method":"import_text_v1"}"#
    }
}

static PROVIDER: UiProvider = UiProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
