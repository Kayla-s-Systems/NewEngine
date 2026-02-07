use crate::providers::{ProviderEntry, TextProviderV1};

pub struct IniProvider;

impl TextProviderV1 for IniProvider {
    fn service_id(&self) -> &'static str {
        "kalitech.import.ini.v1"
    }

    fn container(&self) -> &'static str {
        "ini"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ini", "cfg"]
    }

    fn mime(&self) -> &'static str {
        "text/plain"
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        // INI often has [section] or key=value.
        let mut i = 0usize;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n') {
            i += 1;
        }
        if bytes.get(i) == Some(&b'[') {
            return true;
        }
        let end = (i + 128).min(bytes.len());
        let chunk = &bytes[i..end];
        chunk.contains(&b'=') || true
    }

    fn describe_json(&self) -> &'static str {
        r#"{"service_id":"kalitech.import.ini.v1","container":"ini","extensions":["ini","cfg"],"mime":"text/plain","method":"import_text_v1"}"#
    }
}

static PROVIDER: IniProvider = IniProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
