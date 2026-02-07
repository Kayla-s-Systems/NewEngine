use abi_stable::std_types::{RResult, RString, RVec};
use ddsfile::{Caps2, Dds};
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

pub struct DdsProvider;

impl DdsProvider {
    #[inline]
    fn build_meta_json(dds: &Dds) -> String {
        let width = dds.get_width();
        let height = dds.get_height();
        let depth = dds.get_depth();
        let mips = dds.get_num_mipmap_levels();
        let is_cube = dds.header.caps2.contains(Caps2::CUBEMAP);

        let fmt = if let Some(dxgi) = dds.get_dxgi_format() {
            format!("{:?}", dxgi)
        } else if let Some(d3d) = dds.get_d3d_format() {
            format!("{:?}", d3d)
        } else if let Some(fourcc) = dds.header.spf.fourcc.as_ref() {
            format!("{:?}", fourcc)
        } else {
            "UNKNOWN".to_string()
        };

        format!(
            "{{\"schema\":\"kalitech.texture.meta.v1\",\"container\":\"dds\",\"width\":{width},\"height\":{height},\"depth\":{depth},\"mips\":{mips},\"is_cube\":{is_cube},\"format\":\"{fmt}\"}}"
        )
    }
}

impl ImageProviderV1 for DdsProvider {
    fn container(&self) -> &'static str {
        "dds"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["dds"]
    }

    fn sniff(&self, bytes: &[u8]) -> bool {
        bytes.len() >= 4
            && bytes[0] == b'D'
            && bytes[1] == b'D'
            && bytes[2] == b'S'
            && bytes[3] == b' '
    }

    fn import(&self, bytes: &[u8]) -> RResult<RVec<u8>, RString> {
        let mut cur = Cursor::new(bytes);
        let dds = match Dds::read(&mut cur) {
            Ok(v) => v,
            Err(e) => return err(format!("dds: read failed: {e}")),
        };

        let meta = Self::build_meta_json(&dds);
        ok(pack(&meta, bytes))
    }

    fn describe_json(&self) -> &'static str {
        r#"{"container":"dds","extensions":["dds"],"sniff":"magic: DDS ","method":"import_image_v1"}"#
    }
}

static PROVIDER: DdsProvider = DdsProvider;

inventory::submit!(ProviderEntry {
    provider: &PROVIDER
});
