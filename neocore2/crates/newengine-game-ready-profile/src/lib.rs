#![forbid(unsafe_op_in_unsafe_fn)]

//! Game Ready FPS runtime profile composition.
//!
//! This crate is product/profile composition only: it installs reusable runtime
//! modules, the GameReady render feature pack, the game-ready scene bootstrap
//! module and the selected engine-owned gateway services.

mod game_ready_fps;
mod scene_bootstrap;

use std::sync::Arc;

use newengine_core::{Engine, EngineResult, StartupConfig};
use newengine_render_feature_gameready::GameReadyRenderFeaturePack;
use newengine_runtime_host::physics_runtime::PhysicsBackendRuntimeModule;
use newengine_runtime_host::render_runtime::RenderBackendRuntimeModule;
use newengine_scene_runtime::SceneGatewayAssetMounts;
use newengine_ui::{UiBuildFn, UiProviderKind};

use scene_bootstrap::GameReadySceneBootstrapModule;

pub use game_ready_fps::{run_game_ready_fps_process, GameReadyFpsApp};

pub const GAME_FIXED_DT_MS: u32 = 16;
pub const GAME_APP_ASSETS_DIR_ENV: &str = "NEWENGINE_GAME_ASSETS_DIR";
pub const GAME_READY_APP_DIR_NAME: &str = "game-ready-fps";

#[derive(Clone)]
pub struct GameReadyRuntimeProfile {
    viewport: Arc<newengine_engine_runtime::ViewportBridge>,
    plugins: Arc<newengine_engine_runtime::PluginManagerBridge>,
    scene: Arc<newengine_scene_runtime::SceneBridge>,
}

impl Default for GameReadyRuntimeProfile {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl GameReadyRuntimeProfile {
    #[inline]
    pub fn new() -> Self {
        Self {
            viewport: Arc::new(newengine_engine_runtime::ViewportBridge::new()),
            plugins: Arc::new(newengine_engine_runtime::PluginManagerBridge::new()),
            scene: Arc::new(newengine_scene_runtime::SceneBridge::new(newengine_scene::Scene::new())),
        }
    }

    #[inline]
    pub fn register_modules(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
    ) -> EngineResult<()> {
        // Install GameReady input defaults before constructing render/pause runtime.
        // RuntimeRenderController owns the pause-menu state, and that state snapshots
        // the active input profile during construction. Initializing the bindings
        // gateway here prevents an empty generic profile from being captured first.
        newengine_input_bindings_runtime::register_input_bindings_gateway_best_effort(
            newengine_input_profile_gameready::game_ready_input_profile(),
        );

        engine.register_module(Box::new(PhysicsBackendRuntimeModule::new(
            startup.modules_dir.clone(),
        )))?;

        engine.register_module(Box::new(RenderBackendRuntimeModule::new(
            startup.modules_dir.clone(),
        )))?;

        let render_features = GameReadyRenderFeaturePack::new();
        let mut render_controller = newengine_engine_runtime::RuntimeRenderController::new(
            Arc::clone(&self.viewport),
            Arc::clone(&self.plugins),
            Arc::clone(&self.scene),
        )
        .with_material_pipeline_provider(render_features.material_pipeline_provider())
        .with_primary_lit_material_domain(render_features.primary_lit_material_domain());

        for provider in render_features.draw_list_providers() {
            render_controller = render_controller.with_draw_list_provider(provider);
        }
        for provider in render_features.light_extraction_providers() {
            render_controller = render_controller.with_light_extraction_provider(provider);
        }

        engine.register_module(Box::new(render_controller))?;

        engine.register_module(Box::new(GameReadySceneBootstrapModule::new(Arc::clone(
            &self.scene,
        ))))?;
        Ok(())
    }

    #[inline]
    pub fn register_engine_owned_gateways_best_effort(&self) {
        let asset_mounts = SceneGatewayAssetMounts::new(
            GAME_READY_APP_DIR_NAME,
            GAME_APP_ASSETS_DIR_ENV,
        );
        newengine_scene_runtime::register_scene_gateway_best_effort(
            Arc::clone(&self.scene),
            Some(asset_mounts),
        );
        newengine_ecs_runtime::register_ecs_gateway_best_effort(Arc::clone(&self.scene));
        newengine_entity_runtime::register_entity_gateway_best_effort(Arc::clone(&self.scene));
        newengine_input_bindings_runtime::register_input_bindings_gateway_best_effort(
            newengine_input_profile_gameready::game_ready_input_profile(),
        );
        newengine_time_runtime::register_time_gateway_best_effort();
        newengine_scripting_runtime::register_scripting_gateway_best_effort();
        newengine_assets::register_asset_file_types_gateway_best_effort();

        let host_api = newengine_plugin_host::default_host_api();
        let asset_client = newengine_assets::AssetServiceClient::new(host_api.clone());
        newengine_textures_runtime::register_textures_gateway_best_effort(asset_client.clone());
        newengine_definitions_runtime::register_definitions_gateway_best_effort(asset_client.clone());
        newengine_material_runtime::register_materials_gateway_best_effort_with_host(Some(host_api.clone()), asset_client.clone());
        newengine_model_runtime::register_model_gateway_best_effort_with_host(host_api.clone(), asset_client.clone());
        newengine_model_runtime::register_asset_graph_gateway_best_effort(host_api, asset_client);
    }

    #[inline]
    pub fn bootstrap_content_best_effort(&self) {
        // Game scenes are assembled by GameReadySceneBootstrapModule during engine.start(),
        // after engine plugins are loaded. This keeps geometry imports on the required
        // AssetManager/geometryImporter path and prevents bootstrap-time filesystem fallbacks.
    }

    /// Standalone game builds render directly into the platform surface.
    /// No authoring panels, docking, hierarchy, property grid, or markup loading.
    #[inline]
    pub fn ui_build_from_startup(
        &self,
        _startup: &StartupConfig,
    ) -> Option<Box<dyn UiBuildFn>> {
        None
    }

    #[inline]
    pub fn ui_provider_kind_from_startup(&self, _startup: &StartupConfig) -> UiProviderKind {
        // UI provider selection is discovery-driven at runtime-host level.
        UiProviderKind::Null
    }
}
