#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{RResult, RString, RVec};
use abi_stable::StableAbi;
use ddsfile::{Caps2, Dds};

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
   DDS importer (plugin-owned schema)
   ============================================================================================= */

#[derive(Default)]
struct DdsImporter;

impl DdsImporter {
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

    fn import_dds_v1(bytes: &[u8]) -> RResult<RVec<u8>, RString> {
        let mut cur = Cursor::new(bytes);
        let dds = match Dds::read(&mut cur) {
            Ok(v) => v,
            Err(e) => return err(format!("dds: read failed: {e}")),
        };

        let meta = Self::build_meta_json(&dds);
        ok_blob(pack(&meta, bytes))
    }
}

/* =============================================================================================
   Service capability
   ============================================================================================= */

#[derive(StableAbi)]
#[repr(C)]
struct DdsImporterService;

impl ServiceV1 for DdsImporterService {
    fn id(&self) -> RString {
        RString::from("kalitech.import.dds.v1")
    }

    fn describe(&self) -> RString {
        RString::from(
            r#"{
  "id":"kalitech.import.dds.v1",
  "kind":"asset_importer",
  "asset_importer":{
    "extensions":["dds"],
    "output_type_id":"kalitech.asset.texture",
    "format":"dds",
    "method":"import_dds_v1",
    "wire":"u32_meta_len_le + meta_utf8 + payload"
  },
  "methods":{
    "import_dds_v1":{
      "in":"dds bytes",
      "out":"[u32 meta_len_le][meta_json utf8][original dds bytes]"
    }
  },
  "meta_schema":"kalitech.texture.meta.v1"
}"#,
        )
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            "import_dds_v1" => {
                let bytes: Vec<u8> = payload.into_vec();
                DdsImporter::import_dds_v1(&bytes).map(|v| v)
            }
            _ => RResult::RErr(RString::from(format!(
                "dds-importer: unknown method '{}'",
                method
            ))),
        }
    }
}

/* =============================================================================================
   Plugin module
   ============================================================================================= */

#[derive(Default)]
pub struct DdsImporterPlugin;

impl PluginModule for DdsImporterPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: RString::from("import.dds"),
            name: RString::from("DDS Importer"),
            version: RString::from(env!("CARGO_PKG_VERSION")),
        }
    }

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString> {
        let svc: ServiceV1Dyn<'static> = ServiceV1_TO::from_value(DdsImporterService, TD_Opaque);

        let r = (host.register_service_v1)(svc);
        if let Err(e) = r.clone().into_result() {
            (host.log_warn)(RString::from(format!(
                "dds-importer: register_service_v1 failed: {}",
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