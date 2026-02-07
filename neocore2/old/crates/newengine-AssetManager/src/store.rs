use crate::events::AssetEvent;
use crate::id::AssetId;
use crate::source::AssetSource;
use crate::types::{AssetBlob, AssetError, AssetKey, AssetState, ImporterPriority};
use log::{debug, info, warn};
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub struct PumpBudget {
    pub steps: u32,
}

impl PumpBudget {
    #[inline]
    pub fn steps(steps: u32) -> Self {
        Self { steps }
    }
}

/// Universal importer callback owned by the host (core).
///
/// Core wraps plugin/DLL importers and exposes them here.
pub trait BlobImporterDispatch: Send + Sync + 'static {
    fn import_blob(&self, bytes: &[u8], key: &AssetKey) -> Result<AssetBlob, AssetError>;

    fn output_type_id(&self) -> Arc<str>;
    fn extensions(&self) -> Vec<String>;

    /// Host-defined priority. Higher wins.
    fn priority(&self) -> ImporterPriority {
        ImporterPriority::new(0)
    }

    /// Stable identifier for tie-break and diagnostics (e.g. "dds_importer@plugin:render").
    fn stable_id(&self) -> Arc<str>;
}

struct PendingRequest {
    id: AssetId,
    key: AssetKey,
    type_id: Arc<str>,
    importer: Arc<dyn BlobImporterDispatch>,
    importer_id: Arc<str>,
}

impl std::fmt::Debug for PendingRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingRequest")
            .field("id", &self.id)
            .field("key", &self.key)
            .field("type_id", &self.type_id)
            .field("importer_id", &self.importer_id)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct ImporterBindingInfo {
    pub ext: String,
    pub stable_id: Arc<str>,
    pub output_type_id: Arc<str>,
    pub priority: ImporterPriority,
}

#[derive(Default, Debug, Clone)]
struct AssetDiagnostics {
    pump_total: u64,
    pump_success: u64,
    pump_failed: u64,
    bytes_read: u64,
    io_time_us: u64,
    import_time_us: u64,
}

impl AssetDiagnostics {
    #[inline]
    fn reset_frame(&mut self) {
        self.pump_total = 0;
        self.pump_success = 0;
        self.pump_failed = 0;
        self.bytes_read = 0;
        self.io_time_us = 0;
        self.import_time_us = 0;
    }
}

#[derive(Default)]
struct StoreInner {
    sources: Vec<Arc<dyn AssetSource>>,
    importers_by_ext: HashMap<String, Vec<Arc<dyn BlobImporterDispatch>>>,
    state: HashMap<AssetId, AssetState>,
    blobs: HashMap<AssetId, Arc<AssetBlob>>,
    queue: VecDeque<PendingRequest>,
    events: VecDeque<AssetEvent>,
    diag: AssetDiagnostics,
}

#[derive(Default)]
pub struct AssetStore {
    inner: Mutex<StoreInner>,
}

impl AssetStore {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn add_source(&self, source: Arc<dyn AssetSource>) {
        let mut g = self.inner.lock();
        g.sources.push(source);
    }

    pub fn add_importer(&self, importer: Arc<dyn BlobImporterDispatch>) {
        let exts = importer.extensions();
        let type_id = importer.output_type_id();
        let priority = importer.priority();
        let stable_id = importer.stable_id();

        info!(
            target: "assets",
            "importer.register id='{}' type='{}' priority={}",
            stable_id,
            type_id,
            priority.0
        );

        let mut g = self.inner.lock();

        for ext in exts {
            let norm_ext = normalize_ext(&ext);

            info!(target: "assets", "importer.bind id='{}' ext='.{}'", stable_id, norm_ext);

            let list = g.importers_by_ext.entry(norm_ext).or_default();
            list.push(importer.clone());

            list.sort_by(|a, b| {
                let pa = a.priority();
                let pb = b.priority();
                pb.cmp(&pa).then_with(|| a.stable_id().cmp(&b.stable_id()))
            });
        }
    }

