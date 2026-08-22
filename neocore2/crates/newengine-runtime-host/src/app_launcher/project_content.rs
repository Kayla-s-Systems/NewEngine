use newengine_assets::AssetServiceClient;
use newengine_core::{EngineError, EngineReadinessKey, EngineResult, Module, ModuleCtx};
use newengine_project_api::{
    ContentMountNamespace, ContentMountRegistry, ProjectContentMountState, ProjectScriptRegistry,
};
use newengine_project_runtime::{mount_content_registry_best_effort, RuntimeCompositionContext};
use std::path::PathBuf;

pub(super) struct DeferredProjectContentMountModule {
    assets: AssetServiceClient,
    engine_asset_roots: Vec<PathBuf>,
    engine_roots_mounted: bool,
    mounts_ready: bool,
    entrypoint_loaded: bool,
    last_attempt_frame: Option<u64>,
}

impl DeferredProjectContentMountModule {
    pub(super) fn new(engine_asset_roots: Vec<PathBuf>) -> Self {
        Self {
            assets: AssetServiceClient::new(newengine_plugin_host::default_host_api()),
            engine_asset_roots,
            engine_roots_mounted: false,
            mounts_ready: false,
            entrypoint_loaded: false,
            last_attempt_frame: None,
        }
    }

    fn try_mounts<E: Send + 'static>(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        if self.mounts_ready {
            return self.try_entrypoint(ctx);
        }

        if self.assets.vfs_list_json_v1("").is_err() {
            return Ok(());
        }

        // Runtime-host may discover engine-owned roots before the AssetManager
        // provider is registered. The eager bootstrap attempt is intentionally
        // best-effort, so replay those roots exactly once after EnginePluginsReady
        // instead of making engine content availability depend on plugin load order.
        if !self.engine_roots_mounted {
            newengine_asset_bootstrap_runtime::mount_asset_roots_best_effort(
                &self.assets,
                &self.engine_asset_roots,
            );
            self.engine_roots_mounted = true;
            newengine_ulog_api::ulog::info!(
                "engine content: deferred VFS roots mounted roots={}",
                self.engine_asset_roots.len()
            );
        }

        let registry = ctx
            .resources()
            .get::<ContentMountRegistry>()
            .cloned()
            .unwrap_or_default();
        let mut project_registry = ContentMountRegistry::default();
        for mount in registry
            .mounts()
            .iter()
            .filter(|mount| mount.namespace != ContentMountNamespace::Engine)
        {
            project_registry
                .register(mount.clone())
                .map_err(EngineError::Other)?;
        }

        let diagnostics = mount_content_registry_best_effort(&self.assets, &project_registry);
        for diagnostic in &diagnostics {
            newengine_ulog_api::ulog::info!("project content: {}", diagnostic);
        }
        let errors = diagnostics
            .iter()
            .filter(|line| line.starts_with("ERROR:"))
            .cloned()
            .collect::<Vec<_>>();

        if let Some(state) = ctx.resources_mut().get_mut::<ProjectContentMountState>() {
            state.attempts = state.attempts.saturating_add(1);
            if errors.is_empty() {
                state.mounted = true;
                state.last_error = None;
            } else {
                state.last_error = Some(errors.join(" | "));
            }
        }

        if !errors.is_empty() {
            return Err(EngineError::Other(format!(
                "project content mount failed: {}",
                errors.join(" | ")
            )));
        }

        self.mounts_ready = true;
        newengine_ulog_api::ulog::info!(
            "project content: mount barrier ready mounts={}",
            project_registry.mounts().len()
        );
        self.try_entrypoint(ctx)
    }

    fn try_entrypoint<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
    ) -> EngineResult<()> {
        if self.entrypoint_loaded {
            return Ok(());
        }
        let scripts = ctx
            .resources()
            .get::<ProjectScriptRegistry>()
            .cloned()
            .unwrap_or_default();
        let Some(entrypoint) = scripts.entrypoint() else {
            self.entrypoint_loaded = true;
            return Ok(());
        };

        if !newengine_core::has_engine_gateway_route(newengine_assets_api::ENGINE_ASSET_SERVICE_ID)
            || !newengine_core::has_engine_gateway_route(
                newengine_scripting_api::ENGINE_SCRIPTING_SERVICE_ID,
            )
        {
            return Ok(());
        }

        let runtime_id = ctx
            .resources()
            .get::<RuntimeCompositionContext>()
            .and_then(|runtime| runtime.game_module.as_deref())
            .unwrap_or("game");
        newengine_scripting_client::AssetBackedScriptClient::new(
            entrypoint.clone(),
            format!("game-entrypoint:{runtime_id}"),
        )
        .load_module()
        .map_err(|error| {
            EngineError::Other(format!(
                "game scripting entrypoint load failed ref='{entrypoint}': {error}"
            ))
        })?;
        self.entrypoint_loaded = true;
        newengine_ulog_api::ulog::info!(
            "game scripting: entrypoint loaded ref='{}' runtime='{}'",
            entrypoint,
            scripts.runtime().unwrap_or("provider-selected")
        );
        Ok(())
    }
}

impl<E: Send + 'static> Module<E> for DeferredProjectContentMountModule {
    fn id(&self) -> &'static str {
        "engine.project.content_mounts"
    }

    fn startup_requires(&self) -> &'static [EngineReadinessKey] {
        const REQUIRES: &[EngineReadinessKey] = &[EngineReadinessKey::EnginePluginsReady];
        REQUIRES
    }

    fn start(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        self.try_mounts(ctx)
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        if self.mounts_ready && self.entrypoint_loaded {
            return Ok(());
        }
        let frame_index = ctx.frame().map(|frame| frame.frame_index).unwrap_or(0);
        if self
            .last_attempt_frame
            .is_some_and(|last| frame_index.saturating_sub(last) < 4)
        {
            return Ok(());
        }
        self.last_attempt_frame = Some(frame_index);
        self.try_mounts(ctx)
    }
}
