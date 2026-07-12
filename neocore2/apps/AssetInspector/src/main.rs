#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_core::{Engine, EngineResult, StartupConfig};
use newengine_render_ui_bridge::EngineUiDrawListBridgeProvider;
use newengine_runtime_host::app_launcher::{
    RuntimeHostAppProfile, RuntimeHostBootOption, RuntimeHostLaunchSpec, RuntimeHostLauncher,
};
use newengine_runtime_host::render_runtime::RenderBackendRuntimeModule;
use newengine_ui::{UiBuildFn, UiProviderKind};

const APP_NAME: &str = "asset-inspector";
const APP_ASSETS_ENV: &str = newengine_asset_inspector_runtime::ASSET_INSPECTOR_ASSETS_ENV;
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

struct AssetInspectorApp {
    viewport: Arc<newengine_engine_runtime::ViewportBridge>,
    plugins: Arc<newengine_engine_runtime::PluginManagerBridge>,
    scene: Arc<newengine_scene_runtime::SceneBridge>,
}

impl Default for AssetInspectorApp {
    fn default() -> Self {
        Self {
            viewport: Arc::new(newengine_engine_runtime::ViewportBridge::new()),
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
            app_dir_name: "AssetInspector",
            app_assets_env: APP_ASSETS_ENV,
            window_title: "North Star Asset Inspector",
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
        engine.register_module(Box::new(
            newengine_asset_inspector_runtime::AssetInspectorRuntimeModule::new(),
        ))?;

        let render = newengine_engine_runtime::RuntimeRenderController::new(
            Arc::clone(&self.viewport),
            Arc::clone(&self.plugins),
            Arc::clone(&self.scene),
        )
        .with_draw_list_provider(EngineUiDrawListBridgeProvider::shared());
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
        let _ = newengine_assets::register_asset_document_gateways_best_effort(host.clone());
        let assets = newengine_assets::AssetServiceClient::new(host);
        newengine_assets_ui_runtime::register_assets_ui_gateway_best_effort(assets);
    }

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
    .run_process();
}
