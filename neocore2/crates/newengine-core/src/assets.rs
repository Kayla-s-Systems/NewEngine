use log::info;
use newengine_assets::{
    AssetBlob, AssetError, AssetEvent, AssetId, AssetKey, AssetSource, AssetState, AssetStore,
    BlobImporterDispatch, FileSystemSource, PumpBudget,
};
use std::path::PathBuf;
use std::sync::Arc;

pub struct AssetManager {
    store: Arc<AssetStore>,
    budget: PumpBudget,
}

impl AssetManager {
    #[inline]
    pub fn new_default(root: PathBuf) -> Self {
        info!(target: "assets", "manager.init root='{}'", root.display());

        let store = Arc::new(AssetStore::new());

        info!(
            target: "assets",
            "manager.source.register kind='filesystem' root='{}'",
            root.display()
        );
        store.add_source(Arc::new(FileSystemSource::new(root)));

        let budget = PumpBudget::steps(8);
        info!(target: "assets", "manager.budget steps={}", budget.steps);

        Self { store, budget }
    }

    /// Returns a shared handle to the underlying store.
    #[inline]
    pub fn store(&self) -> &Arc<AssetStore> {
        &self.store
    }

    /// Registers an additional asset source.
    #[inline]
    pub fn add_source(&self, source: Arc<dyn AssetSource>) {
        self.store.add_source(source);
    }

    /// Registers a type-erased importer dispatch (usually a plugin-backed service adapter).
    #[inline]
    pub fn add_importer(&self, importer: Arc<dyn BlobImporterDispatch>) {
        self.store.add_importer(importer);
    }

    /// Enqueues an import request.
    #[inline]
    pub fn load(&self, key: AssetKey) -> Result<AssetId, AssetError> {
        self.store.load(key)
    }

    #[inline]
    pub fn state(&self, id: AssetId) -> AssetState {
        self.store.state(id)
    }

    #[inline]
    pub fn get_blob(&self, id: AssetId) -> Option<Arc<AssetBlob>> {
        self.store.get_blob(id)
    }

    #[inline]
    pub fn drain_events(&self) -> Vec<AssetEvent> {
        self.store.drain_events()
    }

    #[inline]
    pub fn set_budget(&mut self, steps: u32) {
        let steps = steps.max(1);
        info!(target: "assets", "manager.budget.update steps={}", steps);
        self.budget = PumpBudget::steps(steps);
    }

    #[inline]
    pub fn pump(&self) {
        self.store.pump(self.budget);
    }

    /// Convenience: pump and return any produced events.
    #[inline]
    pub fn pump_and_drain(&self) -> Vec<AssetEvent> {
        self.pump();
        self.drain_events()
    }
}