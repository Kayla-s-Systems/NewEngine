#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_plugin_api::EditorExtensionsV1;

pub type PluginEditorExtensionsExport = extern "C" fn() -> EditorExtensionsV1;

#[derive(Clone)]
pub struct LoadedPluginRootSnapshot {
    pub plugin_id: String,
    pub editor_extensions_v1: Option<PluginEditorExtensionsExport>,
}

pub type PluginRootObserver = Arc<dyn Fn(&LoadedPluginRootSnapshot) + Send + Sync + 'static>;

#[derive(Default)]
pub(crate) struct PluginRootObserverState {
    observers: Vec<PluginRootObserver>,
    loaded: Vec<LoadedPluginRootSnapshot>,
}

#[inline]
pub fn register_plugin_root_observer(observer: PluginRootObserver, replay_existing: bool) {
    let replay = {
        let context = crate::host_context::ctx();
        let mut guard = context
            .plugin_root_observers
            .write()
            .expect("plugin root observer state poisoned");
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
        let context = crate::host_context::ctx();
        let mut guard = context
            .plugin_root_observers
            .write()
            .expect("plugin root observer state poisoned");
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

#[inline]
pub(crate) fn loaded_plugin_root_snapshot(plugin_id: &str) -> Option<LoadedPluginRootSnapshot> {
    let context = crate::host_context::ctx();
    let guard = context
        .plugin_root_observers
        .read()
        .expect("plugin root observer state poisoned");
    guard
        .loaded
        .iter()
        .find(|entry| entry.plugin_id == plugin_id)
        .cloned()
}

/// Aggregates editor extensions exported by all currently loaded plugin roots.
///
/// Export callbacks are copied while holding the observer lock and invoked only
/// after releasing it. Plugin code therefore never executes under host locks.
pub fn editor_extensions_snapshot_v1() -> EditorExtensionsV1 {
    let exports = {
        let context = crate::host_context::ctx();
        let guard = context
            .plugin_root_observers
            .read()
            .expect("plugin root observer state poisoned");
        guard
            .loaded
            .iter()
            .filter_map(|entry| entry.editor_extensions_v1)
            .collect::<Vec<_>>()
    };

    let mut merged = EditorExtensionsV1::empty();
    for export in exports {
        merge_editor_extensions(&mut merged, export());
    }
    merged
}

#[inline]
fn merge_editor_extensions(target: &mut EditorExtensionsV1, source: EditorExtensionsV1) {
    for value in source.field_factories.into_iter() {
        target.field_factories.push(value);
    }
    for value in source.context_action_providers.into_iter() {
        target.context_action_providers.push(value);
    }
    for value in source.asset_import_providers.into_iter() {
        target.asset_import_providers.push(value);
    }
    for value in source.asset_assemblers.into_iter() {
        target.asset_assemblers.push(value);
    }
    for value in source.command_handlers.into_iter() {
        target.command_handlers.push(value);
    }
}

/// Removes callbacks owned by a plugin before its dynamic library is unloaded.
/// Keeping an ABI function pointer after `Library` destruction would make hot reload
/// of editing tools unsound.
pub(crate) fn forget_loaded_plugin_root(plugin_id: &str) {
    let context = crate::host_context::ctx();
    let mut guard = context
        .plugin_root_observers
        .write()
        .expect("plugin root observer state poisoned");
    guard.loaded.retain(|entry| entry.plugin_id != plugin_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use abi_stable::std_types::RString;
    use newengine_plugin_api::{
        EditorAssetAssemblerDescriptorV1, EditorImportedAssetAssemblyKindV1,
        EditorImportedAssetKindV1,
    };

    #[test]
    fn extension_merge_preserves_plugin_contributions() {
        let mut target = EditorExtensionsV1::empty();
        let mut source = EditorExtensionsV1::empty();
        source
            .asset_assemblers
            .push(EditorAssetAssemblerDescriptorV1 {
                key: RString::from("test.static_mesh"),
                label: RString::from("Static Mesh"),
                import_kind: EditorImportedAssetKindV1::StaticMesh,
                assembly: EditorImportedAssetAssemblyKindV1::StaticMeshActor,
            });

        merge_editor_extensions(&mut target, source);
        assert_eq!(target.asset_assemblers.len(), 1);
        assert_eq!(target.asset_assemblers[0].key.as_str(), "test.static_mesh");
    }
}
