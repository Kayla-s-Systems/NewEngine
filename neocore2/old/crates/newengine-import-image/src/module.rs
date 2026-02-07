#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{RResult, RString, RVec};
use abi_stable::StableAbi;

use newengine_plugin_api::{
    Blob, HostApiV1, MethodName, PluginInfo, PluginModule, ServiceV1, ServiceV1Dyn, ServiceV1_TO,
};

use std::sync::OnceLock;

use crate::providers;

#[inline]
fn err(msg: impl Into<String>) -> RResult<RVec<u8>, RString> {
    RResult::RErr(RString::from(msg.into()))
}

#[derive(StableAbi)]
#[repr(C)]
struct ImageImporterService;

impl ImageImporterService {
    #[inline]
    fn import_auto(bytes: &[u8]) -> RResult<RVec<u8>, RString> {
        for p in providers::iter_providers() {
            if p.sniff(bytes) {
                return p.import(bytes);
            }
        }
        err("image: unsupported container")
    }

    fn describe_cached() -> &'static str {
        static CACHED: OnceLock<String> = OnceLock::new();
        CACHED.get_or_init(|| {
            let mut exts: Vec<&'static str> = Vec::new();
            let mut formats: Vec<&'static str> = Vec::new();

            for p in providers::iter_providers() {
                for &e in p.extensions() {
                    if !exts.iter().any(|&x| x == e) {
                        exts.push(e);
                    }
                }
                formats.push(p.describe_json());
            }

            let mut exts_json = String::new();
            exts_json.push('[');
            for (i, e) in exts.iter().enumerate() {
                if i != 0 {
                    exts_json.push(',');
                }
                exts_json.push('"');
                exts_json.push_str(e);
                exts_json.push('"');
            }
            exts_json.push(']');

            let mut formats_json = String::new();
            formats_json.push('[');
            for (i, f) in formats.iter().enumerate() {
                if i != 0 {
                    formats_json.push(',');
                }
                formats_json.push_str(f);
            }
            formats_json.push(']');

            format!(
                r#"{{
  "id":"kalitech.import.image.v1",
  "kind":"asset_importer",
  "asset_importer":{{
    "extensions":{exts_json},
    "output_type_id":"kalitech.asset.texture",
    "format":"image",
    "method":"import_image_v1",
    "wire":"u32_meta_len_le + meta_utf8 + payload",
    "formats":{formats_json}
  }},
  "methods":{{
    "import_image_v1":{{"in":"image bytes (auto sniff)","out":"[u32 meta_len_le][meta_json][payload]"}}
  }},
  "meta_schema":"kalitech.texture.meta.v1"
}}"#
            )
        })
            .as_str()
    }
}

impl ServiceV1 for ImageImporterService {
    fn id(&self) -> RString {
        RString::from("kalitech.import.image.v1")
    }

    fn describe(&self) -> RString {
        RString::from(Self::describe_cached())
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        let bytes: Vec<u8> = payload.into_vec();
        match method.as_str() {
            "import_image_v1" => Self::import_auto(&bytes).map(|v| v),
            _ => RResult::RErr(RString::from(format!(
                "image-importer: unknown method '{}'",
                method
            ))),
        }
    }
}

#[derive(Default)]
pub struct ImageImporterPlugin;

impl PluginModule for ImageImporterPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: RString::from("import.image"),
            name: RString::from("Image Importer (Provider-based)"),
            version: RString::from(env!("CARGO_PKG_VERSION")),
        }
    }

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString> {
        let svc: ServiceV1Dyn<'static> = ServiceV1_TO::from_value(ImageImporterService, TD_Opaque);

        let r = (host.register_service_v1)(svc);
        if let Err(e) = r.clone().into_result() {
            (host.log_warn)(RString::from(format!(
                "image-importer: register service failed: {}",
                e
            )));
            return r;
        }

        RResult::ROk(())
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
