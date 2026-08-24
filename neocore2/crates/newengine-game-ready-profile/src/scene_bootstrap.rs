#![forbid(unsafe_op_in_unsafe_fn)]

use std::{any::Any, sync::Arc};

use newengine_asset_bootstrap_runtime::mount_profile_content_best_effort;
use newengine_assets::AssetServiceClient;
use newengine_core::{
    render::SceneLaunchStatus, EngineLifecycleEvent, EngineReadinessKey, EngineReadinessSnapshot,
    EngineResult, Module, ModuleCtx, Resources,
};
use newengine_game_data::GameDataProvider;
use newengine_project_api::ProjectContentMountState;
use newengine_ui_api::UiPresentationFlowState;

use crate::GAME_READY_MOUNT_SPEC;

/// Product-owned adapter for the generic engine scene-bootstrap boundary.
/// The engine never selects this provider by name; the active application profile injects it.
pub(crate) struct GameReadyWorldSceneBootstrapProvider {
    game_data_provider: Arc<dyn GameDataProvider>,
}

impl GameReadyWorldSceneBootstrapProvider {
    #[inline]
    pub(crate) fn shared(
        game_data_provider: Arc<dyn GameDataProvider>,
    ) -> Arc<dyn newengine_engine_runtime::SceneBootstrapProvider> {
        Arc::new(Self { game_data_provider })
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
        let provider_id = self.game_data_provider.id();
        let snapshot = self
            .game_data_provider
            .load_snapshot()
            .map_err(|error| format!("game-data provider '{}' failed: {error}", provider_id))?;
        snapshot.data().validate().map_err(|error| {
            format!(
                "game-data provider '{}' produced invalid snapshot: {error}",
                provider_id
            )
        })?;
        let primary = newengine_game_ready_world::bootstrap_world_scene_with_data(
            ctx.scene,
            ctx.primitives,
            ctx.materials,
            snapshot,
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
    failed_services_generation: Option<u64>,
    waiting_logged: bool,
    project_mount_wait_logged: bool,
    presentation_deferred_logged: bool,
}

impl GameReadySceneBootstrapModule {
    #[inline]
    pub(crate) fn new(scene: Arc<newengine_scene_runtime::SceneBridge>) -> Self {
        Self {
            scene,
            bootstrapped: false,
            failed_services_generation: None,
            waiting_logged: false,
            project_mount_wait_logged: false,
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
        let project_content_ready = ctx
            .resources()
            .get::<ProjectContentMountState>()
            .map(ProjectContentMountState::ready)
            .unwrap_or(true);
        if !project_content_ready {
            if !self.project_mount_wait_logged {
                self.project_mount_wait_logged = true;
                newengine_ulog_api::ulog::info!(
                    "game-ready runtime: waiting for selected project content mounts before scene bootstrap origin='{}'",
                    origin,
                );
            }
            return Ok(());
        }
        if !self.presentation_bootstrap_allowed(ctx, origin) {
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
        let services_generation = newengine_plugin_host::services_generation();
        if !bootstrap_attempt_allowed(
            self.bootstrapped,
            self.failed_services_generation,
            services_generation,
        ) {
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
                self.failed_services_generation = None;
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
                self.failed_services_generation = Some(services_generation);
                newengine_ulog_api::ulog::warn!(
                    "game-ready runtime: scene bootstrap failed after readiness dispatch origin='{}' service_generation={}; publishing engine.ui loading failure overlay and suspending retry until the capability graph changes",
                    origin,
                    services_generation
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

fn bootstrap_attempt_allowed(
    bootstrapped: bool,
    failed_services_generation: Option<u64>,
    current_services_generation: u64,
) -> bool {
    !bootstrapped && failed_services_generation != Some(current_services_generation)
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
    fn failed_bootstrap_retries_only_after_service_graph_revision() {
        assert!(bootstrap_attempt_allowed(false, None, 10));
        assert!(!bootstrap_attempt_allowed(false, Some(10), 10));
        assert!(bootstrap_attempt_allowed(false, Some(10), 11));
        assert!(!bootstrap_attempt_allowed(true, Some(10), 11));
    }

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
