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
fn be_u16(b0: u8, b1: u8) -> u16 {
    ((b0 as u16) << 8) | (b1 as u16)
}

fn parse_jpeg_size(bytes: &[u8]) -> Result<(u32, u32), String> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 || bytes[2] != 0xFF {
        return Err("jpeg: bad SOI".into());
    }

    let mut i = 2usize;
    while i + 1 < bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }

        while i < bytes.len() && bytes[i] == 0xFF {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        let marker = bytes[i];
        i += 1;

        if marker == 0xD9 || marker == 0xDA {
            break;
        }

        if marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }

        if i + 1 >= bytes.len() {
            break;
        }
        let seg_len = be_u16(bytes[i], bytes[i + 1]) as usize;
        if seg_len < 2 || i + seg_len > bytes.len() {
            return Err("jpeg: segment length out of bounds".into());
        }

        let is_sof = matches!(
            marker,
            0xC0 | 0xC1
                | 0xC2
                | 0xC3
                | 0xC5
                | 0xC6
                | 0xC7
                | 0xC9
                | 0xCA
                | 0xCB
                | 0xCD
                | 0xCE
                | 0xCF
        );

        if is_sof {
            let base = i + 2;
            if base + 6 > bytes.len() {
                return Err("jpeg: SOF truncated".into());
            }
            let height = be_u16(bytes[base + 1], bytes[base + 2]) as u32;
            let width = be_u16(bytes[base + 3], bytes[base + 4]) as u32;
            if width == 0 || height == 0 {
                return Err("jpeg: zero size".into());
            }
            return Ok((width, height));
        }

        i += seg_len;
    }

    Err("jpeg: SOF not found".into())
}

pub struct JpegProvider;

impl JpegProvider {
    #[inline]
    fn build_meta_json(width: u32, height: u32) -> String {
        format!(
            "{{\"schema\":\"kalitech.texture.meta.v1\",\"container\":\"jpeg\",\"width\":{width},\"height\":{height},\"depth\":1,\"mips\":1,\"is_cube\":false,\"format\":\"YCBCR\"}}"
        )
    }
}

impl ImageProviderV1 for JpegProvider {
    fn container(&self) -> &'static str {
        "jpeg"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["jpg", "jpeg"]
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF
    }

    fn import(&self, bytes: &[u8]) -> RResult<RVec<u8>, RString> {
        let (w, h) = match parse_jpeg_size(bytes) {
            Ok(v) => v,
            Err(e) => return err(e),
        };

        let meta = Self::build_meta_json(w, h);
        ok(pack(&meta, bytes))
    }

    fn describe_json(&self) -> &'static str {
        r#"{"container":"jpeg","extensions":["jpg","jpeg"],"sniff":"magic: FF D8 FF","method":"import_image_v1"}"#
    }
}

static PROVIDER: JpegProvider = JpegProvider;

inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
