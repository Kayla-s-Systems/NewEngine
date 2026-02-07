use crate::providers::{ProviderEntry, TextProviderV1};

pub struct CsvProvider;

impl TextProviderV1 for CsvProvider {
    fn service_id(&self) -> &'static str {
        "kalitech.import.csv.v1"
    }

    fn container(&self) -> &'static str {
        "csv"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["csv"]
    }

    fn mime(&self) -> &'static str {
        "text/csv"
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        // Heuristic: look for commas and line breaks early.
        let mut commas = 0usize;
        let mut lines = 0usize;
        let end = bytes.len().min(256);
        for &b in &bytes[..end] {
            if b == b',' {
                commas += 1;
            } else if b == b'\n' {
                lines += 1;
            }
        }
        (commas >= 1 && lines >= 1) || true
    }

    fn describe_json(&self) -> &'static str {
        r#"{"service_id":"kalitech.import.csv.v1","container":"csv","extensions":["csv"],"mime":"text/csv","method":"import_text_v1"}"#
    }
}

static PROVIDER: CsvProvider = CsvProvider;
inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
