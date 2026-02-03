#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{RResult, RString, RVec};
use abi_stable::StableAbi;

use newengine_plugin_api::{
    Blob, HostApiV1, MethodName, PluginInfo, PluginModule, ServiceV1, ServiceV1Dyn, ServiceV1_TO,
};

use std::io::Cursor;

/* =============================================================================================
   Binary frame helpers
   ============================================================================================= */

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
fn ok_blob(v: RVec<u8>) -> RResult<RVec<u8>, RString> {
    RResult::ROk(v)
}

#[inline]
fn err(msg: impl Into<String>) -> RResult<RVec<u8>, RString> {
    RResult::RErr(RString::from(msg.into()))
}

/* =============================================================================================
   PNG importer (plugin-owned schema)
   ============================================================================================= */

#[derive(Default)]
struct PngImporter;

impl PngImporter {
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

    fn build_meta_json(width: u32, height: u32, fmt: &str) -> String {
        // Same meta schema as DDS: kalitech.texture.meta.v1
        // PNG always has depth=1, mips=1, is_cube=false at container level.
        format!(
            "{{\"schema\":\"kalitech.texture.meta.v1\",\"container\":\"png\",\"width\":{width},\"height\":{height},\"depth\":1,\"mips\":1,\"is_cube\":false,\"format\":\"{fmt}\"}}"
        )
    }

    fn import_png_v1(bytes: &[u8]) -> RResult<RVec<u8>, RString> {
        let dec = png::Decoder::new(Cursor::new(bytes));
        let reader = match dec.read_info() {
            Ok(r) => r,
            Err(e) => return err(format!("png: read_info failed: {e}")),
        };

        let info = reader.info();
        let width = info.width;
        let height = info.height;

        let fmt = Self::fmt_string(info.color_type, info.bit_depth);
        let meta = Self::build_meta_json(width, height, &fmt);

        ok_blob(pack(&meta, bytes))
    }
}

/* =============================================================================================
   Service capability
   ============================================================================================= */

#[derive(StableAbi)]
#[repr(C)]
struct PngImporterService;

impl ServiceV1 for PngImporterService {
    fn id(&self) -> RString {
        RString::from("kalitech.import.png.v1")
    }

    fn describe(&self) -> RString {
        RString::from(
            r#"{
  "id":"kalitech.import.png.v1",
  "kind":"asset_importer",
  "asset_importer":{
    "extensions":["png"],
    "output_type_id":"kalitech.asset.texture",
    "format":"png",
    "method":"import_png_v1",
    "wire":"u32_meta_len_le + meta_utf8 + payload"
  },
  "methods":{
    "import_png_v1":{
      "in":"png bytes",
      "out":"[u32 meta_len_le][meta_json utf8][original png bytes]"
    }
  },
  "meta_schema":"kalitech.texture.meta.v1"
}"#,
        )
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            "import_png_v1" => {
                let bytes: Vec<u8> = payload.into_vec();
                PngImporter::import_png_v1(&bytes).map(|v| v)
            }
            _ => RResult::RErr(RString::from(format!(
                "png-importer: unknown method '{}'",
                method
            ))),
        }
    }
}

/* =============================================================================================
   Plugin module
   ============================================================================================= */

#[derive(Default)]
pub struct PngImporterPlugin;

impl PluginModule for PngImporterPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: RString::from("import.png"),
            name: RString::from("PNG Importer"),
            version: RString::from(env!("CARGO_PKG_VERSION")),
        }
    }

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString> {
        let svc: ServiceV1Dyn<'static> = ServiceV1_TO::from_value(PngImporterService, TD_Opaque);

        let r = (host.register_service_v1)(svc);
        if let Err(e) = r.clone().into_result() {
            (host.log_warn)(RString::from(format!(
                "png-importer: register_service_v1 failed: {}",
                e
            )));
        }
        r
    }

    fn start(&mut self) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn fixed_update(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn update(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn render(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn shutdown(&mut self) {}
}