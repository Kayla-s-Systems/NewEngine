#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;
use std::sync::Arc;

use newengine_asset_bootstrap_runtime::{collect_app_asset_roots, mount_asset_roots_best_effort};
use newengine_assets::AssetServiceClient;
use newengine_core::{Engine, EngineReadinessKey, EngineResult, Module, ModuleCtx, StartupConfig};
use newengine_render_runtime_adapter::RenderBackendRuntimeModule;
use newengine_runtime_host::app_launcher::{
    RuntimeHostAppProfile, RuntimeHostBootOption, RuntimeHostLaunchSpec, RuntimeHostLauncher,
};
use newengine_ui::{UiBuildFn, UiProviderKind};
use newengine_windowed_host_runtime::{WindowedHostFrontend, WindowedRuntimeHostProfile};

const APP_NAME: &str = "asset-inspector";
const WINDOW_TITLE: &str = "North Star Asset Inspector";
const APP_DIR_NAME: &str = "AssetInspector";
const APP_ASSETS_ENV: &str = newengine_asset_inspector_runtime::ASSET_INSPECTOR_ASSETS_ENV;
const INSPECTOR_UI_ASSET: &str = "ui/tools/asset_inspector.neui";
const ENV_POLICY: &[(&str, &str)] = &[
    ("NEWENGINE_REQUIRE_RENDER_BACKEND", "1"),
    ("NEWENGINE_REQUIRE_ASSET_MANAGER", "1"),
    ("NEWENGINE_PLUGIN_TARGET", "runtime"),
    ("NEWENGINE_BOOTSTRAP_PLUGIN_PRELOAD", "deferred"),
];

const BOOT_OPTIONS: &[RuntimeHostBootOption] = &[
    RuntimeHostBootOption::RuntimeBootstrapOverlay,
    RuntimeHostBootOption::RuntimePlugins,
    RuntimeHostBootOption::PlatformWindow,
    RuntimeHostBootOption::RenderBackend,
];

/// Late VFS bootstrap used because standalone applications defer plugin loading
/// until the platform window and diagnostics are available.
///
/// Path discovery remains RuntimeHost-owned. This module only requests the
/// discovered roots to be mounted through engine.assets and verifies the
/// concrete product surface instead of treating the built-in `assets/` layer as
/// equivalent to the outer `gameAssets` root.
struct DeferredAssetRootMountModule {
    assets: AssetServiceClient,
    roots: Vec<PathBuf>,
    roots_mounted: bool,
    ui_asset_available: bool,
    last_attempt_frame: Option<u64>,
}

impl DeferredAssetRootMountModule {
    fn new() -> Self {
        Self {
            assets: AssetServiceClient::new(newengine_plugin_host::default_host_api()),
            roots: collect_app_asset_roots(APP_DIR_NAME, APP_ASSETS_ENV),
            roots_mounted: false,
            ui_asset_available: false,
            last_attempt_frame: None,
        }
    }

    fn mount_runtime_roots(&mut self) {
        if !self.roots_mounted {
            // A product surface may already exist in a package layer, but that
            // does not mean the complete authored gameAssets tree is mounted.
            // Asset Inspector must see every runtime/source asset root exactly as
            // the game does, so mount discovery cannot be gated by one UI file.
            if self.assets.vfs_list_json_v1("").is_err() {
                return;
            }
            mount_asset_roots_best_effort(&self.assets, &self.roots);
            self.roots_mounted = true;
            newengine_ulog_api::ulog::info!(
                "asset inspector: mounted discovered asset roots count={}",
                self.roots.len()
            );
        }
        if !self.ui_asset_available {
            self.ui_asset_available = self.assets.raw_bytes_v1(INSPECTOR_UI_ASSET).is_ok();
        }
    }
}

impl<E: Send + 'static> Module<E> for DeferredAssetRootMountModule {
    fn id(&self) -> &'static str {
        "app.asset_inspector.asset_bootstrap"
    }

    fn startup_requires(&self) -> &'static [EngineReadinessKey] {
        const REQUIRES: &[EngineReadinessKey] = &[EngineReadinessKey::EnginePluginsReady];
        REQUIRES
    }

    fn update(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        if self.roots_mounted && self.ui_asset_available {
            return Ok(());
        }
        let frame_index = ctx.frame().map(|frame| frame.frame_index).unwrap_or(0);
        if self
            .last_attempt_frame
            .is_some_and(|last| frame_index.saturating_sub(last) < 30)
        {
            return Ok(());
        }
        self.last_attempt_frame = Some(frame_index);
        self.mount_runtime_roots();
        Ok(())
    }
}

