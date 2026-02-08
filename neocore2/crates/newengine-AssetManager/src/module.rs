#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{RResult, RString};
use abi_stable::StableAbi;

use newengine_plugin_api::{
    Blob, CapabilityId, HostApiV1, MethodName, PluginInfo, PluginModule, ServiceV1, ServiceV1Dyn,
    ServiceV1_TO,
};

use libloading::Library;
use serde::Deserialize;
use serde_json::json;

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use crate::source::FileSystemSource;
use crate::store::{AssetStore, BlobImporterDispatch, PumpBudget};
use crate::types::{AssetBlob, AssetError, AssetKey, AssetState, ImporterPriority};

/* =============================================================================================
   Global store (plugin-owned, not part of ABI)
   ============================================================================================= */

static ASSET_STORE: OnceLock<Arc<AssetStore>> = OnceLock::new();

#[inline]
fn asset_store() -> &'static Arc<AssetStore> {
    ASSET_STORE
        .get()
        .expect("assets: AssetStore not initialized")
}

/* =============================================================================================
   Service IDs / methods
   ============================================================================================= */

pub const ASSET_SERVICE_ID: &str = "asset.manager";

pub mod method {
    pub const LOAD: &str = "asset.load";
    pub const RELOAD: &str = "asset.reload";
    pub const PUMP: &str = "asset.pump";
    pub const INFO_JSON: &str = "asset.info_json";
    pub const STATE_JSON: &str = "asset.state_json";
    pub const BLOB_WIRE_V1: &str = "asset.blob_wire_v1";
}

/* =============================================================================================
   Describe parsing for importer services
   ============================================================================================= */

#[derive(Debug, Deserialize)]
struct ServiceDescribe {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    asset_importer: Option<AssetImporterDesc>,
}

#[derive(Debug, Deserialize)]
struct AssetImporterDesc {
    extensions: Vec<String>,
    output_type_id: String,
    format: String,
    method: String,
    #[serde(default)]
    priority: Option<i32>,
    #[serde(default)]
    wire: Option<String>,
}

#[inline]
fn parse_describe(describe_json: &str) -> Option<ServiceDescribe> {
    serde_json::from_str(describe_json).ok()
}

/* =============================================================================================
   Importer binding (service -> BlobImporterDispatch)
   ============================================================================================= */

struct ServiceBlobImporter {
    stable_id: Arc<str>,
    exts: Vec<String>,
    output_type_id: Arc<str>,
    format: Arc<str>,
    method: Arc<str>,
    service_id: Arc<str>,
    priority: ImporterPriority,
    host: HostApiV1,
}

impl ServiceBlobImporter {
    #[inline]
    fn call_import(&self, bytes: &[u8]) -> Result<Vec<u8>, AssetError> {
        let out = (self.host.call_service_v1)(
            CapabilityId::from(self.service_id.as_ref()),
            MethodName::from(self.method.as_ref()),
            Blob::from(bytes.to_vec()),
        );

        out.into_result()
            .map(|b| b.into_vec())
            .map_err(|e| AssetError::new(e.to_string()))
    }

    #[inline]
    fn unpack_wire_v1(frame: &[u8]) -> Result<(Arc<str>, Vec<u8>), AssetError> {
        if frame.len() < 4 {
            return Err(AssetError::new("importer wire v1: frame too small"));
        }

        let meta_len = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        let need = 4usize.saturating_add(meta_len);

        if frame.len() < need {
            return Err(AssetError::new("importer wire v1: truncated meta"));
        }

        let meta = &frame[4..4 + meta_len];
        let payload = frame[4 + meta_len..].to_vec();

        let meta_json = std::str::from_utf8(meta)
            .map_err(|_| AssetError::new("importer wire v1: meta is not utf8"))?
            .to_string();

        Ok((Arc::from(meta_json), payload))
    }
}

impl BlobImporterDispatch for ServiceBlobImporter {
    fn import_blob(&self, bytes: &[u8], _key: &AssetKey) -> Result<AssetBlob, AssetError> {
        let frame = self.call_import(bytes)?;
        let (meta_json, payload) = Self::unpack_wire_v1(&frame)?;

        Ok(AssetBlob {
            type_id: self.output_type_id.clone(),
            format: self.format.clone(),
            payload,
            meta_json,
            dependencies: Vec::new(),
        })
    }

