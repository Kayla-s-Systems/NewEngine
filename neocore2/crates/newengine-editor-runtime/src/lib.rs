#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

#[cfg(feature = "editor-ui")]
use newengine_math::collections_prelude::NeHashSet as HashSet;
#[cfg(feature = "editor-ui")]
use std::{path::PathBuf, sync::Mutex, time::Duration};

#[cfg(feature = "editor-ui")]
use newengine_assets::AssetAccess;
use newengine_core::{Engine, EngineResult, StartupConfig};
use newengine_runtime_host::render_runtime::RenderBackendRuntimeModule;
use newengine_ui::UiBuildFn;
#[cfg(feature = "editor-ui")]
use newengine_ui::UiMarkupDoc;

pub mod gameplay { pub use newengine_engine_runtime::gameplay::*; }
#[cfg(feature = "editor-ui")]
mod material_pipeline;
pub mod plugin_manager;
pub mod render_controller { pub use newengine_engine_runtime::render_controller::*; }
pub mod render_runtime { pub use newengine_engine_runtime::render_runtime::*; }
pub mod scene_bridge { pub use newengine_engine_runtime::scene_bridge::*; }
pub mod scene_io_service;
mod shared;
#[cfg(feature = "editor-ui")]
pub mod ui;
#[cfg(feature = "editor-ui")]
pub mod ui_contrib;
pub mod viewport_bridge { pub use newengine_engine_runtime::viewport_bridge::*; }
pub use gameplay::{CollisionBody, CollisionShape, EditorPlayMode, GameplayActor, PlayerActor};

pub const EDITOR_FIXED_DT_MS: u32 = 16;
pub const EDITOR_UI_MARKUP_PATH: &str = "ui/editor.xml";
pub const EDITOR_APP_ASSETS_DIR_ENV: &str = "NEWENGINE_EDITOR_ASSETS_DIR";
pub const EDITOR_APP_DIR_NAME: &str = "editor";

#[derive(Clone)]
pub struct EditorRuntimeProfile {
    #[cfg(feature = "editor-ui")]
    shared_doc: Arc<Mutex<Option<Arc<UiMarkupDoc>>>>,
    viewport: Arc<viewport_bridge::ViewportBridge>,
    plugins: Arc<plugin_manager::PluginManagerBridge>,
    scene: Arc<scene_bridge::SceneBridge>,
    previews: Arc<parking_lot::Mutex<newengine_previews::PrimitivePreviewService>>,
    #[cfg(feature = "editor-ui")]
    schema_registry: Arc<parking_lot::RwLock<ui::schema::EditorSchemaRegistry>>,
    #[cfg(feature = "editor-ui")]
    extension_registry: Arc<parking_lot::RwLock<ui::extension_abi::EditorExtensionAbiRegistry>>,
    #[cfg(feature = "editor-ui")]
    auto_wired_extension_plugins: Arc<parking_lot::Mutex<HashSet<String>>>,
    #[cfg(feature = "editor-ui")]
    plugin_root_auto_wiring_installed: Arc<std::sync::atomic::AtomicBool>,
}

