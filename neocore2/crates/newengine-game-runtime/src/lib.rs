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
use newengine_runtime_host::physics_runtime::PhysicsBackendRuntimeModule;
use newengine_runtime_host::render_runtime::RenderBackendRuntimeModule;
use newengine_ui::{UiBuildFn, UiProviderKind};
use std::any::Any;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::{Blob, CapabilityId, MethodName, ServiceV1, ServiceV1Dyn};
use newengine_scene::{SceneAsset, SceneAssetOptions};
use newengine_scene_io::{method as scene_method, ENGINE_SCENE_SERVICE_ID, SCENE_BACKEND_CAPABILITY_ID};

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
struct EngineSceneGatewayService {
    scene: Arc<newengine_engine_runtime::SceneBridge>,
}

impl EngineSceneGatewayService {
    #[inline]
    fn new(scene: Arc<newengine_engine_runtime::SceneBridge>) -> Self {
        Self { scene }
    }

    #[inline]
    fn ok_json(value: serde_json::Value) -> RResult<Blob, RString> {
        match serde_json::to_vec(&value) {
            Ok(bytes) => RResult::ROk(Blob::from(bytes)),
            Err(e) => RResult::RErr(RString::from(e.to_string())),
        }
    }

    #[inline]
    fn payload_json(payload: Blob) -> Result<serde_json::Value, String> {
        if payload.is_empty() {
            return Ok(serde_json::json!({}));
        }
        serde_json::from_slice(payload.as_slice()).map_err(|e| e.to_string())
    }

    fn formats_json(&self) -> RResult<Blob, RString> {
        Self::ok_json(serde_json::json!({
            "id": ENGINE_SCENE_SERVICE_ID,
            "origin": "engine-owned",
            "owner": "newengine-game-runtime.scene-bridge",
            "version": 1,
            "formats": [
                {
                    "id": "newengine.scene.asset.v1",
                    "schema": "kalitech.scene.asset.v1",
                    "media_type": "application/json",
                    "load": true,
                    "save": true
                }
            ],
            "methods": [
                scene_method::FORMATS_JSON,
                scene_method::LOAD_JSON_V1,
                scene_method::SAVE_JSON_V1
            ]
        }))
    }

    fn load_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match Self::payload_json(payload) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        let path = req
            .get("path")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(path) = path else {
            return RResult::RErr(RString::from("engine.scene load_json_v1 requires non-empty path"));
        };

        let replace = req.get("replace").and_then(|v| v.as_bool()).unwrap_or(true);
        if !replace {
            return RResult::RErr(RString::from(
                "engine.scene load_json_v1 currently supports replace=true only",
            ));
        }

        if !newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID) {
            return RResult::RErr(RString::from(format!(
                "engine.scene cannot load '{path}': asset gateway '{}' is unavailable",
                newengine_assets::consts::ASSET_SERVICE_ID
            )));
        }

        let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
        let asset_roots = collect_app_asset_roots(GAME_READY_APP_DIR_NAME, GAME_APP_ASSETS_DIR_ENV);
        mount_asset_roots_best_effort(&assets, &asset_roots);

        let bytes = match assets.text_v1(path) {
            Ok(bytes) => bytes,
            Err(e) => {
                return RResult::RErr(RString::from(format!(
                    "engine.scene load_json_v1 asset read failed path='{path}' err='{e}'"
                )));
            }
        };

        let asset = match serde_json::from_slice::<SceneAsset>(&bytes) {
            Ok(asset) => asset,
            Err(e) => {
                return RResult::RErr(RString::from(format!(
                    "engine.scene load_json_v1 scene json parse failed path='{path}' err='{e}'"
                )));
            }
        };

        {
            let scene_lock = self.scene.scene();
            let mut scene = scene_lock.write();
            if let Err(e) = scene.load_asset(&asset) {
                return RResult::RErr(RString::from(format!(
                    "engine.scene load_json_v1 scene apply failed path='{path}' err='{e}'"
                )));
            }
        }

        Self::ok_json(serde_json::json!({
            "ok": true,
            "path": path,
            "replace": true,
            "entities": asset.entities.len(),
            "schema": asset.schema,
            "version": asset.version
        }))
    }

    fn save_json_v1(&self, payload: Blob) -> RResult<Blob, RString> {
        let req = match Self::payload_json(payload) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };
        let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let pretty = req.get("pretty").and_then(|v| v.as_bool()).unwrap_or(true);
        let include_empty_entities = req
            .get("options")
            .and_then(|v| v.get("include_empty_entities"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let asset = {
            let scene_lock = self.scene.scene();
            let mut scene = scene_lock.write();
            scene.to_asset(SceneAssetOptions { include_empty_entities })
        };

        let payload = match serde_json::to_value(&asset) {
            Ok(value) => value,
            Err(e) => return RResult::RErr(RString::from(e.to_string())),
        };
        let payload_text = match if pretty {
            serde_json::to_string_pretty(&asset)
        } else {
            serde_json::to_string(&asset)
        } {
            Ok(value) => value,
            Err(e) => return RResult::RErr(RString::from(e.to_string())),
        };

        Self::ok_json(serde_json::json!({
            "ok": true,
            "path": path,
            "stored": false,
            "storage": "caller-owned",
            "pretty": pretty,
            "payload": payload,
            "payload_text": payload_text
        }))
    }
}