struct AssetInspectorApp {
    viewport: Arc<newengine_viewport_bridge::ViewportBridge>,
    plugins: Arc<newengine_engine_runtime::PluginManagerBridge>,
    scene: Arc<newengine_scene_runtime::SceneBridge>,
}

impl Default for AssetInspectorApp {
    fn default() -> Self {
        Self {
            viewport: Arc::new(newengine_viewport_bridge::ViewportBridge::new()),
            plugins: Arc::new(newengine_engine_runtime::PluginManagerBridge::new()),
            scene: Arc::new(newengine_scene_runtime::SceneBridge::new(
                newengine_scene::Scene::new(),
            )),
        }
    }
}

impl AssetInspectorApp {
    fn launch_spec() -> RuntimeHostLaunchSpec {
        RuntimeHostLaunchSpec {
            product_name: "North Star",
            app_name: APP_NAME,
            app_version: env!("CARGO_PKG_VERSION"),
            startup_config_path: "apps/AssetInspector/config.json",
            fixed_dt_ms: 16,
            app_dir_name: APP_DIR_NAME,
            app_assets_env: APP_ASSETS_ENV,
            early_log_file_name: "asset-inspector-early.log",
            default_profile_env: None,
            env_defaults: ENV_POLICY,
        }
    }
}

impl RuntimeHostAppProfile for AssetInspectorApp {
    fn register_modules(
        &self,
        engine: &mut Engine<()>,
        startup: &StartupConfig,
    ) -> EngineResult<()> {
        engine.register_module(Box::new(RenderBackendRuntimeModule::new(
            startup.modules_dir.clone(),
        )))?;
        engine.register_module(Box::new(DeferredAssetRootMountModule::new()))?;
        let preview_api = Arc::new(newengine_asset_preview_runtime::AssetPreviewApi::new(
            Arc::clone(&self.viewport),
        ));
        let preview_draw_lists = preview_api.draw_list_provider();
        engine.register_module(Box::new(
            newengine_asset_inspector_runtime::AssetInspectorRuntimeModule::new(preview_api),
        ))?;

        let render = newengine_engine_runtime::RuntimeRenderController::new(
            Arc::clone(&self.viewport),
            Arc::clone(&self.plugins),
            Arc::clone(&self.scene),
        )
        .with_material_pipeline_provider(Box::new(
            newengine_material_domain_gameready::GameReadyLitMaterialDomainProvider::new(),
        ))
        .with_primary_lit_material_domain(
            newengine_material_domain_gameready::GAME_READY_LIT_PIPELINE_KEY,
        )
        .with_draw_list_provider(preview_draw_lists);
        engine.register_module(Box::new(render))?;
        Ok(())
    }

    fn boot_options(&self) -> Option<&'static [RuntimeHostBootOption]> {
        Some(BOOT_OPTIONS)
    }

    fn register_engine_provider_routes_best_effort(&self) {
        newengine_schema_runtime::register_schema_gateway_best_effort();
        newengine_assets::register_asset_types_gateway_best_effort();
        let host = newengine_plugin_host::default_host_api();
        for descriptor in newengine_asset_format_nef8::descriptors() {
            let _ = newengine_assets::register_asset_type_descriptor_best_effort(&host, descriptor);
        }

        let assets = newengine_assets::AssetServiceClient::new(host.clone());
        let _ = newengine_material_runtime::register_materials_gateway_best_effort_with_host(
            Some(host.clone()),
            assets.clone(),
        );
        let _ =
            newengine_definitions_runtime::register_definitions_gateway_best_effort(assets.clone());
        let _ = newengine_model_runtime::register_model_gateway_best_effort_with_host(
            host.clone(),
            assets.clone(),
        );
        let _ = newengine_model_runtime::register_asset_graph_gateway_best_effort(
            host.clone(),
            assets.clone(),
        );

        // These are engine-owned semantic facades. They can be registered before
        // StarVault loads because AssetServiceClient resolves engine.assets only
        // when an inspect/edit request is actually invoked.
        let _ = newengine_assets::register_asset_document_gateways_best_effort(host);
        newengine_assets_ui_runtime::register_assets_ui_gateway_best_effort(assets);
    }
}

impl WindowedRuntimeHostProfile for AssetInspectorApp {
    fn ui_build_from_startup(&self, _startup: &StartupConfig) -> Option<Box<dyn UiBuildFn>> {
        None
    }

    fn ui_provider_kind_from_startup(&self, _startup: &StartupConfig) -> UiProviderKind {
        UiProviderKind::Null
    }
}

fn main() {
    RuntimeHostLauncher::new(
        AssetInspectorApp::launch_spec(),
        AssetInspectorApp::default(),
    )
    .run_process_with_frontend(WindowedHostFrontend::new(WINDOW_TITLE));
}
