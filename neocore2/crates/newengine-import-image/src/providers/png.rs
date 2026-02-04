use abi_stable::std_types::{RResult, RString, RVec};
use std::io::Cursor;

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
fn err(msg: impl Into<String>) -> RResult<RVec<u8>, RString> {
    RResult::RErr(RString::from(msg.into()))
}

#[inline]
fn ok(v: RVec<u8>) -> RResult<RVec<u8>, RString> {
    RResult::ROk(v)
}

pub struct PngProvider;

impl PngProvider {
    #[inline]
    fn fmt_string(color: png::ColorType, depth: png::BitDepth) -> String {
        let c = match color {
            png::ColorType::Grayscale => "GRAY",
            png::ColorType::Rgb => "RGB",
            png::ColorType::Indexed => "INDEXED",
            png::ColorType::GrayscaleAlpha => "GRAY_ALPHA",
            png::ColorType::Rgba => "RGBA",
        };
        let b = match depth {
            png::BitDepth::One => 1,
            png::BitDepth::Two => 2,
            png::BitDepth::Four => 4,
            png::BitDepth::Eight => 8,
            png::BitDepth::Sixteen => 16,
        };
        format!("{c}{b}")
    }

    #[inline]
    fn build_meta_json(width: u32, height: u32, fmt: &str) -> String {
        format!(
            "{{\"schema\":\"kalitech.texture.meta.v1\",\"container\":\"png\",\"width\":{width},\"height\":{height},\"depth\":1,\"mips\":1,\"is_cube\":false,\"format\":\"{fmt}\"}}"
        )
    }
}

impl ImageProviderV1 for PngProvider {
    fn container(&self) -> &'static str {
        "png"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["png"]
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        bytes.len() >= 8
            && bytes[0] == 0x89
            && bytes[1] == 0x50
            && bytes[2] == 0x4E
            && bytes[3] == 0x47
            && bytes[4] == 0x0D
            && bytes[5] == 0x0A
            && bytes[6] == 0x1A
            && bytes[7] == 0x0A
    }

    fn import(&self, bytes: &[u8]) -> RResult<RVec<u8>, RString> {
        let dec = png::Decoder::new(Cursor::new(bytes));
        let reader = match dec.read_info() {
            Ok(r) => r,
            Err(e) => return err(format!("png: read_info failed: {e}")),
        };

        let info = reader.info();
        let fmt = Self::fmt_string(info.color_type, info.bit_depth);
        let meta = Self::build_meta_json(info.width, info.height, &fmt);

        ok(pack(&meta, bytes))
    }

    fn describe_json(&self) -> &'static str {
        r#"{"container":"png","extensions":["png"],"sniff":"magic: 89 50 4E 47 ...","method":"import_image_v1"}"#
    }
}

static PROVIDER: PngProvider = PngProvider;

inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