    /// Returns a snapshot of registered importer bindings.
    ///
    /// Intended for diagnostics/UI; avoids exposing internal storage structures.
    pub fn importer_bindings(&self) -> Vec<ImporterBindingInfo> {
        let g = self.inner.lock();
        let mut out = Vec::new();
        for (ext, list) in g.importers_by_ext.iter() {
            for imp in list.iter() {
                out.push(ImporterBindingInfo {
                    ext: ext.clone(),
                    stable_id: imp.stable_id(),
                    output_type_id: imp.output_type_id(),
                    priority: imp.priority(),
                });
            }
        }
        out.sort_by(|a, b| {
            a.ext
                .cmp(&b.ext)
                .then_with(|| b.priority.cmp(&a.priority))
                .then_with(|| a.stable_id.cmp(&b.stable_id))
        });
        out
    }

    #[inline]
    pub fn state(&self, id: AssetId) -> AssetState {
        let g = self.inner.lock();
        g.state.get(&id).cloned().unwrap_or(AssetState::Unloaded)
    }

    #[inline]
    pub fn get_blob(&self, id: AssetId) -> Option<Arc<AssetBlob>> {
        let g = self.inner.lock();
        g.blobs.get(&id).cloned()
    }

    #[inline]
    pub fn drain_events(&self) -> Vec<AssetEvent> {
        let mut g = self.inner.lock();
        g.events.drain(..).collect()
    }

    pub fn load(&self, key: AssetKey) -> Result<AssetId, AssetError> {
        let id = key.id();

        info!(
            target: "assets",
            "asset.load request id={:032x} path='{}'",
            id.to_u128(),
            key.logical_path.display()
        );

        let mut g = self.inner.lock();
        match g.state.get(&id) {
            Some(AssetState::Ready) | Some(AssetState::Loading) | Some(AssetState::Failed(_)) => {
                return Ok(id)
            }
            _ => {}
        }

        let ext = extension_ascii_lower(&key.logical_path)
            .ok_or_else(|| AssetError::new("AssetStore: asset path has no extension"))?;

        let Some(list) = g.importers_by_ext.get(&ext) else {
            warn!(
                target: "assets",
                "asset.load rejected id={:032x} path='{}' reason='no_importer' ext='{}'",
                id.to_u128(),
                key.logical_path.display(),
                ext
            );
            return Err(AssetError::new(format!(
                "AssetStore: no importer registered for extension '.{}'",
                ext
            )));
        };

        let Some(importer) = list.first().cloned() else {
            warn!(
                target: "assets",
                "asset.load rejected id={:032x} path='{}' reason='no_importer' ext='{}'",
                id.to_u128(),
                key.logical_path.display(),
                ext
            );
            return Err(AssetError::new(format!(
                "AssetStore: no importer registered for extension '.{}'",
                ext
            )));
        };

        let type_id = importer.output_type_id();

        info!(
            target: "assets::import",
            "importer.select path='{}' ext='.{}' winner='{}'",
            key.logical_path.display(),
            ext,
            importer.stable_id()
        );

        g.state.insert(id, AssetState::Loading);
        let importer_id = importer.stable_id();
        g.queue.push_back(PendingRequest {
            id,
            key,
            type_id,
            importer,
            importer_id,
        });

        Ok(id)
    }

    pub fn pump(&self, budget: PumpBudget) {
        {
            let mut g = self.inner.lock();
            g.diag.reset_frame();
        }

        let pump_t0 = Instant::now();
        let mut steps_left = budget.steps;

        while steps_left > 0 {
            steps_left -= 1;

            let req = {
                let mut g = self.inner.lock();
                g.queue.pop_front()
            };

            let Some(req) = req else { break; };

            {
                let mut g = self.inner.lock();
                g.diag.pump_total += 1;
            }

            if let Err(err) = self.process_one(req) {
                {
                    let mut g = self.inner.lock();
                    g.diag.pump_failed += 1;
                    g.state.insert(err.id, AssetState::Failed(err.error.clone()));
                    g.events.push_back(AssetEvent::Failed {
                        id: err.id,
                        type_id: err.type_id.clone(),
                        error: err.error.clone(),
                    });
                }

                warn!(
                    target: "assets::events",
                    "asset.failed id={:032x} type='{}' error='{}'",
                    err.id.to_u128(),
                    err.type_id,
                    err.error
                );
            }
        }

        let dt = pump_t0.elapsed();
        let (total, ok, fail, bytes, io_us, imp_us) = {
            let g = self.inner.lock();
            (
                g.diag.pump_total,
                g.diag.pump_success,
                g.diag.pump_failed,
                g.diag.bytes_read,
                g.diag.io_time_us,
                g.diag.import_time_us,
            )
        };

        if total > 0 {
            info!(
                target: "assets",
                "pump.summary total={} ok={} fail={} bytes={} io_us={} import_us={} frame_ms={:.3}",
                total,
                ok,
                fail,
                bytes,
                io_us,
                imp_us,
                dt.as_secs_f64() * 1000.0
            );
        }
    }