    fn output_type_id(&self) -> Arc<str> {
        self.output_type_id.clone()
    }

    fn extensions(&self) -> Vec<String> {
        self.exts.clone()
    }

    fn priority(&self) -> ImporterPriority {
        self.priority
    }

    fn stable_id(&self) -> Arc<str> {
        self.stable_id.clone()
    }
}

/* =============================================================================================
   Asset manager service (ABI-safe: no non-StableAbi fields)
   ============================================================================================= */

#[derive(StableAbi)]
#[repr(C)]
struct AssetManagerService;

impl ServiceV1 for AssetManagerService {
    fn id(&self) -> CapabilityId {
        RString::from(ASSET_SERVICE_ID)
    }

    fn describe(&self) -> RString {
        let d = json!({
          "id": ASSET_SERVICE_ID,
          "version": 1,
          "methods": [
            { "name": method::LOAD, "payload": "utf8 logical_path", "returns": "json {ok,id_u128,error}" },
            { "name": method::RELOAD, "payload": "utf8 logical_path", "returns": "json {ok,id_u128,error}" },
            { "name": method::PUMP, "payload": "empty", "returns": "empty" },
            { "name": method::INFO_JSON, "payload": "utf8 logical_path", "returns": "json {ok,logical_path,id_u128,state,type_id,format,bytes,error}" },
            { "name": method::STATE_JSON, "payload": "utf8 id_u128_hex", "returns": "json {ok,state,error}" },
            { "name": method::BLOB_WIRE_V1, "payload": "utf8 id_u128_hex", "returns": "wire_v1(meta_json,payload)" }
          ]
        });

        RString::from(d.to_string())
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        let store = asset_store();
        let m = method.to_string();

        match m.as_str() {
            method::PUMP => {
                store.pump(PumpBudget::steps(16));
                RResult::ROk(Blob::from(Vec::new()))
            }

            method::LOAD | method::RELOAD => {
                let logical_path = String::from_utf8_lossy(payload.as_slice()).trim().to_string();
                if logical_path.is_empty() {
                    let bytes = serde_json::to_vec(&json!({
                        "ok": false,
                        "id_u128": null,
                        "error": "empty path"
                    }))
                        .unwrap_or_default();
                    return RResult::ROk(Blob::from(bytes));
                }

                let key = AssetKey::new(&logical_path, 0);
                let id_res = store.load(key);


                match id_res {
                    Ok(id) => {
                        let bytes = serde_json::to_vec(&json!({
                            "ok": true,
                            "id_u128": format!("{:032x}", id.to_u128()),
                            "error": null
                        }))
                            .unwrap_or_default();
                        RResult::ROk(Blob::from(bytes))
                    }
                    Err(e) => {
                        let bytes = serde_json::to_vec(&json!({
                            "ok": false,
                            "id_u128": null,
                            "error": e.to_string()
                        }))
                            .unwrap_or_default();
                        RResult::ROk(Blob::from(bytes))
                    }
                }
            }

            method::INFO_JSON => {
                let logical_path = String::from_utf8_lossy(payload.as_slice()).trim().to_string();
                if logical_path.is_empty() {
                    let bytes = serde_json::to_vec(&json!({"ok": false, "error": "empty path"}))
                        .unwrap_or_default();
                    return RResult::ROk(Blob::from(bytes));
                }

                let key = AssetKey::new(&logical_path, 0);
                let id = key.id();

                let st = store.state(id);
                let (state, err) = match st {
                    AssetState::Unloaded => ("unloaded".to_string(), None),
                    AssetState::Loading => ("loading".to_string(), None),
                    AssetState::Ready => ("ready".to_string(), None),
                    AssetState::Failed(e) => ("failed".to_string(), Some(e.to_string())),
                };

                let (type_id, format, bytes_len) = match store.get_blob(id) {
                    Some(b) => (
                        Some(b.type_id.to_string()),
                        Some(b.format.to_string()),
                        Some(b.payload.len() as u64),
                    ),
                    None => (None, None, None),
                };

                let bytes = serde_json::to_vec(&json!({
                    "ok": true,
                    "logical_path": logical_path,
                    "id_u128": format!("{:032x}", id.to_u128()),
                    "state": state,
                    "type_id": type_id,
                    "format": format,
                    "bytes": bytes_len,
                    "error": err,
                }))
                    .unwrap_or_default();

                RResult::ROk(Blob::from(bytes))
            }

            method::STATE_JSON => {
                let id_hex = String::from_utf8_lossy(payload.as_slice()).trim().to_string();
                let Some(id_u128) = parse_u128_hex_32(&id_hex) else {
                    let bytes = serde_json::to_vec(&json!({
                        "ok": false,
                        "state": "invalid",
                        "error": "bad id"
                    }))
                        .unwrap_or_default();
                    return RResult::ROk(Blob::from(bytes));
                };

                let id = crate::id::AssetId::from_u128(id_u128);
                let st = store.state(id);

                let (state, err) = match st {
                    AssetState::Unloaded => ("unloaded", None),
                    AssetState::Loading => ("loading", None),
                    AssetState::Ready => ("ready", None),
                    AssetState::Failed(e) => ("failed", Some(e.to_string())),
                };

                let bytes = serde_json::to_vec(&json!({
                    "ok": true,
                    "state": state,
                    "error": err
                }))
                    .unwrap_or_default();

                RResult::ROk(Blob::from(bytes))
            }

            method::BLOB_WIRE_V1 => {
                let id_hex = String::from_utf8_lossy(payload.as_slice()).trim().to_string();
                let Some(id_u128) = parse_u128_hex_32(&id_hex) else {
                    return RResult::RErr(RString::from("bad id"));
                };

                let id = crate::id::AssetId::from_u128(id_u128);

                let st = store.state(id);
                if !matches!(st, AssetState::Ready) {
                    return RResult::RErr(RString::from("asset not ready"));
                }

                let blob = match store.get_blob(id) {
                    Some(b) => b,
                    None => return RResult::RErr(RString::from("missing blob")),
                };

                let meta = blob.meta_json.as_bytes();
                let meta_len = (meta.len() as u32).to_le_bytes();

                let mut frame = Vec::with_capacity(4 + meta.len() + blob.payload.len());
                frame.extend_from_slice(&meta_len);
                frame.extend_from_slice(meta);
                frame.extend_from_slice(&blob.payload);

                RResult::ROk(Blob::from(frame))
            }

            _ => RResult::RErr(RString::from(format!("unknown method: {}", m))),
        }
    }
}

