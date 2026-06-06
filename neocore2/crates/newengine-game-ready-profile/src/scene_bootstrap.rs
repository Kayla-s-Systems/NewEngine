#![forbid(unsafe_op_in_unsafe_fn)]

use std::{any::Any, sync::Arc};

use newengine_assets::{AssetServiceClient};
use newengine_core::{
    EngineLifecycleEvent, EngineReadinessKey, EngineReadinessSnapshot, EngineResult, Module, ModuleCtx,
};
use newengine_runtime_host::asset_bootstrap::{
    collect_app_asset_roots, mount_asset_roots_best_effort,
};
use newengine_ui_api::{UiEditorRuntimeMode, UiEditorRuntimeState, UiScreenProfile, UiScreenProfileState};

use crate::{GAME_APP_ASSETS_DIR_ENV, GAME_READY_APP_DIR_NAME};

const GAME_READY_SCENE_BOOTSTRAP_REQUIRES: &[EngineReadinessKey] = &[
    EngineReadinessKey::EnginePluginsReady,
];

pub(crate) struct GameReadySceneBootstrapModule {
    scene: Arc<newengine_scene_runtime::SceneBridge>,
    bootstrapped: bool,
    waiting_logged: bool,
    editor_deferred_logged: bool,
}

impl GameReadySceneBootstrapModule {
    #[inline]
    pub(crate) fn new(scene: Arc<newengine_scene_runtime::SceneBridge>) -> Self {
        Self {
            scene,
            bootstrapped: false,
            waiting_logged: false,
            editor_deferred_logged: false,
        }
    }

    #[inline]
    fn log_waiting_once(&mut self, origin: &'static str) {
        if self.waiting_logged {
            return;
        }
        self.waiting_logged = true;
        newengine_ulog_api::ulog::info!(
            "game-ready runtime: waiting for AssetManager/geometryImporter readiness before scene bootstrap origin='{}'",
            origin
        );
    }

    #[inline]
    fn editor_bootstrap_allowed<E: Send + 'static>(&mut self, ctx: &ModuleCtx<'_, E>, origin: &'static str) -> bool {
        let profile = ctx
            .resources()
            .get::<UiScreenProfileState>()
            .map(|state| state.descriptor.profile)
            .unwrap_or(UiScreenProfile::Editor);
        if profile != UiScreenProfile::Editor {
            return true;
        }
        let mode = ctx
            .resources()
            .get::<UiEditorRuntimeState>()
            .map(|state| state.mode)
            .unwrap_or(UiEditorRuntimeMode::Edit);
        let allowed = matches!(mode, UiEditorRuntimeMode::Simulate | UiEditorRuntimeMode::Play);
        if !allowed && !self.editor_deferred_logged {
            self.editor_deferred_logged = true;
            newengine_ulog_api::ulog::info!(
                "game-ready runtime: scene bootstrap deferred by editor profile origin='{}' mode='{}' policy='no game/world load before Simulate or Play'",
                origin,
                mode.id(),
            );
        }
        allowed
    }

    #[inline]
    fn try_bootstrap_if_allowed<E: Send + 'static>(&mut self, ctx: &mut ModuleCtx<'_, E>, origin: &'static str) -> EngineResult<()> {
        if !self.editor_bootstrap_allowed(ctx, origin) {
            return Ok(());
        }
        self.try_bootstrap(origin)
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

        match self.scene.bootstrap_profile_scene_now() {
            Some(player) => {
                self.bootstrapped = true;
                let selected_player_authority = self.scene.selection_authority_handle();
                newengine_ulog_api::ulog::info!(
                    "game-ready runtime: CPU scene bootstrapped via lifecycle dispatch origin='{}' selected_player_cache={:?} selected_player_authority={:?}; waiting for launch gate before public Play",
                    origin,
                    player,
                    selected_player_authority
                );
            }
            None => {
                newengine_ulog_api::ulog::warn!(
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
        self.try_bootstrap_if_allowed(ctx, origin)
    }

    #[inline]
    fn on_event(&mut self, ctx: &mut ModuleCtx<'_, E>, event: &dyn Any) -> EngineResult<()> {
        let Some(event) = event.downcast_ref::<EngineLifecycleEvent>() else {
            return Ok(());
        };

        match event {
            EngineLifecycleEvent::EnginePluginsReady { origin, .. } => {
                self.try_bootstrap_if_allowed(ctx, origin)
            }
            EngineLifecycleEvent::EngineStartCompleted { .. } => {
                if self.bootstrapped {
                    Ok(())
                } else if !self.editor_bootstrap_allowed(ctx, "engine-start-completed") {
                    Ok(())
                } else {
                    self.log_waiting_once("engine-start-completed");
                    Ok(())
                }
            }
        }
    }

    #[inline]
    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        if !self.bootstrapped
            && newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID)
        {
            self.try_bootstrap_if_allowed(ctx, "update-readiness-fallback")?;
        }
        Ok(())
    }
}
