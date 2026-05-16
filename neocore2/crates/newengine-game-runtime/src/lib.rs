#![forbid(unsafe_op_in_unsafe_fn)]

//! Standalone game/runtime composition profile.
//!
//! This crate is the app-facing runtime boundary for playable builds. Game code
//! registers a profile and scene bootstrap; rendering remains owned by the
//! engine render controller and the selected render plugin. No authoring UI, panels,
//! docking, hierarchy, property grid, or Vulkan-specific resource work is pulled
//! into the game binary.

use newengine_assets::{AssetAccess, AssetServiceClient};
use newengine_core::{
    Engine, EngineLifecycleEvent, EngineReadinessKey, EngineReadinessSnapshot, EngineResult, Module, ModuleCtx,
    StartupConfig,
};
use newengine_render_feature_gameready::GameReadyRenderFeaturePack;
use newengine_runtime_host::render_runtime::RenderBackendRuntimeModule;
use newengine_ui::{UiBuildFn, UiProviderKind};
use std::any::Any;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use newengine_runtime_host::asset_bootstrap::{
    collect_app_asset_roots, mount_asset_roots_best_effort,
};

pub use newengine_engine_runtime::{PhysicsBodyDesc, CollisionShapeDesc, GameRunMode, GameplayActor, PlayerActor};

pub const GAME_FIXED_DT_MS: u32 = 16;
pub const GAME_APP_ASSETS_DIR_ENV: &str = "NEWENGINE_GAME_ASSETS_DIR";
pub const GAME_READY_APP_DIR_NAME: &str = "game-ready-fps";

const GAME_READY_SCENE_BOOTSTRAP_REQUIRES: &[EngineReadinessKey] = &[
    EngineReadinessKey::EnginePluginsReady,
];

struct GameReadySceneBootstrapModule {
    scene: Arc<newengine_engine_runtime::SceneBridge>,
    bootstrapped: bool,
    waiting_logged: bool,
}

impl GameReadySceneBootstrapModule {
    #[inline]
    fn new(scene: Arc<newengine_engine_runtime::SceneBridge>) -> Self {
        Self {
            scene,
            bootstrapped: false,
            waiting_logged: false,
        }
    }

    #[inline]
    fn log_waiting_once(&mut self, origin: &'static str) {
        if self.waiting_logged {
            return;
        }
        self.waiting_logged = true;
        log::info!(
            "game-ready runtime: waiting for AssetManager/geometryImporter readiness before scene bootstrap origin='{}'",
            origin
        );
    }

    #[inline]
    fn try_bootstrap(&mut self, origin: &'static str) -> EngineResult<()> {
        if self.bootstrapped {
            return Ok(());
        }

        if !newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID) {
            self.log_waiting_once(origin);
            return Ok(());
        }

        let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
        let asset_roots = collect_app_asset_roots(GAME_READY_APP_DIR_NAME, GAME_APP_ASSETS_DIR_ENV);
        mount_asset_roots_best_effort(&assets, &asset_roots);

        match self.scene.bootstrap_game_ready_scene_now() {
            Some(player) => {
                self.bootstrapped = true;
                log::info!(
                    "game-ready runtime: CPU scene bootstrapped via lifecycle dispatch origin='{}' selected_player={:?}; waiting for launch gate before public Play",
                    origin,
                    player
                );
            }
            None => {
                log::warn!(
                    "game-ready runtime: scene bootstrap failed after readiness dispatch origin='{}'",
                    origin
                );
            }
        }

        Ok(())
    }
}

impl<E: Send + 'static> Module<E> for GameReadySceneBootstrapModule {
    #[inline]
    fn id(&self) -> &'static str {
        "app.game_ready_scene_bootstrap"
    }

    #[inline]
    fn startup_requires(&self) -> &'static [EngineReadinessKey] {
        GAME_READY_SCENE_BOOTSTRAP_REQUIRES
    }

    #[inline]
    fn start(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let origin = if ctx
            .resources()
            .get::<EngineReadinessSnapshot>()
            .map(|s| s.engine_plugins_ready)
            .unwrap_or(false)
        {
            "startup-graph-engine-plugins-ready"
        } else {
            "startup-graph-unexpected-early-start"
        };
        self.try_bootstrap(origin)
    }

    #[inline]
    fn on_event(&mut self, _ctx: &mut ModuleCtx<'_, E>, event: &dyn Any) -> EngineResult<()> {
        let Some(event) = event.downcast_ref::<EngineLifecycleEvent>() else {
            return Ok(());
        };

        match event {
            EngineLifecycleEvent::EnginePluginsReady { origin, .. } => {
                self.try_bootstrap(origin)
            }
            EngineLifecycleEvent::EngineStartCompleted { .. } => {
                if self.bootstrapped {
                    Ok(())
                } else {
                    self.log_waiting_once("engine-start-completed");
                    Ok(())
                }
            }
        }
    }

    #[inline]
    fn update(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        if !self.bootstrapped
            && newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID)
        {
            self.try_bootstrap("update-readiness-fallback")?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct StandaloneGameRuntimeProfile {
    viewport: Arc<newengine_engine_runtime::ViewportBridge>,
    plugins: Arc<newengine_engine_runtime::PluginManagerBridge>,
    scene: Arc<newengine_engine_runtime::SceneBridge>,
}

impl Default for StandaloneGameRuntimeProfile {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl StandaloneGameRuntimeProfile {
    #[inline]
    pub fn new() -> Self {
        Self {
            viewport: Arc::new(newengine_engine_runtime::ViewportBridge::new()),
            plugins: Arc::new(newengine_engine_runtime::PluginManagerBridge::new()),
            scene: Arc::new(newengine_engine_runtime::SceneBridge::new(newengine_scene::Scene::new())),
        }
    }

    #[inline]
    pub fn register_modules(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
    ) -> EngineResult<()> {
        engine.register_module(Box::new(RenderBackendRuntimeModule::new(
            startup.modules_dir.clone(),
        )))?;

        let render_controller = GameReadyRenderFeaturePack::new().install(
            newengine_engine_runtime::RuntimeRenderController::new(
                Arc::clone(&self.viewport),
                Arc::clone(&self.plugins),
                Arc::clone(&self.scene),
            ),
        );

        engine.register_module(Box::new(render_controller))?;

        engine.register_module(Box::new(GameReadySceneBootstrapModule::new(Arc::clone(
            &self.scene,
        ))))?;
        Ok(())
    }

    #[inline]
    pub fn register_scene_io_best_effort(&self) {
        // Standalone games are game-first runtime consumers. They do not register
        // authoring scene save/load host services by default.
    }

    #[inline]
    pub fn bootstrap_game_ready_scene_best_effort(&self) {
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

    #[inline]
    pub fn ui_provider_kind(&self) -> UiProviderKind {
        UiProviderKind::Null
    }

    /// Compatibility no-op retained so launchers can share bootstrap shape.
    #[inline]
    pub fn load_markup_best_effort(
        &self,
        _assets: Option<&dyn AssetAccess>,
        _roots: &[PathBuf],
        _path: &str,
        _timeout: Duration,
    ) {
    }
}
