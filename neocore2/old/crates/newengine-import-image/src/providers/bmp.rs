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
fn le_u32(b: &[u8], off: usize) -> Option<u32> {
    if off + 4 <= b.len() {
        Some(u32::from_le_bytes([
            b[off],
            b[off + 1],
            b[off + 2],
            b[off + 3],
        ]))
    } else {
        None
    }
}

#[inline]
fn le_i32(b: &[u8], off: usize) -> Option<i32> {
    if off + 4 <= b.len() {
        Some(i32::from_le_bytes([
            b[off],
            b[off + 1],
            b[off + 2],
            b[off + 3],
        ]))
    } else {
        None
    }
}

fn parse_bmp_size(bytes: &[u8]) -> Result<(u32, u32, &'static str), String> {
    if bytes.len() < 54 || bytes[0] != b'B' || bytes[1] != b'M' {
        return Err("bmp: bad signature".into());
    }

    let dib_size = le_u32(bytes, 14).ok_or("bmp: truncated DIB header")? as usize;
    if dib_size < 40 {
        return Err("bmp: unsupported DIB header".into());
    }

    let width = le_i32(bytes, 18).ok_or("bmp: truncated width")?;
    let height = le_i32(bytes, 22).ok_or("bmp: truncated height")?;
    let w = width.unsigned_abs();
    let h = height.unsigned_abs();

    if w == 0 || h == 0 {
        return Err("bmp: zero size".into());
    }

    let bpp = le_u16(bytes, 28).ok_or("bmp: truncated bpp")?;
    let fmt = match bpp {
        24 => "BGR8",
        32 => "BGRA8",
        8 => "INDEXED8",
        16 => "BGR565",
        _ => "UNKNOWN",
    };

    Ok((w, h, fmt))
}

pub struct BmpProvider;

impl BmpProvider {
    #[inline]
    fn build_meta_json(width: u32, height: u32, fmt: &str) -> String {
        format!(
            "{{\"schema\":\"kalitech.texture.meta.v1\",\"container\":\"bmp\",\"width\":{width},\"height\":{height},\"depth\":1,\"mips\":1,\"is_cube\":false,\"format\":\"{fmt}\"}}"
        )
    }
}

impl ImageProviderV1 for BmpProvider {
    fn container(&self) -> &'static str {
        "bmp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["bmp"]
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        bytes.len() >= 2 && bytes[0] == b'B' && bytes[1] == b'M'
    }

    fn import(&self, bytes: &[u8]) -> RResult<RVec<u8>, RString> {
        let (w, h, fmt) = match parse_bmp_size(bytes) {
            Ok(v) => v,
            Err(e) => return err(e),
        };
        let meta = Self::build_meta_json(w, h, fmt);
        ok(pack(&meta, bytes))
    }

    fn describe_json(&self) -> &'static str {
        r#"{"container":"bmp","extensions":["bmp"],"sniff":"magic: BM","method":"import_image_v1"}"#
    }
}

static PROVIDER: BmpProvider = BmpProvider;

inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
