#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use libloading::Library;
use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use newengine_assets::{
    AssetBlob, AssetError, AssetKey, AssetStore, BlobImporterDispatch, ImporterPriority,
};
use newengine_plugin_api::{
    Blob, CapabilityId, EventSinkV1Dyn, HostApiV1, MethodName, PluginInfo, PluginModuleDyn,
    PluginRootV1Ref, ServiceV1Dyn,
};

/* =============================================================================================
   Host context (services registry + asset store)
   ============================================================================================= */

pub struct HostContext {
    services: Mutex<HashMap<String, ServiceV1Dyn<'static>>>,
    asset_store: Arc<AssetStore>,
}

static HOST_CTX: OnceLock<Arc<HostContext>> = OnceLock::new();

pub fn init_host_context(asset_store: Arc<AssetStore>) {
    let ctx = Arc::new(HostContext {
        services: Mutex::new(HashMap::new()),
        asset_store,
    });
    let _ = HOST_CTX.set(ctx);
}

#[inline]
fn ctx() -> Arc<HostContext> {
    HOST_CTX
        .get()
        .expect("HostContext not initialized (call init_host_context first)")
        .clone()
}

/* =============================================================================================
   Describe JSON contract (owned by plugin; host discovers)
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

/* =============================================================================================
   Service -> BlobImporterDispatch adapter
   Calls service via HostApiV1.call_service_v1 to avoid Clone/ABI crossing.
   Wire v1: [u32 meta_len_le][meta_json utf8][payload bytes]
   ============================================================================================= */

struct ServiceBlobImporter {
    stable_id: Arc<str>,
    exts: Vec<String>,
    output_type_id: Arc<str>,
    format: Arc<str>,
    method: Arc<str>,
    service_id: Arc<str>,
    priority: ImporterPriority,
}

impl ServiceBlobImporter {
    #[inline]
    fn call_import(&self, bytes: &[u8]) -> Result<Vec<u8>, AssetError> {
        let out: RResult<Blob, RString> = host_call_service_v1(
            CapabilityId::from(self.service_id.as_ref()),
            MethodName::from(self.method.as_ref()),
            Blob::from(bytes.to_vec()),
        );

        out.into_result()
            .map(|b| b.into_vec())
            .map_err(|e| AssetError::new(e.to_string()))
    }

