use std::{any::Any, sync::Arc};

use newengine_core::{
    render::SceneLaunchStatus, EngineLifecycleEvent, EngineReadinessKey, EngineReadinessSnapshot,
    EngineResult, Module, ModuleCtx, Resources,
};
use newengine_project_api::ProjectContentMountState;
use newengine_ui_api::UiPresentationFlowState;

const REQUIRES: &[EngineReadinessKey] = &[EngineReadinessKey::EnginePluginsReady];

/// Optional post-bootstrap action owned by composition rather than the authored-world runtime.
/// Typical users install project-derived runtime resources after the scene provider has completed.
pub trait AuthoredWorldBootstrapCompletion: Send + Sync {
    fn id(&self) -> &'static str;
    fn complete(
        &self,
        resources: &mut Resources,
        scene: &Arc<newengine_scene_runtime::SceneBridge>,
    ) -> Result<(), String>;
}

pub struct AuthoredWorldBootstrapModule {
    scene: Arc<newengine_scene_runtime::SceneBridge>,
    completion: Option<Arc<dyn AuthoredWorldBootstrapCompletion>>,
    bootstrapped: bool,
    failed_services_generation: Option<u64>,
    waiting_logged: bool,
    project_mount_wait_logged: bool,
    presentation_deferred_logged: bool,
}

impl AuthoredWorldBootstrapModule {
    pub fn new(scene: Arc<newengine_scene_runtime::SceneBridge>) -> Self {
        Self {
            scene,
            completion: None,
            bootstrapped: false,
            failed_services_generation: None,
            waiting_logged: false,
            project_mount_wait_logged: false,
            presentation_deferred_logged: false,
        }
    }

    #[inline]
    pub fn with_completion(
        mut self,
        completion: Arc<dyn AuthoredWorldBootstrapCompletion>,
    ) -> Self {
        self.completion = Some(completion);
        self
    }

    #[inline]
    fn log_waiting_once(&mut self, origin: &'static str, assets_ready: bool, maps_ready: bool) {
        if self.waiting_logged {
            return;
        }
        self.waiting_logged = true;
        newengine_ulog_api::ulog::info!(
            "authored-world bootstrap waiting origin='{}' assets={} maps={}",
            origin,
            assets_ready,
            maps_ready
        );
    }

    #[inline]
    fn presentation_bootstrap_allowed<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
        origin: &'static str,
    ) -> bool {
        let Some(flow) = ctx.resources().get::<UiPresentationFlowState>() else {
            return true;
        };
        if flow.allows_world_bootstrap() {
            return true;
        }
        if !self.presentation_deferred_logged {
            self.presentation_deferred_logged = true;
            newengine_ulog_api::ulog::info!(
                "authored-world bootstrap deferred by presentation flow origin='{}' flow='{}' state='{}' surface='{}'",
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
        let mounts_ready = ctx
            .resources()
            .get::<ProjectContentMountState>()
            .map(ProjectContentMountState::ready)
            .unwrap_or(true);
        if !mounts_ready {
            if !self.project_mount_wait_logged {
                self.project_mount_wait_logged = true;
                newengine_ulog_api::ulog::info!(
                    "authored-world bootstrap waiting for selected project content mounts origin='{}'",
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

        let assets_ready =
            newengine_core::has_engine_gateway_route(newengine_assets_api::ENGINE_ASSET_SERVICE_ID);
        let maps_ready = newengine_core::has_engine_gateway_route(
            newengine_assets_api::ENGINE_ASSETS_MAPS_SERVICE_ID,
        );
        if !assets_ready || !maps_ready {
            self.log_waiting_once(origin, assets_ready, maps_ready);
            return Ok(());
        }

        match self.scene.bootstrap_profile_scene_now() {
            Some(primary) => {
                if let Some(completion) = self.completion.as_ref() {
                    if let Err(error) = completion.complete(ctx.resources_mut(), &self.scene) {
                        self.failed_services_generation = Some(services_generation);
                        newengine_ulog_api::ulog::error!(
                            "authored-world bootstrap completion failed completion='{}' err='{}'",
                            completion.id(),
                            error,
                        );
                        ctx.resources_mut().insert(SceneLaunchStatus::loading(
                            "World bootstrap completion failed",
                            "Project runtime resources were not installed",
                            error,
                            0.997,
                        ));
                        return Ok(());
                    }
                }
                self.failed_services_generation = None;
                self.bootstrapped = true;
                newengine_ulog_api::ulog::info!(
                    "authored-world bootstrap ready origin='{}' primary={:?} completion='{}'",
                    origin,
                    primary,
                    self.completion
                        .as_ref()
                        .map(|completion| completion.id())
                        .unwrap_or("<none>"),
                );
            }
            None => {
                self.failed_services_generation = Some(services_generation);
                ctx.resources_mut().insert(SceneLaunchStatus::loading(
                    "Scene bootstrap failed",
                    "Authored world was not loaded",
                    "The selected authored-world scene provider failed before playable-world handoff. Check the preceding AssetManager/definition diagnostics.",
                    0.995,
                ));
            }
        }
        Ok(())
    }
}

#[inline]
fn bootstrap_attempt_allowed(
    bootstrapped: bool,
    failed_services_generation: Option<u64>,
    current_services_generation: u64,
) -> bool {
    !bootstrapped && failed_services_generation != Some(current_services_generation)
}

impl<E: Send + 'static> Module<E> for AuthoredWorldBootstrapModule {
    fn id(&self) -> &'static str {
        "engine.authored_world.bootstrap"
    }

    fn startup_requires(&self) -> &'static [EngineReadinessKey] {
        REQUIRES
    }

    fn start(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let origin = if ctx
            .resources()
            .get::<EngineReadinessSnapshot>()
            .map(|snapshot| snapshot.engine_plugins_ready)
            .unwrap_or(false)
        {
            "startup-graph-engine-plugins-ready"
        } else {
            "startup-graph-unexpected-early-start"
        };
        self.try_bootstrap_if_allowed(ctx, origin)
    }

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
                    let assets_ready = newengine_core::has_engine_gateway_route(
                        newengine_assets_api::ENGINE_ASSET_SERVICE_ID,
                    );
                    let maps_ready = newengine_core::has_engine_gateway_route(
                        newengine_assets_api::ENGINE_ASSETS_MAPS_SERVICE_ID,
                    );
                    self.log_waiting_once("engine-start-completed", assets_ready, maps_ready);
                    Ok(())
                }
            }
        }
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        if !self.bootstrapped {
            self.try_bootstrap_if_allowed(ctx, "update-readiness-fallback")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_bootstrap_retries_only_after_service_graph_revision() {
        assert!(bootstrap_attempt_allowed(false, None, 10));
        assert!(!bootstrap_attempt_allowed(false, Some(10), 10));
        assert!(bootstrap_attempt_allowed(false, Some(10), 11));
        assert!(!bootstrap_attempt_allowed(true, Some(10), 11));
    }

    #[test]
    fn absent_presentation_flow_allows_world_bootstrap() {
        let resources = Resources::default();
        assert!(resources.get::<UiPresentationFlowState>().is_none());
    }
}