    fn process_one(&self, req: PendingRequest) -> Result<(), ProcessError> {
        let sources = {
            let g = self.inner.lock();
            g.sources.clone()
        };

        let importer = req.importer;

        let io_t0 = Instant::now();
        let bytes = read_from_any_source_list(&sources, &req.key.logical_path).map_err(|e| {
            ProcessError {
                id: req.id,
                type_id: req.type_id.clone(),
                error: Arc::from(e.msg().to_string()),
            }
        })?;
        let io_dt = io_t0.elapsed();

        {
            let mut g = self.inner.lock();
            g.diag.bytes_read += bytes.len() as u64;
            g.diag.io_time_us += io_dt.as_micros() as u64;
        }

        debug!(
            target: "assets::io",
            "io.read id={:032x} path='{}' bytes={} dt_us={}",
            req.id.to_u128(),
            req.key.logical_path.display(),
            bytes.len(),
            io_dt.as_micros()
        );

        let imp_t0 = Instant::now();
        let blob = importer.import_blob(&bytes, &req.key).map_err(|e| ProcessError {
            id: req.id,
            type_id: req.type_id.clone(),
            error: Arc::from(e.msg().to_string()),
        })?;
        let imp_dt = imp_t0.elapsed();

        {
            let mut g = self.inner.lock();
            g.diag.import_time_us += imp_dt.as_micros() as u64;
        }

        debug!(
            target: "assets::import",
            "import.done id={:032x} importer='{}' type='{}' format='{}' payload={} dt_us={}",
            req.id.to_u128(),
            importer.stable_id(),
            blob.type_id,
            blob.format,
            blob.payload.len(),
            imp_dt.as_micros()
        );

        let format = blob.format.clone();
        let blob = Arc::new(blob);

        {
            let mut g = self.inner.lock();
            g.diag.pump_success += 1;
            g.blobs.insert(req.id, blob);
            g.state.insert(req.id, AssetState::Ready);
            g.events.push_back(AssetEvent::Ready {
                id: req.id,
                type_id: req.type_id.clone(),
                format: format.clone(),
            });
        }

        info!(
            target: "assets::events",
            "asset.ready id={:032x} type='{}' format='{}' path='{}'",
            req.id.to_u128(),
            req.type_id,
            format,
            req.key.logical_path.display()
        );

        Ok(())
    }
}

#[derive(Debug)]
struct ProcessError {
    id: AssetId,
    type_id: Arc<str>,
    error: Arc<str>,
}

