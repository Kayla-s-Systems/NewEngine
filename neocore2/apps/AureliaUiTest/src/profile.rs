use std::sync::Arc;

use newengine_core::{Engine, EngineResult, StartupConfig};
use newengine_runtime_host::app_launcher::{RuntimeHostAppProfile, RuntimeHostBootOption};
use newengine_ui::{UiBuildFn, UiProviderKind};
use newengine_windowed_host_runtime::WindowedRuntimeHostProfile;

use crate::options::BOOT_OPTIONS;
use crate::surface_module::AureliaUiTestSurfaceModule;

pub struct AureliaUiTestApp {
    viewport: Arc<newengine_engine_runtime::ViewportBridge>,
    plugins: Arc<newengine_engine_runtime::PluginManagerBridge>,
    scene: Arc<newengine_scene_runtime::SceneBridge>,
}

impl Default for AureliaUiTestApp {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl AureliaUiTestApp {
    #[inline]
    fn new() -> Self {
        Self {
            viewport: Arc::new(newengine_engine_runtime::ViewportBridge::new()),
            plugins: Arc::new(newengine_engine_runtime::PluginManagerBridge::new()),
            scene: Arc::new(newengine_scene_runtime::SceneBridge::new(
                newengine_scene::Scene::new(),
            )),
        }
    }
}

impl RuntimeHostAppProfile for AureliaUiTestApp {
    fn register_modules(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
    ) -> EngineResult<()> {
        engine.register_module(Box::new(
            newengine_render_runtime_adapter::RenderBackendRuntimeModule::new(
                startup.modules_dir.clone(),
            ),
        ))?;

        let render_controller = newengine_engine_runtime::RuntimeRenderController::new(
            Arc::clone(&self.viewport),
            Arc::clone(&self.plugins),
            Arc::clone(&self.scene),
        );

        engine.register_module(Box::new(render_controller))?;
        engine.register_module(Box::new(AureliaUiTestSurfaceModule::default()))?;
        Ok(())
    }

    #[inline]
    fn boot_options(&self) -> Option<&'static [RuntimeHostBootOption]> {
        Some(BOOT_OPTIONS)
    }

    #[inline]
    fn register_engine_provider_routes_best_effort(&self) {
        let host_api = newengine_plugin_host::default_host_api();
        let asset_client = newengine_assets::AssetServiceClient::new(host_api);
        let _registered =
            newengine_assets_ui_runtime::register_assets_ui_gateway_best_effort(asset_client);
    }
}

impl WindowedRuntimeHostProfile for AureliaUiTestApp {
    #[inline]
    fn ui_build_from_startup(&self, _startup: &StartupConfig) -> Option<Box<dyn UiBuildFn>> {
        None
    }

    #[inline]
    fn ui_provider_kind_from_startup(&self, _startup: &StartupConfig) -> UiProviderKind {
        UiProviderKind::Null
    }
}
