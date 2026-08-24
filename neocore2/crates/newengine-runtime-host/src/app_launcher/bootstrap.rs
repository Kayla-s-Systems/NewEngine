use newengine_asset_bootstrap_runtime::{collect_app_asset_roots, mount_asset_roots_best_effort};
use std::{path::Path, sync::Arc};

use newengine_assets::AssetServiceClient;
use newengine_core::{
    ConfigPaths, Engine, EngineError, EngineResult, PluginDiscoveryRoot, StartupConfig,
    StartupLoader,
};
use newengine_project_api::{
    ContentMountRegistry, ProjectContentMountState, PROJECT_STARTUP_SCENE_ENV,
};
use newengine_project_runtime::{
    apply_resolved_project_launch_env, game_manifest_request_from_process,
    load_project_from_request_with_launch, project_launch_request_from_process,
    project_request_from_process, register_engine_asset_roots, ProjectRuntimeContext,
    RuntimeCompositionContext,
};

use crate::engine_factory::build_engine_from_startup;

use super::boot_options::{apply_declared_boot_options_env, boot_option_enabled};
use super::types::{
    RuntimeHostAppProfile, RuntimeHostFrontend, RuntimeHostFrontendContext, RuntimeHostLauncher,
};

impl<P> RuntimeHostLauncher<P>
where
    P: RuntimeHostAppProfile,
{
    pub fn run_with_frontend<F>(&self, frontend: &F) -> EngineResult<()>
    where
        F: RuntimeHostFrontend<P>,
    {
        self.early_log(format_args!("run.begin app={}", self.spec.app_name));
        let run_id = newengine_core::init_run_id().to_owned();
        self.bind_early_log_to_run(&run_id);
        self.early_log(format_args!("run_id.init.ok run_id={}", run_id));
        newengine_ulog_api::ulog::info_event!(
            "engine.startup.run_id",
            "Run ID initialized",
            {
                "app_name": self.spec.app_name,
                "run_id": run_id.as_str()
            }
        );
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: run start run_id={}",
            self.spec.app_name, run_id
        ));

        std::env::set_var("NEWENGINE_RUN_ID", &run_id);
        self.spec.apply_env_defaults();
        let boot_options = self.profile.boot_options();
        apply_declared_boot_options_env(self.spec.app_name, boot_options);
        self.install_error_reporter();

        // PreInit providers are composition-owned. A profile may register an
        // alternative engine.host.capabilities route before the native fallback
        // is considered.
        newengine_plugin_host::init_host_context();
        self.profile
            .register_preinit_provider_routes_best_effort();

        // Void Engine Host PreInit is deliberately before project/runtime composition.
        // OS/hardware discovery produces immutable DTOs and installs only generic
        // gateway-selection policy; no game module or concrete runtime exists yet.
        let host_preinit = crate::preinit::run_host_preinit();

        let editor_project_request = project_request_from_process();
        let game_request = game_manifest_request_from_process();
        if game_request.is_none()
            && boot_option_enabled(
                boot_options,
                super::boot_options::RuntimeHostBootOption::ProjectBrowser,
            )
            && !super::env_bool("NEWENGINE_HEADLESS", false)
            && !super::env_bool("NEWENGINE_PROJECT_BROWSER_DISABLED", false)
        {
            return Err(EngineError::Other(
                "runtime host does not discover projects; editor must resolve game.toml before runtime launch"
                    .to_owned(),
            ));
        }
        let project_launch_request = project_launch_request_from_process();
        let game_context = match game_request {
            Some(request) => {
                let context = load_project_from_request_with_launch(
                    &request,
                    project_launch_request.as_deref(),
                )
                .map_err(|error| {
                    EngineError::Other(format!(
                        "game manifest load failed request='{}': {error}",
                        request.display()
                    ))
                })?;
                let editor_owned = editor_project_request.is_some();
                if editor_owned {
                    std::env::set_var("NEWENGINE_PROJECT_ROOT", &context.project_root);
                    std::env::set_var("NEWENGINE_PROJECT_MANIFEST", &context.manifest_path);
                } else {
                    std::env::set_var("NEWENGINE_GAME_ROOT", &context.project_root);
                    std::env::set_var(
                        newengine_project_api::GAME_MANIFEST_ENV,
                        &context.manifest_path,
                    );
                }
                if let Some(startup_scene) = context
                    .launch
                    .startup_scene
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    std::env::set_var(PROJECT_STARTUP_SCENE_ENV, startup_scene);
                } else {
                    std::env::remove_var(PROJECT_STARTUP_SCENE_ENV);
                }
                apply_resolved_project_launch_env(&context.launch);
                self.early_log(format_args!(
                    "game.manifest.loaded id={} root={} manifest={} mounts={} launch={} launch_profile={} runtime_profile={} editor_owned={}",
                    context.manifest.id,
                    context.project_root.display(),
                    context.manifest_path.display(),
                    context.mounts.mounts().len(),
                    context.launch.preset_id,
                    context.launch.profile.id(),
                    context.launch.runtime_profile.as_deref().unwrap_or("app-default"),
                    editor_owned,
                ));
                Some(context)
            }
            None => None,
        };
        let runtime_context = game_context
            .as_ref()
            .map(RuntimeCompositionContext::from_project);
        frontend.prepare_startup(&self.profile, &self.spec)?;
        let mut startup = self.load_startup_config()?;
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: startup config loaded",
            self.spec.app_name
        ));
        self.configure_sharded_log_files(Arc::make_mut(&mut startup), &run_id);

        let asset_roots = collect_app_asset_roots(self.spec.app_dir_name, self.spec.app_assets_env);
        if let Some(engine_content_root) = asset_roots.iter().find(|root| {
            root.file_name().and_then(|name| name.to_str()) == Some("Content")
                && root
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str())
                    == Some("Engine")
        }) {
            std::env::set_var("NEWENGINE_ENGINE_CONTENT_ROOT", engine_content_root);
        }
        self.early_log(format_args!(
            "asset_roots.collected count={}",
            asset_roots.len()
        ));

        let mut content_mount_registry = ContentMountRegistry::default();
        register_engine_asset_roots(&mut content_mount_registry, &asset_roots)
            .map_err(EngineError::Other)?;
        if let Some(runtime) = runtime_context.as_ref() {
            for mount in runtime.mounts.mounts() {
                content_mount_registry
                    .register(mount.clone())
                    .map_err(EngineError::Other)?;
            }
        }

        let mut engine = self.build_engine(&startup)?;
        // Runtime consumers receive an immutable snapshot. Provider selection has
        // already consumed the derived generic policy; systems never re-probe OS hardware.
        engine.resources_mut().insert(Arc::clone(&host_preinit));
        newengine_runtime_session_runtime::init_runtime_session_command_service();

        if let Some(game) = game_context.as_ref() {
            configure_game_plugin_roots(&mut engine, game)?;
        }
        engine
            .resources_mut()
            .insert(content_mount_registry.clone());
        if let Some(runtime) = runtime_context.clone() {
            engine.resources_mut().insert(runtime.scripts.clone());
            engine
                .resources_mut()
                .insert::<RuntimeCompositionContext>(runtime);
            if editor_project_request.is_some() {
                if let Some(project) = game_context.clone() {
                    engine
                        .resources_mut()
                        .insert::<ProjectRuntimeContext>(project);
                    newengine_asset_hot_reload_runtime::install_asset_file_watcher(
                        engine.resources_mut(),
                    );
                }
            }
            engine
                .resources_mut()
                .insert(ProjectContentMountState::pending());
            engine.register_module(Box::new(
                super::project_content::DeferredProjectContentMountModule::new(asset_roots.clone()),
            ))?;
            self.early_log(format_args!(
                "runtime.content.bootstrap deferred-module=true mounts={} project_hot_reload={}",
                content_mount_registry.mounts().len(),
                editor_project_request.is_some(),
            ));
        } else {
            engine
                .resources_mut()
                .insert(ProjectContentMountState::default());
        }
        self.profile.initialize_composition_services(
            &mut engine,
            host_preinit.as_ref(),
            runtime_context.as_ref(),
        )?;
        self.initialize_profile_and_plugins(&mut engine, &startup, boot_options)?;

        let asset_host = newengine_plugin_host::default_host_api();
        let asset_gateway_available =
            newengine_core::has_engine_gateway_route(newengine_assets_api::ENGINE_ASSET_SERVICE_ID);
        let asset_provider_available =
            newengine_plugin_host::has_service(newengine_assets_api::ASSET_PROVIDER_SERVICE_ID);
        let assets_available = asset_gateway_available || asset_provider_available;
        let assets = if asset_gateway_available {
            AssetServiceClient::new(asset_host.clone())
        } else if asset_provider_available {
            AssetServiceClient::for_service(
                asset_host.clone(),
                newengine_assets_api::ASSET_PROVIDER_SERVICE_ID,
            )
        } else {
            AssetServiceClient::new(asset_host.clone())
        };
        self.early_log(format_args!(
            "asset_service.availability available={} gateway={} gateway_ready={} provider={} provider_ready={}",
            assets_available,
            newengine_assets_api::ENGINE_ASSET_SERVICE_ID,
            asset_gateway_available,
            newengine_assets_api::ASSET_PROVIDER_SERVICE_ID,
            asset_provider_available,
        ));

        if assets_available {
            // Compatibility roots remain mounted at the legacy empty prefix so existing
            // asset refs continue to work. The new registry adds namespaced project/game/plugin/user
            // mounts alongside them instead of silently changing path semantics.
            mount_asset_roots_best_effort(&assets, &asset_roots);
            self.early_log(format_args!(
                "asset_roots.mount.requested count={} registry_mounts={} project={} project_mounts=deferred-module",
                asset_roots.len(),
                content_mount_registry.mounts().len(),
                editor_project_request.is_some(),
            ));
        } else {
            newengine_ulog_api::ulog::warn!(
                "{} launcher: engine.assets route unavailable after profile registration; asset root mount skipped until provider readiness",
                self.spec.app_name
            );
            newengine_core::crash::record_breadcrumb(format!(
                "{} launcher: engine.assets unavailable during initial asset mount",
                self.spec.app_name
            ));
        }

        frontend.launch(
            &self.profile,
            engine,
            RuntimeHostFrontendContext {
                launch_spec: &self.spec,
                startup: &startup,
                assets_available,
                assets: &assets,
                asset_roots: &asset_roots,
            },
        )
    }

    /// Generic runtime-host default: platformless/headless control plane. Windowed
    /// products must explicitly provide `newengine-windowed-host-runtime`.
    pub fn run(&self) -> EngineResult<()> {
        self.run_with_frontend(&crate::HeadlessRuntimeFrontend)
    }

    fn install_error_reporter(&self) {
        self.early_log(format_args!("error_reporter.install.begin"));
        newengine_core::EngineErrorReporter::install(newengine_core::EngineErrorReporterConfig {
            crash: newengine_core::crash::CrashReporterConfig {
                product_name: self.spec.product_name.to_owned(),
                app_name: self.spec.app_name.to_owned(),
                app_version: self.spec.app_version.to_owned(),
                spawn_reporter: std::env::var_os("NEWENGINE_CRASH_REPORTER_PATH").is_some(),
                ..Default::default()
            },
            ..Default::default()
        });
        self.early_log(format_args!("error_reporter.install.ok"));
    }

    fn load_startup_config(&self) -> EngineResult<Arc<StartupConfig>> {
        let startup_path = std::env::var(crate::runtime_config::ENGINE_STARTUP_CONFIG_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| self.spec.startup_config_path.to_owned());
        let paths = ConfigPaths::from_startup_str(&startup_path);
        self.early_log(format_args!("startup.load.begin path={}", startup_path));
        let (startup, _report) = StartupLoader::load_json(&paths)?;
        self.early_log(format_args!(
            "startup.load.ok modules_dir={} cache_files={} config={}",
            startup.modules_dir.display(),
            startup.resolved_cache_files_dir().display(),
            startup.resolved_config_dir().display()
        ));
        Ok(Arc::new(startup))
    }

    fn build_engine(&self, startup: &StartupConfig) -> EngineResult<Engine<()>> {
        self.early_log(format_args!("engine.build.begin"));
        let engine = build_engine_from_startup(startup, self.spec.fixed_dt_ms)?;
        self.early_log(format_args!("engine.build.ok"));
        newengine_core::crash::record_breadcrumb(format!(
            "{} launcher: host engine constructed",
            self.spec.app_name
        ));
        Ok(engine)
    }
}

