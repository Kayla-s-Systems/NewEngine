#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{RResult, RString, RVec};
use abi_stable::StableAbi;

use newengine_plugin_api::{
    Blob, HostApiV1, MethodName, PluginInfo, PluginModule, ServiceV1, ServiceV1Dyn, ServiceV1_TO,
};

use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

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

/* =============================================================================================
Audio importer (plugin-owned schema)
============================================================================================= */

#[derive(Default)]
struct AudioImporter;

#[derive(Default)]
struct AudioMeta {
    container: String,
    codec: String,
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    frames: u64,
    duration_sec: f64,
}

impl AudioImporter {
    fn sniff_container(ext: &str) -> String {
        let e = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        if e.is_empty() {
            return "unknown".to_string();
        }
        e
    }

    fn build_meta_json(meta: &AudioMeta) -> String {
        let container = &meta.container;
        let codec = &meta.codec;
        let sample_rate = meta.sample_rate;
        let channels = meta.channels;
        let bits_per_sample = meta.bits_per_sample;
        let frames = meta.frames;
        let duration_sec = meta.duration_sec;

        format!(
            "{{\"schema\":\"kalitech.audio.meta.v1\",\"container\":\"{container}\",\"codec\":\"{codec}\",\"sample_rate\":{sample_rate},\"channels\":{channels},\"bits_per_sample\":{bits_per_sample},\"frames\":{frames},\"duration_sec\":{duration_sec}}}"
        )
    }

    fn probe_audio_meta(bytes: &[u8], ext_hint: &str) -> Result<AudioMeta, String> {
        let ext = Self::sniff_container(ext_hint);
        let mut hint = Hint::new();
        if ext != "unknown" {
            hint.with_extension(&ext);
        }

        let cursor = std::io::Cursor::new(bytes.to_vec());
        let mss = MediaSourceStream::new(Box::new(cursor), MediaSourceStreamOptions::default());

        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| format!("probe failed: {e}"))?;

        let format = probed.format;

        let track = format
            .default_track()
            .ok_or_else(|| "no default track".to_string())?;

        let params = &track.codec_params;

        let sample_rate = params.sample_rate.unwrap_or(0);
        let channels = params.channels.map(|c| c.count() as u16).unwrap_or(0);
        let bits_per_sample = params.bits_per_sample.unwrap_or(0) as u16;

        let frames = params.n_frames.unwrap_or(0);

        // В Symphonia поля называются time_base.num/den (в некоторых версиях)
        let duration_sec = match (params.n_frames, params.time_base) {
            (Some(nf), Some(tb)) if tb.denom > 0 => {
                (nf as f64) * (tb.numer as f64) / (tb.denom as f64)
            }
            _ => 0.0,
        };

        // FIX: codec — не Option
        let codec = format!("{:?}", params.codec);

        Ok(AudioMeta {
            container: ext,
            codec,
            sample_rate,
            channels,
            bits_per_sample,
            frames,
            duration_sec,
        })
    }

    fn import_audio_v1(bytes: &[u8], ext_hint: &str) -> RResult<RVec<u8>, RString> {
        let meta = match Self::probe_audio_meta(bytes, ext_hint) {
            Ok(m) => m,
            Err(_e) => {
                // Fallback: still produce a valid envelope.
                AudioMeta {
                    container: Self::sniff_container(ext_hint),
                    codec: "UNKNOWN".to_string(),
                    sample_rate: 0,
                    channels: 0,
                    bits_per_sample: 0,
                    frames: 0,
                    duration_sec: 0.0,
                }
            }
        };

        let meta_json = Self::build_meta_json(&meta);
        ok_blob(pack(&meta_json, bytes))
    }
}

/* =============================================================================================
Service capability
============================================================================================= */

#[derive(StableAbi)]
#[repr(C)]
struct AudioImporterService;

impl ServiceV1 for AudioImporterService {
    fn id(&self) -> RString {
        RString::from("kalitech.import.audio.v1")
    }

    fn describe(&self) -> RString {
        RString::from(
            r#"{
  "id":"kalitech.import.audio.v1",
  "kind":"asset_importer",
  "asset_importer":{
    "extensions":["wav","ogg","mp3","flac","aac","m4a"],
    "output_type_id":"kalitech.asset.audio",
    "format":"audio",
    "method":"import_audio_v1",
    "wire":"u32_meta_len_le + meta_utf8 + payload"
  },
  "methods":{
    "import_audio_v1":{
      "in":"audio bytes",
      "out":"[u32 meta_len_le][meta_json utf8][original bytes]"
    }
  },
  "meta_schema":"kalitech.audio.meta.v1"
}"#,
        )
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            "import_audio_v1" => {
                // Optional convention: allow "ext" hint in method name suffix:
                // "import_audio_v1:ogg" / "import_audio_v1:wav"
                // If not provided, importer still tries to probe content.
                let ext_hint = method
                    .as_str()
                    .split_once(':')
                    .map(|(_, ext)| ext)
                    .unwrap_or("unknown");

                let bytes: Vec<u8> = payload.into_vec();
                AudioImporter::import_audio_v1(&bytes, ext_hint).map(|v| v)
            }
            _ => RResult::RErr(RString::from(format!(
                "audio-importer: unknown method '{}'",
                method
            ))),
        }
    }
}

/* =============================================================================================
Plugin module
============================================================================================= */

#[derive(Default)]
pub struct AudioImporterPlugin;

impl PluginModule for AudioImporterPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: RString::from("import.audio"),
            name: RString::from("Audio Importer"),
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
