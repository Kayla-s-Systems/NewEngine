use crate::providers::{ProviderEntry, TextProviderV1};

pub struct XmlProvider;

impl TextProviderV1 for XmlProvider {
    fn service_id(&self) -> &'static str {
        "kalitech.import.xml.v1"
    }

    fn container(&self) -> &'static str {
        "xml"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["xml"]
    }

    fn mime(&self) -> &'static str {
        "application/xml"
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        let mut i = 0usize;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
        }
        bytes.get(i) == Some(&b'<')
    }

    fn describe_json(&self) -> &'static str {
        r#"{"service_id":"kalitech.import.xml.v1","container":"xml","extensions":["xml"],"mime":"application/xml","method":"import_text_v1"}"#
    }
}

static PROVIDER: XmlProvider = XmlProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