/* =============================================================================================
   Importer staging HostApi shim
   ============================================================================================= */

#[derive(Default)]
struct ImporterLoadState {
    staged: Vec<ServiceV1Dyn<'static>>,
}

thread_local! {
    static IMPORTER_LOAD_STATE: Cell<*mut ImporterLoadState> = const { Cell::new(std::ptr::null_mut()) };
}

fn with_importer_load_state<R>(state: &mut ImporterLoadState, f: impl FnOnce() -> R) -> R {
    IMPORTER_LOAD_STATE.with(|slot| {
        let prev = slot.replace(state as *mut _);
        let out = f();
        slot.set(prev);
        out
    })
}

extern "C" fn host_register_service_v1_importers(
    svc: ServiceV1Dyn<'static>,
) -> RResult<(), RString> {
    IMPORTER_LOAD_STATE.with(|slot| {
        let p = slot.get();
        if p.is_null() {
            return RResult::RErr(RString::from("importer loader: host state is not set"));
        }
        let st = unsafe { &mut *p };
        st.staged.push(svc);
        RResult::ROk(())
    })
}

/* =============================================================================================
   Plugin module
   ============================================================================================= */

#[derive(Default)]
pub struct AssetsPlugin {
    store: Option<Arc<AssetStore>>,
    budget_steps: u32,
    importers_dir: Option<PathBuf>,
    _importer_libs: Vec<Library>,
}

impl AssetsPlugin {
    #[inline]
    fn ensure_store(&mut self) -> Arc<AssetStore> {
        if let Some(s) = self.store.as_ref() {
            return s.clone();
        }

        let store = Arc::new(AssetStore::new());

        let root = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("assets");

        store.add_source(Arc::new(FileSystemSource::new(root)));

        self.store = Some(store.clone());
        store
    }

    #[inline]
    fn default_importers_dir() -> PathBuf {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        let base = exe
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("../../importers")
    }

