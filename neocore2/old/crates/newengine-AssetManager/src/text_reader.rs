#![forbid(unsafe_op_in_unsafe_fn)]

use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextFormat {
    Json,
    Xml,
    Html,
    Txt,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct TextMeta {
    pub schema: String,
    pub container: String,
    pub encoding: String,
    pub byte_len: u64,
}

#[derive(Debug, Clone)]
pub struct TextDocument {
    pub format: TextFormat,
    pub meta: TextMeta,
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TextReadError {
    #[error("wire: too short")]
    TooShort,
    #[error("wire: bad magic/version")]
    BadWireHeader,
    #[error("wire: meta length out of bounds")]
    MetaOutOfBounds,
    #[error("wire: meta length too large ({0} bytes)")]
    MetaTooLarge(usize),
    #[error("wire: payload length out of bounds")]
    PayloadOutOfBounds,
    #[error("meta json: {0}")]
    MetaJson(String),
    #[error("utf8: {0}")]
    Utf8(String),
    #[error("json parse: {0}")]
    JsonParse(String),
    #[error("xml parse: {0}")]
    XmlParse(String),
}

pub struct TextReader;

impl TextReader {
    /// Hard cap to prevent pathological allocations / malformed assets.
    pub const MAX_META_BYTES: usize = 64 * 1024;

    /// "Real wire" frame:
    /// [4]  magic = b"NTX1"
    /// [4]  meta_len_le (u32)
    /// [4]  payload_len_le (u32)
    /// [N]  meta_json utf8
    /// [M]  payload bytes (utf-8 text)
    pub const WIRE_MAGIC: [u8; 4] = *b"NTX1";

    /// Builds a TextDocument from an AssetBlob split fields:
    /// - meta_json: blob.meta_json
    /// - payload: blob.payload (utf-8 text bytes)
    ///
    /// Invariant: output text uses LF newlines only (CRLF/CR are normalized).
    pub fn from_blob_parts(meta_json: &str, payload: &[u8]) -> Result<TextDocument, TextReadError> {
        let meta = parse_meta_json(meta_json)?;

        let raw = std::str::from_utf8(payload).map_err(|e| TextReadError::Utf8(e.to_string()))?;
        let raw = strip_utf8_bom(raw);

        let text = normalize_newlines_to_lf(raw);

        let format = match meta.container.as_str() {
            "json" => TextFormat::Json,
            "xml" => TextFormat::Xml,
            "html" => TextFormat::Html,
            "txt" => TextFormat::Txt,
            _ => TextFormat::Unknown,
        };

        Ok(TextDocument { format, meta, text })
    }

    /// Decodes the "real wire" frame into TextDocument.
    /// This is for cases where you intentionally stored/transported a single packed buffer.
    pub fn read_wire(bytes: &[u8]) -> Result<TextDocument, TextReadError> {
        // header: magic(4) + meta_len(4) + payload_len(4) = 12
        if bytes.len() < 12 {
            return Err(TextReadError::TooShort);
        }

        if bytes[0..4] != Self::WIRE_MAGIC {
            return Err(TextReadError::BadWireHeader);
        }

        let meta_len = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        if meta_len > Self::MAX_META_BYTES {
            return Err(TextReadError::MetaTooLarge(meta_len));
        }

        let payload_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;

        let meta_start = 12usize;
        let meta_end = meta_start.saturating_add(meta_len);
        if meta_end > bytes.len() {
            return Err(TextReadError::MetaOutOfBounds);
        }

        let payload_start = meta_end;
        let payload_end = payload_start.saturating_add(payload_len);
        if payload_end > bytes.len() {
            return Err(TextReadError::PayloadOutOfBounds);
        }

        let meta_bytes = &bytes[meta_start..meta_end];
        let payload = &bytes[payload_start..payload_end];

        let meta_str =
            std::str::from_utf8(meta_bytes).map_err(|e| TextReadError::Utf8(e.to_string()))?;

        Self::from_blob_parts(meta_str, payload)
    }

    /// Encodes split (meta_json + payload) into a "real wire" buffer (magic + lengths).
    pub fn encode_wire_v1(meta_json: &str, payload: &[u8]) -> Vec<u8> {
        let meta = meta_json.as_bytes();
        let meta_len = (meta.len().min(u32::MAX as usize)) as u32;
        let payload_len = (payload.len().min(u32::MAX as usize)) as u32;

        let mut out = Vec::with_capacity(12 + meta.len() + payload.len());
        out.extend_from_slice(&Self::WIRE_MAGIC);
        out.extend_from_slice(&meta_len.to_le_bytes());
        out.extend_from_slice(&payload_len.to_le_bytes());
        out.extend_from_slice(meta);
        out.extend_from_slice(payload);
        out
    }

    pub fn parse_json(doc: &TextDocument) -> Result<JsonValue, TextReadError> {
        if doc.format != TextFormat::Json {
            return Err(TextReadError::JsonParse("document is not json".to_owned()));
        }
        serde_json::from_str(&doc.text).map_err(|e| TextReadError::JsonParse(e.to_string()))
    }

    /// Validates XML well-formedness by tokenizing it.
    /// This does not trim/normalize text nodes (keeps compatibility with quick-xml versions).
    pub fn validate_xml(doc: &TextDocument) -> Result<(), TextReadError> {
        if doc.format != TextFormat::Xml {
            return Err(TextReadError::XmlParse("document is not xml".to_owned()));
        }

        let mut r = quick_xml::Reader::from_reader(doc.text.as_bytes());
        let mut buf = Vec::new();

        loop {
            match r.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(_) => {}
                Err(e) => return Err(TextReadError::XmlParse(e.to_string())),
            }
            buf.clear();
        }

        Ok(())
    }
}

fn parse_meta_json(meta_json: &str) -> Result<TextMeta, TextReadError> {
    let v: serde_json::Value =
        serde_json::from_str(meta_json).map_err(|e| TextReadError::MetaJson(e.to_string()))?;

    let schema = v.get("schema").and_then(|x| x.as_str()).unwrap_or("").to_owned();

    let container_raw = v
        .get("container")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_owned();

    let container = normalize_container(&container_raw);

    let encoding = v
        .get("encoding")
        .and_then(|x| x.as_str())
        .unwrap_or("utf-8")
        .to_owned();

    let byte_len = v.get("byte_len").and_then(|x| x.as_u64()).unwrap_or(0);

    Ok(TextMeta {
        schema,
        container,
        encoding,
        byte_len,
    })
}

#[inline]
fn normalize_container(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "htm" | "html" => "html".to_owned(),
        "json" => "json".to_owned(),
        "xml" => "xml".to_owned(),
        "txt" | "text" | "ui" | "md" => "txt".to_owned(),
        other => other.to_owned(),
    }
}

#[inline]
fn strip_utf8_bom(s: &str) -> &str {
    const BOM: char = '\u{feff}';
    s.strip_prefix(BOM).unwrap_or(s)
}

/// Normalizes CRLF and CR line endings into LF.
///
/// Fast path: if no '\r' exists, returns input as-is owned.
/// This keeps an invariant: engine-side text is always LF.
#[inline]
fn normalize_newlines_to_lf(s: &str) -> String {
    if !s.as_bytes().contains(&b'\r') {
        return s.to_owned();
    }

    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());

    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\r' {
            // Convert CR or CRLF into single LF.
            out.push('\n');
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }

        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}