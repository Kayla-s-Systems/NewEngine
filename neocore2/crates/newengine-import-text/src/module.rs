#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{RResult, RString, RVec};
use abi_stable::StableAbi;

use newengine_plugin_api::{
    Blob, HostApiV1, MethodName, PluginInfo, PluginModule, ServiceV1, ServiceV1Dyn, ServiceV1_TO,
};

use std::io::Cursor;

const META_SCHEMA_V1: &str = "kalitech.text.meta.v1";

/* =============================================================================================
   Binary frame helpers
   wire = [u32 meta_len_le][meta_json utf8][payload bytes]
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
   Text importer implementation
   ============================================================================================= */

#[derive(Default)]
struct TextImporter;

impl TextImporter {
    fn ensure_utf8(bytes: &[u8]) -> Result<(), String> {
        std::str::from_utf8(bytes).map(|_| ()).map_err(|e| format!("utf8: {e}"))
    }

    fn validate_json(bytes: &[u8]) -> Result<(), String> {
        Self::ensure_utf8(bytes)?;
        serde_json::from_slice::<serde_json::Value>(bytes)
            .map(|_| ())
            .map_err(|e| format!("json: {e}"))
    }

    fn validate_xml_bytes(bytes: &[u8]) -> Result<(), String> {
        Self::ensure_utf8(bytes)?;

        let mut r = quick_xml::Reader::from_reader(Cursor::new(bytes));
        let mut buf = Vec::new();

        loop {
            match r.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(_) => {}
                Err(e) => return Err(format!("xml: {e}")),
            }
            buf.clear();
        }

        Ok(())
    }

    fn build_meta_json(container: &str, byte_len: usize) -> String {
        format!(
            "{{\"schema\":\"{schema}\",\"container\":\"{container}\",\"encoding\":\"utf-8\",\"byte_len\":{byte_len}}}",
            schema = META_SCHEMA_V1,
            container = container,
            byte_len = byte_len
        )
    }

    fn import_text(bytes: &[u8], container: &str, validate: bool) -> RResult<RVec<u8>, RString> {
        if validate {
            let r = match container {
                "json" => Self::validate_json(bytes),
                "xml" | "ui" => Self::validate_xml_bytes(bytes),
                "html" | "txt" => Self::ensure_utf8(bytes),
                _ => Ok(()),
            };
            if let Err(e) = r {
                return err(e);
            }
        } else if let Err(e) = Self::ensure_utf8(bytes) {
            return err(e);
        }

        let meta = Self::build_meta_json(container, bytes.len());
        ok_blob(pack(&meta, bytes))
    }
}

/* =============================================================================================
   Services
   ============================================================================================= */

#[derive(StableAbi)]
#[repr(C)]
struct JsonImporterService;

impl ServiceV1 for JsonImporterService {
    fn id(&self) -> RString {
        RString::from("kalitech.import.json.v1")
    }

    fn describe(&self) -> RString {
        RString::from(
            r#"{
  "id":"kalitech.import.json.v1",
  "kind":"asset_importer",
  "asset_importer":{
    "extensions":["json"],
    "output_type_id":"kalitech.asset.text",
    "format":"json",
    "method":"import_json_v1",
    "wire":"u32_meta_len_le + meta_json_utf8 + payload"
  },
  "meta_schema":"kalitech.text.meta.v1",
  "container":"json",
  "validation":"serde_json"
}"#,
        )
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            "import_json_v1" => {
                let bytes: Vec<u8> = payload.into_vec();
                TextImporter::import_text(&bytes, "json", true).map(|v| v)
            }
            _ => RResult::RErr(RString::from(format!(
                "textimporter: unknown method '{}'",
                method
            ))),
        }
    }
}

#[derive(StableAbi)]
#[repr(C)]
struct XmlImporterService;

impl ServiceV1 for XmlImporterService {
    fn id(&self) -> RString {
        RString::from("kalitech.import.xml.v1")
    }

    fn describe(&self) -> RString {
        RString::from(
            r#"{
  "id":"kalitech.import.xml.v1",
  "kind":"asset_importer",
  "asset_importer":{
    "extensions":["xml"],
    "output_type_id":"kalitech.asset.text",
    "format":"xml",
    "method":"import_xml_v1",
    "wire":"u32_meta_len_le + meta_json_utf8 + payload"
  },
  "meta_schema":"kalitech.text.meta.v1",
  "container":"xml",
  "validation":"quick-xml"
}"#,
        )
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            "import_xml_v1" => {
                let bytes: Vec<u8> = payload.into_vec();
                TextImporter::import_text(&bytes, "xml", true).map(|v| v)
            }
            _ => RResult::RErr(RString::from(format!(
                "textimporter: unknown method '{}'",
                method
            ))),
        }
    }
}

