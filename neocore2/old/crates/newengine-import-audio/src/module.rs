#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{RResult, RString, RVec};
use abi_stable::StableAbi;

use newengine_plugin_api::{
    Blob, HostApiV1, MethodName, PluginInfo, PluginModule, ServiceV1, ServiceV1Dyn, ServiceV1_TO,
};

use std::sync::OnceLock;

use crate::providers::{self, AudioMetaV1};

/* =============================================================================================
Wire helpers: [u32 meta_len_le][meta_json utf8][payload bytes]
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
fn ok(v: RVec<u8>) -> RResult<RVec<u8>, RString> {
    RResult::ROk(v)
}

#[inline]
fn err(msg: impl Into<String>) -> RResult<RVec<u8>, RString> {
    RResult::RErr(RString::from(msg.into()))
}

#[inline]
fn build_meta_json(meta: &AudioMetaV1) -> String {
    format!(
        "{{\"schema\":\"kalitech.audio.meta.v1\",\"container\":\"{}\",\"codec\":\"{}\",\"sample_rate\":{},\"channels\":{},\"bits_per_sample\":{},\"frames\":{},\"duration_sec\":{}}}",
        meta.container,
        escape_json_string(&meta.codec),
        meta.sample_rate,
        meta.channels,
        meta.bits_per_sample,
        meta.frames,
        meta.duration_sec
    )
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

fn import_audio(bytes: &[u8], ext_hint: Option<&str>) -> RResult<RVec<u8>, RString> {
    if let Some(ext) = ext_hint {
        let e = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        if !e.is_empty() {
            for p in providers::iter_providers() {
                if p.extensions().iter().any(|&x| x.eq_ignore_ascii_case(&e)) {
                    match p.probe_meta(bytes) {
                        Ok(meta) => {
                            let meta_json = build_meta_json(&meta);
                            return ok(pack(&meta_json, bytes));
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    }

    for p in providers::iter_providers() {
        if p.sniff(bytes) {
            let meta = match p.probe_meta(bytes) {
                Ok(m) => m,
                Err(e) => return err(e),
            };
            let meta_json = build_meta_json(&meta);
            return ok(pack(&meta_json, bytes));
        }
    }

    for p in providers::iter_providers() {
        let meta = match p.probe_meta(bytes) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let meta_json = build_meta_json(&meta);
        return ok(pack(&meta_json, bytes));
    }

    err("audio: unsupported container")
}

#[derive(StableAbi)]
#[repr(C)]
struct AudioImporterService;

impl AudioImporterService {
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
  "id":"kalitech.import.audio.v1",
  "kind":"asset_importer",
  "asset_importer":{{
    "priority":100,
    "extensions":{exts_json},
    "output_type_id":"kalitech.asset.audio",
    "format":"audio",
    "method":"import_audio_v1",
    "wire":"u32_meta_len_le + meta_utf8 + payload",
    "formats":{formats_json}
  }},
  "methods":{{
    "import_audio_v1":{{"in":"audio bytes","out":"[u32 meta_len_le][meta_json utf8][original bytes]"}}
  }},
  "meta_schema":"kalitech.audio.meta.v1"
}}"#
            )
        })
            .as_str()
    }
}

impl ServiceV1 for AudioImporterService {
    fn id(&self) -> RString {
        RString::from("kalitech.import.audio.v1")
    }

    fn describe(&self) -> RString {
        RString::from(Self::describe_cached())
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        let bytes: Vec<u8> = payload.into_vec();

        match method.as_str() {
            "import_audio_v1" => import_audio(&bytes, None).map(|v| v),

            _ => {
                if let Some((base, ext)) = method.as_str().split_once(':') {
                    if base == "import_audio_v1" {
                        return import_audio(&bytes, Some(ext)).map(|v| v);
                    }
                }

                RResult::RErr(RString::from(format!(
                    "audio-importer: unknown method '{}'",
                    method
                )))
            }
        }
    }
}

#[derive(Default)]
pub struct AudioImporterPlugin;

impl PluginModule for AudioImporterPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: RString::from("import.audio"),
            name: RString::from("Audio Importer (Provider-based)"),
            version: RString::from(env!("CARGO_PKG_VERSION")),
        }
    }

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString> {
        let svc: ServiceV1Dyn<'static> = ServiceV1_TO::from_value(AudioImporterService, TD_Opaque);

        let r = (host.register_service_v1)(svc);
        if let Err(e) = r.clone().into_result() {
            (host.log_warn)(RString::from(format!(
                "audio-importer: register_service_v1 failed: {}",
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