impl ServiceV1 for EngineSceneGatewayService {
    fn id(&self) -> CapabilityId {
        CapabilityId::from(ENGINE_SCENE_SERVICE_ID)
    }

    fn describe(&self) -> RString {
        RString::from(
            serde_json::json!({
                "id": ENGINE_SCENE_SERVICE_ID,
                "version": 1,
                "contract": "newengine.scene gateway >= 0.1.x",
                "origin": "engine-owned",
                "owner": "newengine-game-runtime.scene-bridge",
                "methods": [
                    scene_method::FORMATS_JSON,
                    scene_method::LOAD_JSON_V1,
                    scene_method::SAVE_JSON_V1
                ]
            })
            .to_string(),
        )
    }

    fn call(&self, method: MethodName, payload: Blob) -> RResult<Blob, RString> {
        match method.as_str() {
            scene_method::FORMATS_JSON => self.formats_json(),
            scene_method::LOAD_JSON_V1 => self.load_json_v1(payload),
            scene_method::SAVE_JSON_V1 => self.save_json_v1(payload),
            other => RResult::RErr(RString::from(format!(
                "engine.scene unknown method '{other}'"
            ))),
        }
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
        engine.register_module(Box::new(PhysicsBackendRuntimeModule::new(
            startup.modules_dir.clone(),
        )))?;

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
        if newengine_plugin_host::has_service(ENGINE_SCENE_SERVICE_ID) {
            log::debug!(
                "engine.scene gateway registration skipped; service already available"
            );
            return;
        }

        let service = EngineSceneGatewayService::new(Arc::clone(&self.scene));
        let dyn_svc = ServiceV1Dyn::from_value(service, abi_stable::sabi_trait::TD_Opaque);

        match newengine_plugin_host::host_register_service_impl(dyn_svc) {
            RResult::ROk(()) => {}
            RResult::RErr(e) => {
                log::error!(
                    "engine.scene service registration failed id='{}' err='{}'",
                    ENGINE_SCENE_SERVICE_ID,
                    e
                );
                return;
            }
        }

        match newengine_plugin_host::register_engine_owned_gateway(
            ENGINE_SCENE_SERVICE_ID,
            newengine_service_api::EngineServiceKind::Scene,
            ENGINE_SCENE_SERVICE_ID,
            SCENE_BACKEND_CAPABILITY_ID,
            0,
            "newengine-game-runtime.scene-bridge",
        ) {
            Ok(()) => {
                log::info!(
                    "engine.scene gateway registered source=engine-owned service='{}' capability='{}'",
                    ENGINE_SCENE_SERVICE_ID,
                    SCENE_BACKEND_CAPABILITY_ID
                );
            }
            Err(e) => {
                log::error!(
                    "engine.scene gateway route registration failed id='{}' err='{}'",
                    ENGINE_SCENE_SERVICE_ID,
                    e
                );
            }
        }
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
