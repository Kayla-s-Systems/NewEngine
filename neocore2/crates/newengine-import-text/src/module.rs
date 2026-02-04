#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{RResult, RString, RVec};

use newengine_plugin_api::{
    Blob, HostApiV1, MethodName, PluginInfo, PluginModule, ServiceV1, ServiceV1Dyn, ServiceV1_TO,
};

use crate::providers::{self, TextMetaV1};

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
fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

#[inline]
fn meta_to_json(m: &TextMetaV1) -> String {
    format!(
        "{{\"schema\":\"kalitech.text.meta.v1\",\"container\":\"{}\",\"mime\":\"{}\",\"encoding\":\"{}\",\"is_utf8\":{}}}",
        m.container,
        escape_json_string(m.mime),
        escape_json_string(m.encoding),
        if m.is_utf8 { "true" } else { "false" }
    )
}
struct TextService {
    id: &'static str,
    provider: &'static dyn providers::TextProviderV1,
}

impl TextService {
    fn describe(provider: &'static dyn providers::TextProviderV1) -> RString {
        // Host читает JSON и сам биндит расширения.
        // priority=100, чтобы unified побеждал при дублях.
        // meta_schema фиксируем под текст.
        let mut exts_json = String::new();
        exts_json.push('[');
        for (i, e) in provider.extensions().iter().enumerate() {
            if i != 0 {
                exts_json.push(',');
            }
            exts_json.push('"');
            exts_json.push_str(e);
            exts_json.push('"');
        }
        exts_json.push(']');

        RString::from(format!(
            r#"{{
  "id":"{id}",
  "kind":"asset_importer",
  "asset_importer":{{
    "priority":100,
    "extensions":{exts},
    "output_type_id":"kalitech.asset.text",
    "format":"{container}",
    "method":"import_text_v1",
    "wire":"u32_meta_len_le + meta_utf8 + payload"
  }},
  "methods":{{
    "import_text_v1":{{"in":"bytes","out":"[u32 meta_len_le][meta_json utf8][original bytes]"}}
  }},
  "meta_schema":"kalitech.text.meta.v1",
  "provider":{provider_desc}
}}"#,
            id = provider.service_id(),
            exts = exts_json,
            container = provider.container(),
            provider_desc = provider.describe_json(),
        ))
    }
}

impl ServiceV1 for TextService {
    fn id(&self) -> RString {
        RString::from(self.id)
    }

    fn describe(&self) -> RString {
        Self::describe(self.provider)
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        let bytes: Vec<u8> = payload.into_vec();

        match method.as_str() {
            "import_text_v1" => {
                if !self.provider.sniff(&bytes) {
                    return err(format!(
                        "text: sniff failed for container '{}'",
                        self.provider.container()
                    ))
                    .map(|v| v);
                }

                let meta = self.provider.meta(&bytes);
                let meta_json = meta_to_json(&meta);
                ok(pack(&meta_json, &bytes)).map(|v| v)
            }
            _ => RResult::RErr(RString::from(format!(
                "text-importer({}): unknown method '{}'",
                self.id, method
            ))),
        }
    }
}

#[derive(Default)]
pub struct TextImporterPlugin;

impl PluginModule for TextImporterPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: RString::from("import.text"),
            name: RString::from("Text Importer (Provider-based)"),
            version: RString::from(env!("CARGO_PKG_VERSION")),
        }
    }

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString> {
        let mut registered = 0usize;

        for p in providers::iter_providers() {
            let svc = TextService {
                id: p.service_id(),
                provider: p,
            };

            let dyn_svc: ServiceV1Dyn<'static> = ServiceV1_TO::from_value(svc, TD_Opaque);

            let r = (host.register_service_v1)(dyn_svc);
            if let Err(e) = r.clone().into_result() {
                (host.log_warn)(RString::from(format!(
                    "text-importer: register_service_v1 failed for id='{}': {}",
                    p.service_id(),
                    e
                )));
                return r;
            }

            registered += 1;
        }

        if registered == 0 {
            (host.log_warn)(RString::from(
                "text-importer: no providers registered (inventory empty)".to_string(),
            ));
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
