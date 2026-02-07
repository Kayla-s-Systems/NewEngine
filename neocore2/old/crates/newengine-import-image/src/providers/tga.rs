use abi_stable::std_types::{RResult, RString, RVec};

use crate::providers::{ImageProviderV1, ProviderEntry};

#[inline]
fn pack(meta_json: &str, payload: &[u8]) -> RVec<u8> {
    let meta = meta_json.as_bytes();
    let meta_len: u32 = meta.len().min(u32::MAX as usize) as u32;

    let mut out = Vec::with_capacity(4 + meta.len() + payload.len());
    out.extend_from_slice(&meta_len.to_le_bytes());
    out.extend_from_slice(meta);
    out.extend_from_slice(payload);
    RVec::from(out)
}

#[inline]
fn ok(v: RVec<u8>) -> RResult<RVec<u8>, RString> {
    RResult::ROk(v)
}

#[inline]
fn err(msg: impl Into<String>) -> RResult<RVec<u8>, RString> {
    RResult::RErr(RString::from(msg.into()))
}

#[inline]
fn le_u16(b: &[u8], off: usize) -> Option<u16> {
    if off + 2 <= b.len() {
        Some(u16::from_le_bytes([b[off], b[off + 1]]))
    } else {
        None
    }
}

fn plausible_tga_header(bytes: &[u8]) -> Option<(u32, u32, &'static str)> {
    if bytes.len() < 18 {
        return None;
    }

    let id_len = bytes[0] as usize;
    let cmap_type = bytes[1];
    let img_type = bytes[2];
    if cmap_type > 1 {
        return None;
    }

    let supported = matches!(img_type, 2 | 3 | 10 | 11);
    if !supported {
        return None;
    }

    let width = le_u16(bytes, 12)? as u32;
    let height = le_u16(bytes, 14)? as u32;
    if width == 0 || height == 0 {
        return None;
    }
    if width > 16384 || height > 16384 {
        return None;
    }

    let bpp = bytes[16];
    let fmt = match (img_type, bpp) {
        (3 | 11, 8) => "GRAY8",
        (2 | 10, 24) => "BGR8",
        (2 | 10, 32) => "BGRA8",
        (2 | 10, 16) => "BGR565",
        _ => "UNKNOWN",
    };

    if 18 + id_len > bytes.len() {
        return None;
    }

    Some((width, height, fmt))
}

pub struct TgaProvider;

impl TgaProvider {
    #[inline]
    fn build_meta_json(width: u32, height: u32, fmt: &str) -> String {
        format!(
            "{{\"schema\":\"kalitech.texture.meta.v1\",\"container\":\"tga\",\"width\":{width},\"height\":{height},\"depth\":1,\"mips\":1,\"is_cube\":false,\"format\":\"{fmt}\"}}"
        )
    }
}

impl ImageProviderV1 for TgaProvider {
    fn container(&self) -> &'static str {
        "tga"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["tga"]
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        plausible_tga_header(bytes).is_some()
    }

    fn import(&self, bytes: &[u8]) -> RResult<RVec<u8>, RString> {
        let (w, h, fmt) = match plausible_tga_header(bytes) {
            Some(v) => v,
            None => return err("tga: unsupported or invalid header"),
        };

        let meta = Self::build_meta_json(w, h, fmt);
        ok(pack(&meta, bytes))
    }

    fn describe_json(&self) -> &'static str {
        r#"{"container":"tga","extensions":["tga"],"sniff":"heuristic: TGA header","method":"import_image_v1"}"#
    }
}

static PROVIDER: TgaProvider = TgaProvider;

inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