#[inline]
fn extension_ascii_lower(p: &Path) -> Option<String> {
    let ext = p.extension()?.to_string_lossy();
    if ext.is_empty() {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}

#[inline]
fn normalize_ext(ext: &str) -> String {
    ext.trim().trim_start_matches('.').to_ascii_lowercase()
}

#[inline]
fn read_from_any_source_list(
    sources: &[Arc<dyn AssetSource>],
    logical_path: &Path,
) -> Result<Vec<u8>, AssetError> {
    if sources.is_empty() {
        return Err(AssetError::new("AssetStore: no sources registered"));
    }

    for s in sources {
        if s.exists(logical_path) {
            return s.read(logical_path);
        }
    }

    Err(AssetError::new(format!(
        "AssetStore: asset not found in any source: '{}'",
        logical_path.to_string_lossy()
    )))
}

/// Utility: stable single-line preview for logs/UI.
///
/// - Limits by char count (not bytes)
/// - Replaces '\r' with "" (elide) and '\n' with "\n"
/// - Escapes non-printable characters
#[inline]
pub fn preview_single_line_escaped(s: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(s.len().min(max_chars * 2).max(32));
    let mut count = 0usize;

    for ch in s.chars() {
        if count >= max_chars {
            out.push_str("…");
            break;
        }
        count += 1;

        match ch {
            '\r' => {
                // drop CR to avoid console artifacts
            }
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                // escape as \u{..}
                use std::fmt::Write;
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }

    out
}

#[derive(Debug, Clone)]
pub struct AssetStoreStats {
    pub sources: usize,
    pub importers: usize,
    pub importers_bindings: usize,
    pub state_entries: usize,
    pub blobs_ready: usize,
    pub blobs_bytes: u64,
    pub queue_len: usize,
}

#[derive(Debug, Clone)]
pub struct AssetEntrySnapshot {
    pub id_u128: u128,
    pub path: String,
    pub state: String,
    pub type_id: Option<String>,
    pub format: Option<String>,
    pub bytes: Option<u64>,
}

impl AssetStore {
    /// Returns a snapshot of basic store statistics for console/UI.
    pub fn stats_snapshot(&self) -> AssetStoreStats {
        let g = self.inner.lock();

        let sources = g.sources.len();
        let importers = g.importers_by_ext.values().map(|v| v.len()).sum::<usize>();
        let importers_bindings = g
            .importers_by_ext
            .iter()
            .map(|(k, v)| (k.len(), v.len()))
            .count();

        let state_entries = g.state.len();
        let blobs_ready = g.blobs.len();
        let blobs_bytes = g
            .blobs
            .values()
            .map(|b| b.payload.len() as u64)
            .sum::<u64>();

        let queue_len = g.queue.len();

        AssetStoreStats {
            sources,
            importers,
            importers_bindings,
            state_entries,
            blobs_ready,
            blobs_bytes,
            queue_len,
        }
    }

    /// Lists assets currently known to the store (state map), limited by `limit`.
    /// Intended for console/UI; avoids exposing internal types.
    pub fn list_snapshot(&self, limit: usize) -> Vec<AssetEntrySnapshot> {
        let g = self.inner.lock();

        let mut out = Vec::new();
        out.reserve(g.state.len().min(limit));

        for (id, st) in g.state.iter().take(limit) {
            let id_u128 = id.to_u128();

            let (type_id, format, bytes) = match g.blobs.get(id) {
                Some(blob) => (
                    Some(blob.type_id.to_string()),
                    Some(blob.format.to_string()),
                    Some(blob.payload.len() as u64),
                ),
                None => (None, None, None),
            };

            let state_str = match st {
                crate::types::AssetState::Unloaded => "unloaded".to_string(),
                crate::types::AssetState::Loading => "loading".to_string(),
                crate::types::AssetState::Ready => "ready".to_string(),
                crate::types::AssetState::Failed(e) => format!("failed: {}", e),
            };

            // Path is not stored in the state map. We can only show id-based entry here.
            // The canonical path is available at load-time via AssetKey, but not retained.
            // For console use, we support list by id and state; path is available via commands like asset.load/reload.
            out.push(AssetEntrySnapshot {
                id_u128,
                path: String::new(),
                state: state_str,
                type_id,
                format,
                bytes,
            });
        }

        out
    }

    /// Convenience: enqueue load by logical path with settings_hash=0.
    pub fn load_path(&self, logical_path: &str) -> Result<crate::id::AssetId, crate::types::AssetError> {
        let key = AssetKey::new(logical_path, 0);
        self.load(key)
    }

    /// Convenience: attempt "reload" semantics:
    /// - mark asset Unloaded and drop cached blob (if any)
    /// - enqueue new load
    pub fn reload_path(&self, logical_path: &str) -> Result<crate::id::AssetId, crate::types::AssetError> {
        let key = AssetKey::new(logical_path, 0);
        let id = key.id();

        {
            let mut g = self.inner.lock();
            g.blobs.remove(&id);
            g.state.insert(id, crate::types::AssetState::Unloaded);
        }

        self.load(key)
    }

    /// Returns the current queue length (for console/UI).
    #[inline]
    pub fn queue_len(&self) -> usize {
        let g = self.inner.lock();
        g.queue.len()
    }
}
