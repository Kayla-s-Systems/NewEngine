use crate::events::AssetEvent;
use crate::id::AssetId;
use crate::importer::{AnyImporter, ImporterBox, Importer};
use crate::source::AssetSource;
use crate::types::{Asset, AssetError, AssetKey, AssetRef, AssetState, Handle};
use parking_lot::Mutex;
use std::any::{Any, TypeId};
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::Arc;

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

#[derive(Debug)]
struct PendingRequest {
    id: AssetId,
    key: AssetKey,
    type_id: TypeId,
    type_name: &'static str,
}

#[derive(Default)]
struct StoreInner {
    sources: Vec<Arc<dyn AssetSource>>,
    importers: Vec<Arc<dyn AnyImporter>>,
    ext_index: HashMap<&'static str, Vec<usize>>,

    state: HashMap<AssetId, AssetState>,
    // type_id -> (asset_id -> Arc<Any>)
    loaded: HashMap<TypeId, HashMap<AssetId, Arc<dyn Any + Send + Sync>>>,

    queue: VecDeque<PendingRequest>,
    events: VecDeque<AssetEvent>,
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

    #[inline]
    pub fn add_importer<T: Asset>(&self, importer: Box<dyn Importer<T>>) {
        let mut g = self.inner.lock();
        let idx = g.importers.len();
        let boxed: Arc<dyn AnyImporter> = Arc::new(ImporterBox::<T>::new(importer));
        for &ext in boxed.supported_extensions() {
            g.ext_index.entry(ext).or_default().push(idx);
        }
        g.importers.push(boxed);
    }

    #[inline]
    pub fn state(&self, id: AssetId) -> AssetState {
        let g = self.inner.lock();
        g.state.get(&id).cloned().unwrap_or(AssetState::Unloaded)
    }

    #[inline]
    pub fn drain_events(&self) -> Vec<AssetEvent> {
        let mut g = self.inner.lock();
        g.events.drain(..).collect()
    }

    /// Enqueue load request and return handle immediately.
    /// Loading is progressed via `pump`.
    pub fn load<T: Asset>(&self, key: AssetKey) -> Result<Handle<T>, AssetError> {
        let id = key.id();
        let type_id = TypeId::of::<T>();

        let mut g = self.inner.lock();

        match g.state.get(&id) {
            Some(AssetState::Ready) => return Ok(Handle::new(id)),
            Some(AssetState::Loading) => return Ok(Handle::new(id)),
            Some(AssetState::Failed(_)) => return Ok(Handle::new(id)),
            _ => {}
        }

        let ext = extension_ascii_lower(&key.logical_path)
            .ok_or_else(|| AssetError::new("AssetStore: asset path has no extension"))?;

        let candidates = g
            .ext_index
            .get(ext.as_str())
            .cloned()
            .unwrap_or_default();

        let importer_ok = candidates.iter().any(|&i| g.importers[i].output_type() == type_id);
        if !importer_ok {
            return Err(AssetError::new(format!(
                "AssetStore: no importer for type '{}' with extension '.{}'",
                T::type_name(),
                ext
            )));
        }

        g.state.insert(id, AssetState::Loading);
        g.queue.push_back(PendingRequest {
            id,
            key,
            type_id,
            type_name: T::type_name(),
        });

        Ok(Handle::new(id))
    }

    /// Get a loaded asset (if ready).
    pub fn get<T: Asset>(&self, handle: Handle<T>) -> Option<AssetRef<T>> {
        let g = self.inner.lock();
        let map = g.loaded.get(&TypeId::of::<T>())?;
        let any = map.get(&handle.id())?.clone();
        drop(g);

        // Safe by construction: map is keyed by TypeId::of::<T>() and inserted only for that type.
        any.downcast::<T>().ok()
    }

    /// Progress loading pipeline for a limited amount of work.
    /// Deterministic and single-threaded by design; easy to move reading/import to worker later.
    pub fn pump(&self, budget: PumpBudget) {
        let mut steps_left = budget.steps;
        while steps_left > 0 {
            steps_left -= 1;

            let req = {
                let mut g = self.inner.lock();
                g.queue.pop_front()
            };

            let Some(req) = req else { break; };

            let result = self.process_one(req);
            if let Err(e) = result {
                let mut g = self.inner.lock();
                g.state.insert(e.id, AssetState::Failed(e.error.clone()));
                g.events.push_back(AssetEvent::Failed {
                    id: e.id,
                    type_name: e.type_name,
                    error: e.error,
                });
            }
        }
    }

    fn process_one(&self, req: PendingRequest) -> Result<(), ProcessError> {
        let bytes = self.read_from_any_source(&req.key.logical_path).map_err(|e| ProcessError {
            id: req.id,
            type_name: req.type_name,
            error: Arc::from(e.msg().to_string()),
        })?;

        let imported = self.import_with_registered(&req, &bytes).map_err(|e| ProcessError {
            id: req.id,
            type_name: req.type_name,
            error: Arc::from(e.msg().to_string()),
        })?;

        {
            let mut g = self.inner.lock();
            g.loaded
                .entry(req.type_id)
                .or_default()
                .insert(req.id, imported);
            g.state.insert(req.id, AssetState::Ready);
            g.events.push_back(AssetEvent::Ready {
                id: req.id,
                type_name: req.type_name,
            });
        }

        Ok(())
    }

    fn read_from_any_source(&self, logical_path: &Path) -> Result<Vec<u8>, AssetError> {
        let g = self.inner.lock();
        if g.sources.is_empty() {
            return Err(AssetError::new("AssetStore: no sources registered"));
        }

        // First source that "exists" wins (allows mod override layering).
        for s in &g.sources {
            if s.exists(logical_path) {
                return s.read(logical_path);
            }
        }

        Err(AssetError::new(format!(
            "AssetStore: asset not found in any source: '{}'",
            logical_path.to_string_lossy()
        )))
    }

    fn import_with_registered(
        &self,
        req: &PendingRequest,
        bytes: &[u8],
    ) -> Result<Arc<dyn Any + Send + Sync>, AssetError> {
        let g = self.inner.lock();
        let ext = extension_ascii_lower(&req.key.logical_path)
            .ok_or_else(|| AssetError::new("AssetStore: asset path has no extension"))?;

        let candidates = g
            .ext_index
            .get(ext.as_str())
            .cloned()
            .unwrap_or_default();

        for i in candidates {
            let imp = &g.importers[i];
            if imp.output_type() != req.type_id {
                continue;
            }
            return imp.import_dyn(bytes, &req.key);
        }

        Err(AssetError::new(format!(
            "AssetStore: importer disappeared for type '{}' ('.{}')",
            req.type_name, ext
        )))
    }
}

#[derive(Debug)]
struct ProcessError {
    id: AssetId,
    type_name: &'static str,
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