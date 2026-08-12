#![forbid(unsafe_op_in_unsafe_fn)]

use std::{any::Any, sync::Arc};

use newengine_assets::AssetServiceClient;
use newengine_core::{
    render::SceneLaunchStatus, EngineLifecycleEvent, EngineReadinessKey, EngineReadinessSnapshot,
    EngineResult, Module, ModuleCtx, Resources,
};
use newengine_runtime_host::asset_bootstrap::mount_profile_content_best_effort;
use newengine_ui_api::{
    UiEditorRuntimeMode, UiEditorRuntimeState, UiPresentationFlowState, UiScreenProfile,
    UiScreenProfileState,
};

use crate::GAME_READY_MOUNT_SPEC;

/// Product-owned adapter for the generic engine scene-bootstrap boundary.
/// The engine never selects this provider by name; the active application profile injects it.
pub(crate) struct GameReadyWorldSceneBootstrapProvider;

impl GameReadyWorldSceneBootstrapProvider {
    #[inline]
    pub(crate) fn shared() -> Arc<dyn newengine_engine_runtime::SceneBootstrapProvider> {
        Arc::new(Self)
    }
}

impl newengine_engine_runtime::SceneBootstrapProvider for GameReadyWorldSceneBootstrapProvider {
    #[inline]
    fn id(&self) -> &'static str {
        "app.game-ready.world-bootstrap"
    }

    fn bootstrap(
        &self,
        ctx: &mut newengine_engine_runtime::SceneBootstrapContext<'_>,
    ) -> Result<newengine_engine_runtime::SceneBootstrapResult, String> {
        let primary = newengine_game_ready_world::bootstrap_world_scene(
            ctx.scene,
            ctx.primitives,
            ctx.materials,
        );
        primary
            .map(|entity| newengine_engine_runtime::SceneBootstrapResult::new(Some(entity)))
            .ok_or_else(|| {
                "authored GameReady world bootstrap returned no primary entity".to_owned()
            })
    }
}

const GAME_READY_SCENE_BOOTSTRAP_REQUIRES: &[EngineReadinessKey] =
    &[EngineReadinessKey::EnginePluginsReady];

pub(crate) struct GameReadySceneBootstrapModule {
    scene: Arc<newengine_scene_runtime::SceneBridge>,
    bootstrapped: bool,
    waiting_logged: bool,
    editor_deferred_logged: bool,
    presentation_deferred_logged: bool,
}

impl GameReadySceneBootstrapModule {
    #[inline]
    pub(crate) fn new(scene: Arc<newengine_scene_runtime::SceneBridge>) -> Self {
        Self {
            scene,
            bootstrapped: false,
            waiting_logged: false,
            editor_deferred_logged: false,
            presentation_deferred_logged: false,
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
    fn editor_bootstrap_allowed<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        origin: &'static str,
    ) -> bool {
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
        let allowed = matches!(
            mode,
            UiEditorRuntimeMode::Simulate | UiEditorRuntimeMode::Play
        );
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
    fn presentation_bootstrap_allowed<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        origin: &'static str,
    ) -> bool {
        if presentation_flow_allows_bootstrap(ctx.resources()) {
            return true;
        }
        let flow = ctx
            .resources()
            .get::<UiPresentationFlowState>()
            .expect("blocked presentation flow state");
        if !self.presentation_deferred_logged {
            self.presentation_deferred_logged = true;
            newengine_ulog_api::ulog::info!(
                "game-ready runtime: scene bootstrap deferred by authored presentation flow origin='{}' flow='{}' state='{}' surface='{}'",
                origin,
                flow.flow_id,
                flow.state_id,
                flow.active_surface_id.as_deref().unwrap_or("<none>"),
            );
        }
        false
    }

    #[inline]
    fn try_bootstrap_if_allowed<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
        origin: &'static str,
    ) -> EngineResult<()> {
        if !self.editor_bootstrap_allowed(ctx, origin)
            || !self.presentation_bootstrap_allowed(ctx, origin)
        {
            return Ok(());
        }
        self.try_bootstrap(ctx, origin)
    }

    #[inline]
    fn try_bootstrap<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
        origin: &'static str,
    ) -> EngineResult<()> {
        if self.bootstrapped {
            return Ok(());
        }

        if !newengine_plugin_host::has_service(newengine_assets::consts::ASSET_SERVICE_ID) {
            self.log_waiting_once(origin);
            return Ok(());
        }

        let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
        mount_profile_content_best_effort(&assets, GAME_READY_MOUNT_SPEC);

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
                    "game-ready runtime: scene bootstrap failed after readiness dispatch origin='{}'; publishing engine.ui loading failure overlay",
                    origin
                );
                ctx.resources_mut().insert(SceneLaunchStatus::loading(
                    "Scene bootstrap failed",
                    "Authored world was not loaded",
                    "The strict data-driven .ymap bootstrap failed before playable-world handoff. Emergency fallback profiles are forbidden, so the host keeps the loading/error surface visible instead of presenting a black viewport. Check the preceding game-ready asset diagnostics for the exact AssetManager decode failure.",
                    0.995,
                ));
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
                if self.bootstrapped
                    || !self.editor_bootstrap_allowed(ctx, "engine-start-completed")
                    || !self.presentation_bootstrap_allowed(ctx, "engine-start-completed")
                {
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

fn presentation_flow_allows_bootstrap(resources: &Resources) -> bool {
    resources
        .get::<UiPresentationFlowState>()
        .map(UiPresentationFlowState::allows_world_bootstrap)
        .unwrap_or(true)
}

#[cfg(test)]
mod presentation_flow_tests {
    use super::*;

    #[test]
    fn absent_presentation_flow_keeps_legacy_bootstrap_behavior() {
        assert!(presentation_flow_allows_bootstrap(&Resources::default()));
    }

    #[test]
    fn authored_frontend_can_gate_world_bootstrap() {
        let mut resources = Resources::default();
        resources.insert(UiPresentationFlowState {
            flow_id: "game.frontend".to_owned(),
            state_id: "main_menu".to_owned(),
            blocks_world_bootstrap: true,
            blocks_gameplay_input: true,
            ..UiPresentationFlowState::default()
        });
        assert!(!presentation_flow_allows_bootstrap(&resources));

        resources
            .get_mut::<UiPresentationFlowState>()
            .expect("flow state")
            .blocks_world_bootstrap = false;
        assert!(presentation_flow_allows_bootstrap(&resources));
    }
}