    fn priority(&self) -> ImporterPriority {
        self.priority
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

fn try_auto_register_importer(service_id: &str, describe_json: &str) {
    let parsed: ServiceDescribe = match serde_json::from_str(describe_json) {
        Ok(v) => v,
        Err(_) => return,
    };

    if parsed.kind.as_deref() != Some("asset_importer") {
        return;
    }
    let Some(imp) = parsed.asset_importer else {
        return;
    };

    // Optional guard: if plugin declares a wire string, you can enforce a known one.
    // We keep it permissive for now.
    let _wire = imp.wire;

    let importer = ServiceBlobImporter {
        stable_id: Arc::from(service_id.to_string()),
        exts: imp.extensions,
        output_type_id: Arc::from(imp.output_type_id),
        format: Arc::from(imp.format),
        method: Arc::from(imp.method),
        service_id: Arc::from(service_id.to_string()),
        priority: ImporterPriority::new(imp.priority.unwrap_or(0)),
    };

    let ctx = ctx();
    ctx.asset_store.add_importer(Arc::new(importer));
    log::info!(target: "assets", "importer.auto_registered service_id='{}'", service_id);
}

/* =============================================================================================
   Host API (extern "C" ABI-safe)
   ============================================================================================= */

extern "C" fn host_log_info(s: RString) {
    log::info!("{}", s);
}

extern "C" fn host_log_warn(s: RString) {
    log::warn!("{}", s);
}

extern "C" fn host_log_error(s: RString) {
    log::error!("{}", s);
}

extern "C" fn host_register_service_v1(svc: ServiceV1Dyn<'static>) -> RResult<(), RString> {
    // Read before moving into registry; no Clone required.
    let service_id = svc.id().to_string();
    let describe_json = svc.describe().to_string();

    let ctx = ctx();

    {
        let mut g = match ctx.services.lock() {
            Ok(v) => v,
            Err(_) => return RResult::RErr(RString::from("services mutex poisoned")),
        };

        if g.contains_key(&service_id) {
            return RResult::RErr(RString::from(format!(
                "service already registered: {}",
                service_id
            )));
        }

        g.insert(service_id.clone(), svc);
    }

    try_auto_register_importer(&service_id, &describe_json);
    RResult::ROk(())
}

extern "C" fn host_call_service_v1(
    cap_id: CapabilityId,
    method: MethodName,
    payload: Blob,
) -> RResult<Blob, RString> {
    let id = cap_id.to_string();
    let ctx = ctx();

    let g = match ctx.services.lock() {
        Ok(v) => v,
        Err(_) => return RResult::RErr(RString::from("services mutex poisoned")),
    };

    let svc = match g.get(&id) {
        Some(v) => v,
        None => return RResult::RErr(RString::from(format!("service not found: {id}"))),
    };

    svc.call(method, payload)
}

extern "C" fn host_emit_event_v1(_topic: RString, _payload: Blob) -> RResult<(), RString> {
    RResult::ROk(())
}

extern "C" fn host_subscribe_events_v1(_sink: EventSinkV1Dyn<'static>) -> RResult<(), RString> {
    RResult::ROk(())
}

pub fn default_host_api() -> HostApiV1 {
    HostApiV1 {
        log_info: host_log_info,
        log_warn: host_log_warn,
        log_error: host_log_error,

        register_service_v1: host_register_service_v1,
        call_service_v1: host_call_service_v1,

        emit_event_v1: host_emit_event_v1,
        subscribe_events_v1: host_subscribe_events_v1,
    }
}

/* =============================================================================================
   Plugin manager (DLL loader + lifecycle)
   ============================================================================================= */

#[derive(Debug)]
pub struct PluginLoadError {
    pub path: PathBuf,
    pub message: String,
}

impl std::fmt::Display for PluginLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for PluginLoadError {}

struct LoadedPlugin {
    _lib: Library,
    module: PluginModuleDyn<'static>,
    info: PluginInfo,
    path: PathBuf,
}

pub struct PluginManager {
    loaded: Vec<LoadedPlugin>,
}

impl PluginManager {
    #[inline]
    pub fn new() -> Self {
        Self { loaded: Vec::new() }
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &PluginModuleDyn<'static>> {
        self.loaded.iter().map(|p| &p.module)
    }

    pub fn load_default(&mut self, host: HostApiV1) -> Result<(), PluginLoadError> {
        let dir = default_plugins_dir()?;
        self.load_from_dir(&dir, host)
    }

    pub fn load_from_dir(&mut self, dir: &Path, host: HostApiV1) -> Result<(), PluginLoadError> {
        let dir = resolve_plugins_dir(dir)?;
        log::info!("plugins: scanning directory '{}'", dir.display());

        let mut candidates = Vec::new();
        let rd = std::fs::read_dir(&dir).map_err(|e| PluginLoadError {
            path: dir.clone(),
            message: format!("read_dir failed: {e}"),
        })?;

        for ent in rd {
            let ent = ent.map_err(|e| PluginLoadError {
                path: dir.clone(),
                message: format!("read_dir entry failed: {e}"),
            })?;

            let p = ent.path();
            if !is_dynamic_lib(&p) {
                continue;
            }
            candidates.push(p);
        }

        candidates.sort();

        log::info!(
            "plugins: found {} candidate(s) in '{}'",
            candidates.len(),
            dir.display()
        );

        for path in candidates {
            match self.load_one(&path, host.clone()) {
                Ok(()) => {}
                Err(e) => {
                    log::warn!("plugins: failed to load '{}': {}", path.display(), e);
                }
            }
        }

        Ok(())
    }


    pub fn start_all(&mut self) -> Result<(), String> {
        for p in self.loaded.iter_mut() {
            if let Err(e) = p.module.start().into_result() {
                return Err(format!(
                    "plugin '{}' start failed: {}",
                    p.info.id, e
                ));
            }
        }
        Ok(())
    }

    pub fn fixed_update_all(&mut self, dt: f32) -> Result<(), String> {
        for p in self.loaded.iter_mut() {
            if let Err(e) = p.module.fixed_update(dt).into_result() {
                return Err(format!(
                    "plugin '{}' fixed_update failed: {}",
                    p.info.id, e
                ));
            }
        }
        Ok(())
    }

    pub fn update_all(&mut self, dt: f32) -> Result<(), String> {
        for p in self.loaded.iter_mut() {
            if let Err(e) = p.module.update(dt).into_result() {
                return Err(format!("plugin '{}' update failed: {}", p.info.id, e));
            }
        }
        Ok(())
    }

    pub fn render_all(&mut self, dt: f32) -> Result<(), String> {
        for p in self.loaded.iter_mut() {
            if let Err(e) = p.module.render(dt).into_result() {
                return Err(format!("plugin '{}' render failed: {}", p.info.id, e));
            }
        }
        Ok(())
    }

    pub fn shutdown(&mut self) {
        for p in self.loaded.iter_mut().rev() {
            p.module.shutdown();
        }
        self.loaded.clear();
    }

    fn load_one(&mut self, path: &Path, host: HostApiV1) -> Result<(), PluginLoadError> {
        log::info!("plugins: loading '{}'", path.display());

        let lib = unsafe { Library::new(path) }.map_err(|e| PluginLoadError {
            path: path.to_path_buf(),
            message: format!("Library::new failed: {e}"),
        })?;

        // ABI: PluginRootV1Ref::BASE_NAME == "export_plugin_root"
        let sym: libloading::Symbol<unsafe extern "C" fn() -> PluginRootV1Ref> =
            unsafe { lib.get(b"export_plugin_root\0") }.map_err(|e| PluginLoadError {
                path: path.to_path_buf(),
                message: format!("symbol export_plugin_root not found: {e}"),
            })?;

        let root = unsafe { sym() };
        let mut module = root.create()();

        let info = module.info();
        if let Err(e) = module.init(host).into_result() {
            return Err(PluginLoadError {
                path: path.to_path_buf(),
                message: format!("init failed: {e}"),
            });
        }

        log::info!(
            "plugins: loaded id='{}' ver='{}' from '{}'",
            info.id,
            info.version,
            path.display()
        );

        self.loaded.push(LoadedPlugin {
            _lib: lib,
            module,
            info,
            path: path.to_path_buf(),
        });

        Ok(())
    }
}

fn resolve_plugins_dir(dir: &Path) -> Result<PathBuf, PluginLoadError> {
    // 1) Empty path => default to exe directory.
    if dir.as_os_str().is_empty() {
        return default_plugins_dir();
    }

    // 2) "." is acceptable, but we still resolve it relative to exe dir for stability.
    let is_dot = dir == Path::new(".");

    // 3) Absolute path => use as-is.
    if dir.is_absolute() && !is_dot {
        return Ok(dir.to_path_buf());
    }

    // 4) Relative => resolve against exe dir (not cwd).
    let base = default_plugins_dir()?;
    if is_dot {
        return Ok(base);
    }

    Ok(base.join(dir))
}

fn is_dynamic_lib(p: &Path) -> bool {
    match p.extension().and_then(OsStr::to_str) {
        Some("dll") => true,
        Some("so") => true,
        Some("dylib") => true,
        _ => false,
    }
}

fn default_plugins_dir() -> Result<PathBuf, PluginLoadError> {
    let exe = std::env::current_exe().map_err(|e| PluginLoadError {
        path: PathBuf::new(),
        message: format!("current_exe failed: {e}"),
    })?;

    let dir = exe
        .parent()
        .ok_or_else(|| PluginLoadError {
            path: exe.clone(),
            message: "current_exe has no parent".to_string(),
        })?
        .to_path_buf();

    Ok(dir)
}