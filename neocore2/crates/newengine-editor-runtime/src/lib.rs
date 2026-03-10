#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use newengine_assets::AssetAccess;
use newengine_core::{Engine, EngineResult, StartupConfig};
use newengine_runtime_host::render_runtime::RenderBackendRuntimeModule;
use newengine_ui::{UiBuildFn, UiMarkupDoc};

mod editor_camera;
mod material_pipeline;
pub mod plugin_manager;
pub mod render_controller;
pub mod render_runtime;
mod scene_bootstrap;
pub mod scene_bridge;
pub mod scene_io_service;
mod shared;
pub mod ui;
pub mod ui_contrib;
pub mod viewport_bridge;

pub const EDITOR_FIXED_DT_MS: u32 = 16;
pub const EDITOR_UI_MARKUP_PATH: &str = "ui/editor.xml";
pub const EDITOR_APP_ASSETS_DIR_ENV: &str = "NEWENGINE_EDITOR_ASSETS_DIR";
pub const EDITOR_APP_DIR_NAME: &str = "editor";

pub struct EditorRuntimeProfile {
    shared_doc: Arc<Mutex<Option<Arc<UiMarkupDoc>>>>,
    viewport: Arc<viewport_bridge::ViewportBridge>,
    plugins: Arc<plugin_manager::PluginManagerBridge>,
    scene: Arc<scene_bridge::SceneBridge>,
    previews: Arc<parking_lot::Mutex<newengine_previews::PrimitivePreviewService>>,
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
            shared_doc: Arc::new(Mutex::new(None)),
            viewport: Arc::new(viewport_bridge::ViewportBridge::new()),
            plugins: Arc::new(plugin_manager::PluginManagerBridge::new()),
            scene: Arc::new(scene_bridge::SceneBridge::new(newengine_scene::Scene::new())),
            previews: Arc::new(parking_lot::Mutex::new(
                newengine_previews::PrimitivePreviewService::new(),
            )),
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

    #[inline]
    pub fn ui_build_from_startup(
        &self,
        startup: &StartupConfig,
    ) -> Option<Box<dyn UiBuildFn>> {
        match startup.ui_backend {
            newengine_core::startup::UiBackend::Disabled => None,
            _ => Some(Box::new(ui::EditorUiBuild::new(
                Arc::clone(&self.shared_doc),
                Arc::clone(&self.viewport),
                Arc::clone(&self.plugins),
                Arc::clone(&self.scene),
                Arc::clone(&self.previews),
            ))),
        }
    }

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
}
