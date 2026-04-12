#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::{Arc, OnceLock, RwLock};

use newengine_plugin_api::EditorExtensionsV1;

pub type PluginEditorExtensionsExport = extern "C" fn() -> EditorExtensionsV1;

#[derive(Clone)]
pub struct LoadedPluginRootSnapshot {
    pub plugin_id: String,
    pub editor_extensions_v1: Option<PluginEditorExtensionsExport>,
}

pub type PluginRootObserver = Arc<dyn Fn(&LoadedPluginRootSnapshot) + Send + Sync + 'static>;

#[derive(Default)]
struct PluginRootObserverState {
    observers: Vec<PluginRootObserver>,
    loaded: Vec<LoadedPluginRootSnapshot>,
}

fn state() -> &'static RwLock<PluginRootObserverState> {
    static STATE: OnceLock<RwLock<PluginRootObserverState>> = OnceLock::new();
    STATE.get_or_init(|| RwLock::new(PluginRootObserverState::default()))
}

#[inline]
pub fn register_plugin_root_observer(observer: PluginRootObserver, replay_existing: bool) {
    let replay = {
        let mut guard = state().write().expect("plugin root observer state poisoned");
        let replay = if replay_existing {
            guard.loaded.clone()
        } else {
            Vec::new()
        };
        guard.observers.push(Arc::clone(&observer));
        replay
    };

    for snapshot in replay.iter() {
        observer(snapshot);
    }
}

#[inline]
pub(crate) fn record_loaded_plugin_root(snapshot: LoadedPluginRootSnapshot) {
    let observers = {
        let mut guard = state().write().expect("plugin root observer state poisoned");
        if let Some(existing) = guard
            .loaded
            .iter_mut()
            .find(|entry| entry.plugin_id == snapshot.plugin_id)
        {
            *existing = snapshot.clone();
        } else {
            guard.loaded.push(snapshot.clone());
        }
        guard.observers.clone()
    };

    for observer in observers.iter() {
        observer(&snapshot);
    }
}