fn configure_game_plugin_roots(
    engine: &mut Engine<()>,
    project: &ProjectRuntimeContext,
) -> EngineResult<()> {
    let project_id = project.manifest.id.trim();
    let conventional_plugins = project.project_root.join("plugins");
    engine.add_plugin_discovery_root(
        PluginDiscoveryRoot::new(
            conventional_plugins,
            newengine_plugin_host::PluginLoadOrigin::GamePlugin,
        )
        .with_owner(format!("project:{project_id}:plugins")),
    )?;

    for plugin in &project.manifest.plugins {
        let plugin_id = plugin.id.trim();
        if plugin.required && !plugin_id.is_empty() {
            engine.require_plugin_id(plugin_id.to_owned())?;
        }
        let Some(path) = plugin.path.as_ref() else {
            continue;
        };
        let path = if path.is_absolute() {
            path.clone()
        } else {
            project.project_root.join(path)
        };
        let root_dir = if path.is_dir() {
            path
        } else if path.extension().is_some() {
            path.parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| project.project_root.clone())
        } else {
            path
        };
        engine.add_plugin_discovery_root(
            PluginDiscoveryRoot::new(
                root_dir,
                newengine_plugin_host::PluginLoadOrigin::GamePlugin,
            )
            .required(plugin.required)
            .with_owner(if plugin_id.is_empty() {
                format!("project:{project_id}:plugin")
            } else {
                format!("project:{project_id}:plugin:{plugin_id}")
            }),
        )?;
    }

    if let Some(game_module) = project
        .manifest
        .game_module
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        newengine_ulog_api::ulog::info!(
            "game-module boundary: module id='{}' project='{}' resolution='engine.game.module + composition registry'",
            game_module,
            project.manifest.id,
        );
    }
    Ok(())
}
