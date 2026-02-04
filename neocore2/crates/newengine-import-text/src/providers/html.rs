use crate::providers::{ProviderEntry, TextProviderV1};

pub struct HtmlProvider;

impl TextProviderV1 for HtmlProvider {
    fn service_id(&self) -> &'static str {
        "kalitech.import.html.v1"
    }

    fn container(&self) -> &'static str {
        "html"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["html", "htm"]
    }

    fn mime(&self) -> &'static str {
        "text/html"
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        let mut i = 0usize;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
        }
        if bytes.get(i) != Some(&b'<') {
            return false;
        }
        let tail = &bytes[i..bytes.len().min(i + 64)];
        let low = tail
            .iter()
            .map(|c| c.to_ascii_lowercase())
            .collect::<Vec<u8>>();
        low.windows(5).any(|w| w == b"<html") || low.windows(5).any(|w| w == b"<!doc")
    }

    fn describe_json(&self) -> &'static str {
        r#"{"service_id":"kalitech.import.html.v1","container":"html","extensions":["html","htm"],"mime":"text/html","method":"import_text_v1"}"#
    }
}

static PROVIDER: HtmlProvider = HtmlProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