#[derive(StableAbi)]
#[repr(C)]
struct UiImporterService;

impl ServiceV1 for UiImporterService {
    fn id(&self) -> RString {
        RString::from("kalitech.import.ui.v1")
    }

    fn describe(&self) -> RString {
        RString::from(
            r#"{
  "id":"kalitech.import.ui.v1",
  "kind":"asset_importer",
  "asset_importer":{
    "extensions":["ui"],
    "output_type_id":"kalitech.asset.text",
    "format":"ui",
    "method":"import_ui_v1",
    "wire":"u32_meta_len_le + meta_json_utf8 + payload"
  },
  "meta_schema":"kalitech.text.meta.v1",
  "container":"ui",
  "validation":"xml-subset (quick-xml)"
}"#,
        )
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            "import_ui_v1" => {
                let bytes: Vec<u8> = payload.into_vec();
                // UI markup must be well-formed XML subset => validate strictly
                TextImporter::import_text(&bytes, "ui", true).map(|v| v)
            }
            _ => RResult::RErr(RString::from(format!(
                "textimporter: unknown method '{}'",
                method
            ))),
        }
    }
}

#[derive(StableAbi)]
#[repr(C)]
struct HtmlImporterService;

impl ServiceV1 for HtmlImporterService {
    fn id(&self) -> RString {
        RString::from("kalitech.import.html.v1")
    }

    fn describe(&self) -> RString {
        RString::from(
            r#"{
  "id":"kalitech.import.html.v1",
  "kind":"asset_importer",
  "asset_importer":{
    "extensions":["html","htm"],
    "output_type_id":"kalitech.asset.text",
    "format":"html",
    "method":"import_html_v1",
    "wire":"u32_meta_len_le + meta_json_utf8 + payload"
  },
  "meta_schema":"kalitech.text.meta.v1",
  "container":"html",
  "validation":"utf8-only"
}"#,
        )
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            "import_html_v1" => {
                let bytes: Vec<u8> = payload.into_vec();
                TextImporter::import_text(&bytes, "html", false).map(|v| v)
            }
            _ => RResult::RErr(RString::from(format!(
                "textimporter: unknown method '{}'",
                method
            ))),
        }
    }
}

#[derive(StableAbi)]
#[repr(C)]
struct TxtImporterService;

impl ServiceV1 for TxtImporterService {
    fn id(&self) -> RString {
        RString::from("kalitech.import.txt.v1")
    }

    fn describe(&self) -> RString {
        RString::from(
            r#"{
  "id":"kalitech.import.txt.v1",
  "kind":"asset_importer",
  "asset_importer":{
    "extensions":["txt","md"],
    "output_type_id":"kalitech.asset.text",
    "format":"txt",
    "method":"import_txt_v1",
    "wire":"u32_meta_len_le + meta_json_utf8 + payload"
  },
  "meta_schema":"kalitech.text.meta.v1",
  "container":"txt",
  "validation":"utf8-only"
}"#,
        )
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            "import_txt_v1" => {
                let bytes: Vec<u8> = payload.into_vec();
                TextImporter::import_text(&bytes, "txt", false).map(|v| v)
            }
            _ => RResult::RErr(RString::from(format!(
                "textimporter: unknown method '{}'",
                method
            ))),
        }
    }
}

/* =============================================================================================
   Plugin module
   ============================================================================================= */

#[derive(Default)]
pub struct TextImporterPlugin;

impl PluginModule for TextImporterPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: RString::from("import.text"),
            name: RString::from("Text Importer"),
            version: RString::from(env!("CARGO_PKG_VERSION")),
        }
    }

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString> {
        let services: [ServiceV1Dyn<'static>; 5] = [
            ServiceV1_TO::from_value(JsonImporterService, TD_Opaque),
            ServiceV1_TO::from_value(XmlImporterService, TD_Opaque),
            ServiceV1_TO::from_value(UiImporterService, TD_Opaque),
            ServiceV1_TO::from_value(HtmlImporterService, TD_Opaque),
            ServiceV1_TO::from_value(TxtImporterService, TD_Opaque),
        ];

        for svc in services {
            let r = (host.register_service_v1)(svc);
            if let Err(e) = r.clone().into_result() {
                (host.log_warn)(RString::from(format!(
                    "textimporter: register_service_v1 failed: {}",
                    e
                )));
                return RResult::RErr(e);
            }
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