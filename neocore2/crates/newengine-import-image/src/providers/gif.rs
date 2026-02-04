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

fn parse_gif_size(bytes: &[u8]) -> Result<(u32, u32), String> {
    if bytes.len() < 10 {
        return Err("gif: truncated".into());
    }
    let sig = &bytes[..6];
    if sig != b"GIF87a" && sig != b"GIF89a" {
        return Err("gif: bad signature".into());
    }

    let w = le_u16(bytes, 6).ok_or("gif: truncated width")? as u32;
    let h = le_u16(bytes, 8).ok_or("gif: truncated height")? as u32;
    if w == 0 || h == 0 {
        return Err("gif: zero size".into());
    }
    Ok((w, h))
}

pub struct GifProvider;

impl GifProvider {
    #[inline]
    fn build_meta_json(width: u32, height: u32) -> String {
        format!(
            "{{\"schema\":\"kalitech.texture.meta.v1\",\"container\":\"gif\",\"width\":{width},\"height\":{height},\"depth\":1,\"mips\":1,\"is_cube\":false,\"format\":\"INDEXED8\"}}"
        )
    }
}

impl ImageProviderV1 for GifProvider {
    fn container(&self) -> &'static str {
        "gif"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["gif"]
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        bytes.len() >= 6 && (&bytes[..6] == b"GIF87a" || &bytes[..6] == b"GIF89a")
    }

    fn import(&self, bytes: &[u8]) -> RResult<RVec<u8>, RString> {
        let (w, h) = match parse_gif_size(bytes) {
            Ok(v) => v,
            Err(e) => return err(e),
        };
        let meta = Self::build_meta_json(w, h);
        ok(pack(&meta, bytes))
    }

    fn describe_json(&self) -> &'static str {
        r#"{"container":"gif","extensions":["gif"],"sniff":"magic: GIF87a/GIF89a","method":"import_image_v1"}"#
    }
}

static PROVIDER: GifProvider = GifProvider;

inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