impl Default for EditorRuntimeProfile {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl EditorRuntimeProfile {
    #[inline]
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "editor-ui")]
            shared_doc: Arc::new(Mutex::new(None)),
            viewport: Arc::new(viewport_bridge::ViewportBridge::new()),
            plugins: Arc::new(plugin_manager::PluginManagerBridge::new()),
            scene: Arc::new(scene_bridge::SceneBridge::new(newengine_scene::Scene::new())),
            previews: Arc::new(parking_lot::Mutex::new(
                newengine_previews::PrimitivePreviewService::new(),
            )),
            #[cfg(feature = "editor-ui")]
            schema_registry: Arc::new(parking_lot::RwLock::new(
                ui::schema::EditorSchemaRegistry::default(),
            )),
            #[cfg(feature = "editor-ui")]
            extension_registry: Arc::new(parking_lot::RwLock::new(
                ui::extension_abi::EditorExtensionAbiRegistry::default(),
            )),
            #[cfg(feature = "editor-ui")]
            auto_wired_extension_plugins: Arc::new(parking_lot::Mutex::new(HashSet::default())),
            #[cfg(feature = "editor-ui")]
            plugin_root_auto_wiring_installed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    #[inline]
    pub fn register_modules(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
    ) -> EngineResult<()> {
        engine.register_module(Box::new(RenderBackendRuntimeModule::new(
            startup.render_backend.clone(),
            startup.modules_dir.clone(),
        )))?;

        engine.register_module(Box::new(render_controller::EditorRenderController::new(
            Arc::clone(&self.viewport),
            Arc::clone(&self.plugins),
            Arc::clone(&self.scene),
            Arc::clone(&self.previews),
        )))?;

        Ok(())
    }

    #[inline]
    pub fn register_scene_io_best_effort(&self) {
        scene_io_service::register_scene_io_best_effort(Arc::clone(&self.scene));
    }

    #[cfg(feature = "editor-ui")]
    #[inline]
    pub fn ui_build_from_startup(
        &self,
        startup: &StartupConfig,
    ) -> Option<Box<dyn UiBuildFn>> {
        if startup.ui_backend.is_none() {
            return None;
        }

        Some(Box::new(ui::EditorUiBuild::new(
            Arc::clone(&self.shared_doc),
            Arc::clone(&self.viewport),
            Arc::clone(&self.plugins),
            Arc::clone(&self.scene),
            Arc::clone(&self.previews),
            Arc::clone(&self.schema_registry),
            Arc::clone(&self.extension_registry),
        )))
    }

    #[cfg(not(feature = "editor-ui"))]
    #[inline]
    pub fn ui_build_from_startup(
        &self,
        _startup: &StartupConfig,
    ) -> Option<Box<dyn UiBuildFn>> {
        None
    }

    #[cfg(feature = "editor-ui")]
    #[inline]
    pub fn load_markup_best_effort(
        &self,
        assets: Option<&dyn AssetAccess>,
        roots: &[PathBuf],
        path: &str,
        timeout: Duration,
    ) {
        match UiMarkupDoc::load_best_effort(assets, roots, path, timeout) {
            Ok(doc) => {
                if let Ok(mut guard) = self.shared_doc.lock() {
                    *guard = Some(Arc::new(doc));
                }
            }
            Err(e) => {
                log::warn!(
                    "editor runtime: ui markup load failed path='{}' err='{}' (degraded mode)",
                    path,
                    e
                );
            }
        }
    }

    #[cfg(not(feature = "editor-ui"))]
    #[inline]
    pub fn load_markup_best_effort(
        &self,
        _assets: Option<&dyn newengine_assets::AssetAccess>,
        _roots: &[std::path::PathBuf],
        _path: &str,
        _timeout: std::time::Duration,
    ) {
    }

    #[cfg(feature = "editor-ui")]
    #[inline]
    pub fn register_editor_field_factory(
        &self,
        factory: ui::schema::RegisteredFieldFactory,
    ) {
        self.schema_registry.write().field_factories.push(factory);
    }

    #[cfg(feature = "editor-ui")]
    #[inline]
    pub fn register_editor_context_action_provider(
        &self,
        provider: ui::schema::RegisteredContextActionProvider,
    ) {
        self.schema_registry.write().context_action_providers.push(provider);
    }

    #[cfg(feature = "editor-ui")]
    #[inline]
    pub fn register_editor_asset_import_provider(
        &self,
        provider: ui::schema::RegisteredAssetImportProvider,
    ) {
        self.schema_registry.write().asset_import_providers.push(provider);
    }

    #[inline]
    pub fn register_imported_asset_assembler(
        &self,
        assembler: scene_bridge::SceneImportedAssetAssembler,
    ) {
        self.scene.register_imported_asset_assembler(assembler);
    }

    #[cfg(feature = "editor-ui")]
    #[inline]
    pub fn register_editor_extensions_v1(
        &self,
        plugin_id: &str,
        extensions: newengine_plugin_api::EditorExtensionsV1,
    ) -> usize {
        ui::extension_abi::register_editor_extensions(
            &self.extension_registry,
            self.scene.as_ref(),
            plugin_id,
            extensions,
        )
    }

    #[cfg(not(feature = "editor-ui"))]
    #[inline]
    pub fn register_editor_extensions_v1(
        &self,
        _plugin_id: &str,
        _extensions: newengine_plugin_api::EditorExtensionsV1,
    ) -> usize {
        0
    }

    #[inline]
    pub fn register_plugin_root_editor_extensions(
        &self,
        plugin_id: &str,
        root: newengine_plugin_api::PluginRootV1Ref,
    ) -> usize {
        let Some(export) = root.editor_extensions_v1() else {
            return 0;
        };
        self.register_editor_extensions_v1(plugin_id, export())
    }

    #[cfg(feature = "editor-ui")]
    #[inline]
    pub fn install_plugin_root_editor_auto_wiring(&self, replay_existing: bool) {
        use std::sync::atomic::Ordering;

        if self
            .plugin_root_auto_wiring_installed
            .swap(true, Ordering::AcqRel)
        {
            return;
        }

        let profile = self.clone();
        newengine_plugin_host::register_plugin_root_observer(
            Arc::new(move |snapshot| {
                let Some(export) = snapshot.editor_extensions_v1 else {
                    return;
                };

                {
                    let mut wired = profile.auto_wired_extension_plugins.lock();
                    if !wired.insert(snapshot.plugin_id.clone()) {
                        return;
                    }
                }

                let installed = profile.register_editor_extensions_v1(
                    &snapshot.plugin_id,
                    export(),
                );
                if installed > 0 {
                    log::info!(
                        "editor runtime: auto-wired {} editor extension entries from plugin '{}'",
                        installed,
                        snapshot.plugin_id
                    );
                } else {
                    log::debug!(
                        "editor runtime: plugin '{}' exported no editor extension entries",
                        snapshot.plugin_id
                    );
                }
            }),
            replay_existing,
        );
    }

    #[cfg(not(feature = "editor-ui"))]
    #[inline]
    pub fn install_plugin_root_editor_auto_wiring(&self, _replay_existing: bool) {}

    #[inline]
    pub fn viewport_bridge(&self) -> &Arc<viewport_bridge::ViewportBridge> {
        &self.viewport
    }

    #[inline]
    pub fn plugin_manager_bridge(&self) -> &Arc<plugin_manager::PluginManagerBridge> {
        &self.plugins
    }

    #[inline]
    pub fn scene_bridge(&self) -> &Arc<scene_bridge::SceneBridge> {
        &self.scene
    }

    #[inline]
    pub fn bootstrap_game_ready_scene_best_effort(&self) {
        let _ = self.scene.bootstrap_game_ready_scene_now();
    }
}
