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

#[inline]
fn le_u24(b: &[u8], off: usize) -> Option<u32> {
    if off + 3 <= b.len() {
        Some((b[off] as u32) | ((b[off + 1] as u32) << 8) | ((b[off + 2] as u32) << 16))
    } else {
        None
    }
}

fn parse_webp_size(bytes: &[u8]) -> Result<(u32, u32, &'static str), String> {
    if bytes.len() < 16 {
        return Err("webp: truncated".into());
    }
    if &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return Err("webp: bad RIFF/WEBP".into());
    }

    let chunk = &bytes[12..16];
    if chunk == b"VP8 " {
        if bytes.len() < 30 {
            return Err("webp: VP8 truncated".into());
        }
        if bytes[20] != 0x9D || bytes[21] != 0x01 || bytes[22] != 0x2A {
            return Err("webp: VP8 invalid frame".into());
        }
        let w = le_u16(bytes, 26).ok_or("webp: VP8 width")? as u32 & 0x3FFF;
        let h = le_u16(bytes, 28).ok_or("webp: VP8 height")? as u32 & 0x3FFF;
        if w == 0 || h == 0 {
            return Err("webp: VP8 zero size".into());
        }
        return Ok((w, h, "YUV"));
    }

    if chunk == b"VP8L" {
        if bytes.len() < 25 {
            return Err("webp: VP8L truncated".into());
        }
        if bytes[20] != 0x2F {
            return Err("webp: VP8L bad signature".into());
        }
        let b0 = bytes[21] as u32;
        let b1 = bytes[22] as u32;
        let b2 = bytes[23] as u32;
        let b3 = bytes[24] as u32;
        let bits = b0 | (b1 << 8) | (b2 << 16) | (b3 << 24);

        let w = (bits & 0x3FFF) + 1;
        let h = ((bits >> 14) & 0x3FFF) + 1;
        if w == 0 || h == 0 {
            return Err("webp: VP8L zero size".into());
        }
        return Ok((w, h, "RGBA8"));
    }

    if chunk == b"VP8X" {
        if bytes.len() < 30 {
            return Err("webp: VP8X truncated".into());
        }
        let w = le_u24(bytes, 24).ok_or("webp: VP8X width")? + 1;
        let h = le_u24(bytes, 27).ok_or("webp: VP8X height")? + 1;
        if w == 0 || h == 0 {
            return Err("webp: VP8X zero size".into());
        }
        return Ok((w, h, "RGBA8"));
    }

    Err("webp: unsupported chunk type".into())
}

pub struct WebpProvider;

impl WebpProvider {
    #[inline]
    fn build_meta_json(width: u32, height: u32, fmt: &str) -> String {
        format!(
            "{{\"schema\":\"kalitech.texture.meta.v1\",\"container\":\"webp\",\"width\":{width},\"height\":{height},\"depth\":1,\"mips\":1,\"is_cube\":false,\"format\":\"{fmt}\"}}"
        )
    }
}

impl ImageProviderV1 for WebpProvider {
    fn container(&self) -> &'static str {
        "webp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["webp"]
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
    }

    fn import(&self, bytes: &[u8]) -> RResult<RVec<u8>, RString> {
        let (w, h, fmt) = match parse_webp_size(bytes) {
            Ok(v) => v,
            Err(e) => return err(e),
        };
        let meta = Self::build_meta_json(w, h, fmt);
        ok(pack(&meta, bytes))
    }

    fn describe_json(&self) -> &'static str {
        r#"{"container":"webp","extensions":["webp"],"sniff":"RIFF....WEBP","method":"import_image_v1"}"#
    }
}

static PROVIDER: WebpProvider = WebpProvider;

inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