    fn load_importers(&mut self, host: HostApiV1) {
        let dir = self
            .importers_dir
            .clone()
            .unwrap_or_else(Self::default_importers_dir);

        self.importers_dir = Some(dir.clone());
        let _ = std::fs::create_dir_all(&dir);

        let mut candidates: Vec<PathBuf> = match std::fs::read_dir(&dir) {
            Ok(rd) => rd.filter_map(|e| e.ok().map(|x| x.path())).collect(),
            Err(_) => Vec::new(),
        };

        candidates.sort();

        for path in candidates {
            if !is_dynamic_lib(&path) {
                continue;
            }

            match self.load_one_importer(&path, host.clone()) {
                Ok(()) => (host.log_info)(RString::from(format!(
                    "assets: importer loaded '{}'",
                    path.display()
                ))),
                Err(e) => (host.log_warn)(RString::from(format!(
                    "assets: importer failed '{}' err='{}'",
                    path.display(),
                    e
                ))),
            }
        }
    }

    fn load_one_importer(&mut self, path: &Path, host: HostApiV1) -> Result<(), String> {
        unsafe {
            let lib = Library::new(path).map_err(|e| e.to_string())?;

            let func: libloading::Symbol<
                unsafe extern "C" fn() -> newengine_plugin_api::PluginRootV1Ref,
            > = lib.get(b"export_plugin_root").map_err(|e| e.to_string())?;

            let root = func();
            let mut module = (root.create())();

            let mut st = ImporterLoadState::default();

            with_importer_load_state(&mut st, || {
                let mut host2 = host.clone();
                host2.register_service_v1 = host_register_service_v1_importers;
                let _ = module.init(host2);
            });

            for svc in st.staged.drain(..) {
                let service_id = svc.id().to_string();
                let describe_json = svc.describe().to_string();

                let r = (host.register_service_v1)(svc);
                if let Err(e) = r.into_result() {
                    return Err(e.to_string());
                }

                self.try_bind_importer(&service_id, &describe_json, host.clone());
            }

            self._importer_libs.push(lib);
            Ok(())
        }
    }

    fn try_bind_importer(&mut self, service_id: &str, describe_json: &str, host: HostApiV1) {
        let Some(d) = parse_describe(describe_json) else {
            return;
        };
        if d.kind.as_deref() != Some("asset_importer") {
            return;
        }
        let Some(imp) = d.asset_importer else {
            return;
        };

        let importer = ServiceBlobImporter {
            stable_id: Arc::from(service_id.to_string()),
            exts: imp.extensions,
            output_type_id: Arc::from(imp.output_type_id),
            format: Arc::from(imp.format),
            method: Arc::from(imp.method),
            service_id: Arc::from(service_id.to_string()),
            priority: ImporterPriority::new(imp.priority.unwrap_or(0)),
            host,
        };

        let store = self.ensure_store();
        store.add_importer(Arc::new(importer));
    }
}

impl PluginModule for AssetsPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: RString::from(env!("CARGO_PKG_NAME")),
            name: RString::from("NewEngine Assets"),
            version: RString::from(env!("CARGO_PKG_VERSION")),
        }
    }

    fn init(&mut self, host: HostApiV1) -> RResult<(), RString> {
        let store = self.ensure_store();
        let _ = ASSET_STORE.set(store);

        let svc: ServiceV1Dyn<'static> = ServiceV1_TO::from_value(AssetManagerService, TD_Opaque);

        if let Err(e) = (host.register_service_v1)(svc).into_result() {
            return RResult::RErr(RString::from(format!(
                "assets: register_service_v1 failed: {}",
                e
            )));
        }

        self.load_importers(host.clone());

        (host.log_info)(RString::from("assets: initialized"));
        RResult::ROk(())
    }

    fn start(&mut self) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn fixed_update(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn update(&mut self, _dt: f32) -> RResult<(), RString> {
        let steps = self.budget_steps.max(1).max(16);
        asset_store().pump(PumpBudget::steps(steps));
        RResult::ROk(())
    }

    fn render(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn shutdown(&mut self) {}
}

/* =============================================================================================
   Helpers
   ============================================================================================= */

fn is_dynamic_lib(p: &Path) -> bool {
    let Some(ext) = p.extension().and_then(|s| s.to_str()) else {
        return false;
    };
    let ext = ext.to_ascii_lowercase();
    ext == "dll" || ext == "so" || ext == "dylib"
}

fn parse_u128_hex_32(s: &str) -> Option<u128> {
    let t = s.trim();
    if t.len() != 32 {
        return None;
    }
    u128::from_str_radix(t, 16).ok()
}