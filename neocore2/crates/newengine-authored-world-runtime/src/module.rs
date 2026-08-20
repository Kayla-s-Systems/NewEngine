use std::{any::Any, sync::Arc};

use newengine_core::{
    render::SceneLaunchStatus, EngineLifecycleEvent, EngineReadinessKey, EngineReadinessSnapshot,
    EngineResult, Module, ModuleCtx,
};
use newengine_project_api::ProjectContentMountState;

const REQUIRES: &[EngineReadinessKey] = &[EngineReadinessKey::EnginePluginsReady];

pub struct AuthoredWorldBootstrapModule {
    scene: Arc<newengine_scene_runtime::SceneBridge>,
    bootstrapped: bool,
    waiting_logged: bool,
}

impl AuthoredWorldBootstrapModule {
    pub fn new(scene: Arc<newengine_scene_runtime::SceneBridge>) -> Self {
        Self {
            scene,
            bootstrapped: false,
            waiting_logged: false,
        }
    }

    fn try_bootstrap<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
        origin: &'static str,
    ) -> EngineResult<()> {
        if self.bootstrapped {
            return Ok(());
        }
        let mounts_ready = ctx
            .resources()
            .get::<ProjectContentMountState>()
            .map(ProjectContentMountState::ready)
            .unwrap_or(true);
        let assets_ready =
            newengine_core::has_engine_gateway_route(newengine_assets_api::ENGINE_ASSET_SERVICE_ID);
        let maps_ready = newengine_core::has_engine_gateway_route(
            newengine_assets_api::ENGINE_ASSETS_MAPS_SERVICE_ID,
        );
        if !mounts_ready || !assets_ready || !maps_ready {
            if !self.waiting_logged {
                self.waiting_logged = true;
                newengine_ulog_api::ulog::info!(
                    "authored-world bootstrap waiting origin='{}' mounts={} assets={} maps={}",
                    origin,
                    mounts_ready,
                    assets_ready,
                    maps_ready
                );
            }
            return Ok(());
        }

        match self.scene.bootstrap_profile_scene_now() {
            Some(primary) => {
                self.bootstrapped = true;
                newengine_ulog_api::ulog::info!(
                    "authored-world bootstrap ready origin='{}' primary={:?}",
                    origin,
                    primary
                );
            }
            None => {
                ctx.resources_mut().insert(SceneLaunchStatus::loading(
                    "Scene bootstrap failed",
                    "Authored YMAP was not loaded",
                    "The generic authored-world runtime could not materialize the selected startup .ymap. Check engine.assets.maps and engine.assets.definitions diagnostics.",
                    0.995,
                ));
            }
        }
        Ok(())
    }
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
        self.try_bootstrap(ctx, origin)
    }
    fn on_event(&mut self, ctx: &mut ModuleCtx<'_, E>, event: &dyn Any) -> EngineResult<()> {
        if let Some(EngineLifecycleEvent::EnginePluginsReady { origin, .. }) =
            event.downcast_ref::<EngineLifecycleEvent>()
        {
            self.try_bootstrap(ctx, origin)?;
        }
        Ok(())
    }
    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        if !self.bootstrapped {
            self.try_bootstrap(ctx, "update-readiness-fallback")?;
        }
        Ok(())
    }
}
