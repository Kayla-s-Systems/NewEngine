#![forbid(unsafe_op_in_unsafe_fn)]

//! Standalone game runtime profile.
//!
//! This crate is the app-facing boundary for game binaries. It intentionally
//! exposes no editor UI contract. Internally it still reuses the current shared
//! render/scene controller while those systems are being physically extracted
//! from `newengine-editor-runtime` into neutral runtime crates.

use newengine_assets::{AssetAccess, AssetServiceClient};
use newengine_core::{
    Engine, EngineLifecycleEvent, EngineReadinessKey, EngineReadinessSnapshot, EngineResult, Module, ModuleCtx,
    StartupConfig,
};
use newengine_editor_runtime::{scene_bridge::SceneBridge, EditorRuntimeProfile};
use newengine_ui::{UiBuildFn, UiProviderKind};
use std::any::Any;
use std::path::PathBuf;
use std::time::Duration;

use newengine_runtime_host::asset_bootstrap::{
    collect_app_asset_roots, mount_asset_roots_best_effort,
};

pub const GAME_FIXED_DT_MS: u32 = 16;
pub const GAME_APP_ASSETS_DIR_ENV: &str = "NEWENGINE_GAME_ASSETS_DIR";
pub const GAME_READY_APP_DIR_NAME: &str = "game-ready-fps";

const GAME_READY_SCENE_BOOTSTRAP_REQUIRES: &[EngineReadinessKey] = &[
    EngineReadinessKey::EnginePluginsReady,
];

struct GameReadySceneBootstrapModule {
    scene: std::sync::Arc<SceneBridge>,
    bootstrapped: bool,
    waiting_logged: bool,
}

impl GameReadySceneBootstrapModule {
    #[inline]
    fn new(scene: std::sync::Arc<SceneBridge>) -> Self {
        Self {
            scene,
            bootstrapped: false,
            waiting_logged: false,
        }
    }
}

impl GameReadySceneBootstrapModule {
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
                    "game-ready runtime: scene bootstrapped via lifecycle dispatch origin='{}' selected_player={:?}",
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
        // Safety net for external hosts that publish readiness before modules are
        // running but do not use synchronous dispatch. This still obeys the gate:
        // it only runs after the AssetManager service exists.
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
    inner: EditorRuntimeProfile,
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
            inner: EditorRuntimeProfile::new(),
        }
    }

    #[inline]
    pub fn register_modules(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
    ) -> EngineResult<()> {
        self.inner.register_modules(engine, startup)?;
        engine.register_module(Box::new(GameReadySceneBootstrapModule::new(std::sync::Arc::clone(
            self.inner.scene_bridge(),
        ))))?;
        Ok(())
    }

    #[inline]
    pub fn register_scene_io_best_effort(&self) {
        self.inner.register_scene_io_best_effort();
    }

    #[inline]
    pub fn bootstrap_game_ready_scene_best_effort(&self) {
        // Game scenes are assembled by GameReadySceneBootstrapModule during engine.start(),
        // after engine plugins are loaded. This keeps geometry imports on the required
        // AssetManager/geometryImporter path and prevents bootstrap-time filesystem fallbacks.
    }

    /// Standalone game builds render directly into the platform surface.
    /// No editor panels, docking, hierarchy, property grid, or markup loading.
    #[inline]
    pub fn ui_build_from_startup(
        &self,
        _startup: &StartupConfig,
    ) -> Option<Box<dyn UiBuildFn>> {
        None
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
