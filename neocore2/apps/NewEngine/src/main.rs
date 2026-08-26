#![forbid(unsafe_op_in_unsafe_fn)]

use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use newengine_plugin_api::Blob;
use newengine_plugin_host::{
    call_service_v1, default_host_api, has_service, init_host_context, PluginLoadOrigin,
    PluginManager,
};
use newengine_project_api::{
    runtime_profile_service_id_for_game, GAME_MANIFEST_ENV, PROJECT_BROWSER_PRESENT_METHOD_V1,
    PROJECT_BROWSER_SERVICE_ID, PROJECT_MANIFEST_FILE, RUNTIME_PROFILE_LAUNCH_METHOD_V1,
};
use newengine_project_runtime::{
    adjacent_game_manifest_from_exe, default_projects_root, game_manifest_request_from_process,
    load_project_from_request_with_launch, project_launch_request_from_process,
    ProjectBrowserSelection,
};
use newengine_runtime_host::runtime_config::{load_engine_runtime_config, ENGINE_RUNTIME_MODE_ENV};

const DEFAULT_PROJECT_BROWSER_PLUGIN_ID: &str = "newengine.project-browser.egui";
const PROJECT_BROWSER_PLUGIN_ID_ENV: &str = "NEWENGINE_PROJECT_BROWSER_PLUGIN_ID";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("NewEngine: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let (runtime_config_path, runtime_config) = load_engine_runtime_config()?;
    runtime_config.apply_process_env(&runtime_config_path)?;

    let project_browser_requested =
        env::args().any(|arg| arg == "--list-projects" || arg == "--project-browser");
    if project_browser_requested {
        return run_project_selection(&runtime_config_path);
    }

    let manifest_path = project_request_from_cli()
        .map(normalize_manifest_request)
        .or_else(game_manifest_request_from_process)
        .or_else(adjacent_game_manifest_from_exe);
    let Some(manifest_path) = manifest_path else {
        if runtime_config.runtime.startup_window {
            return run_project_selection(&runtime_config_path);
        }
        return Err("runtime package is incomplete: game.toml was not found; pass --project <game.toml> or enable the startup project browser".to_owned());
    };
    let manifest_path = normalize_manifest_request(manifest_path);
    let launch_request = project_launch_request_from_process()
        .or_else(|| Some(runtime_config.runtime.mode.launch_id().to_owned()));
    let project = load_project_from_request_with_launch(&manifest_path, launch_request.as_deref())?;
    dispatch_project_launch(&runtime_config_path, project)
}

fn dispatch_project_launch(
    runtime_config_path: &Path,
    project: newengine_project_runtime::ProjectRuntimeContext,
) -> Result<(), String> {
    // Project Browser is a startup chooser only. Editing-tools plugins remain in
    // normal runtime composition and are discovered as optional capabilities.
    exclude_project_browser_from_runtime();

    env::set_var(GAME_MANIFEST_ENV, &project.manifest_path);
    env::remove_var("NEWENGINE_PROJECT");
    env::set_var(
        newengine_project_api::PROJECT_LAUNCH_PRESET_ENV,
        &project.launch.preset_id,
    );

    // The resolved Game/Server/Test launch owns the effective runtime mode.
    // Optional editing tools do not alter runtime mode or world ownership.
    env::set_var(ENGINE_RUNTIME_MODE_ENV, project.launch.profile.id());
    env::set_var("NEWENGINE_LAUNCH_PROFILE", project.launch.profile.id());
    env::set_var(
        "NEWENGINE_HEADLESS",
        if matches!(
            project.launch.profile,
            newengine_project_api::RuntimeLaunchProfile::Server
                | newengine_project_api::RuntimeLaunchProfile::Test
        ) {
            "1"
        } else {
            "0"
        },
    );
    // Publish the complete resolved project contract before loading a runtime
    // composition DLL. The composition may read screen/profile policy during
    // plugin initialization, before its own runtime host reloads game.toml.
    newengine_project_runtime::apply_resolved_project_launch_env(&project.launch);
    newengine_project_runtime::apply_project_ui_env(&project.manifest);

    if let Some(runtime_profile) = project
        .launch
        .runtime_profile
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        println!(
            "North Star: launching project '{}' profile='{}' runtime_profile='{}' launch='{}' game_manifest='{}' runtime_config='{}'",
            project.manifest.name,
            project.launch.profile.id(),
            runtime_profile,
            project.launch.preset_id,
            project.manifest_path.display(),
            runtime_config_path.display(),
        );
        if env_bool("NEWENGINE_LAUNCHER_DRY_RUN", false) {
            return Ok(());
        }
        return launch_runtime_profile_via_plugins(runtime_profile, &project);
    }

    let launcher = project
        .manifest
        .launcher
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "project '{}' launch '{}' does not declare runtime_profile or launcher in {}",
                project.manifest.id,
                project.launch.preset_id,
                project.manifest_path.display()
            )
        })?;
    if launcher.eq_ignore_ascii_case("newengine-launcher")
        || launcher.eq_ignore_ascii_case("NewEngineLauncher")
        || launcher.eq_ignore_ascii_case("newengine")
        || launcher.eq_ignore_ascii_case("NewEngine")
    {
        return Err("game launcher cannot recursively target NewEngine".to_owned());
    }

    let target = resolve_launcher_target(launcher)?;
    println!(
        "North Star: launching project '{}' profile='{}' via '{}' game_manifest='{}'",
        project.manifest.name,
        project.launch.profile.id(),
        launcher,
        project.manifest_path.display()
    );

    if env_bool("NEWENGINE_LAUNCHER_DRY_RUN", false) {
        println!("North Star: dry-run target={target:?}");
        return Ok(());
    }

    match target {
        LauncherTarget::Executable(path) => {
            let mut command = Command::new(&path);
            command
                .arg("--game-manifest")
                .arg(&project.manifest_path)
                .arg("--launch")
                .arg(&project.launch.preset_id);
            command.env(GAME_MANIFEST_ENV, &project.manifest_path).env(
                newengine_project_api::PROJECT_LAUNCH_PRESET_ENV,
                &project.launch.preset_id,
            );
            if env_bool("NEWENGINE_LAUNCHER_WAIT_CHILD", false) {
                let status = command
                    .status()
                    .map_err(|error| format!("launch '{}' failed: {error}", path.display()))?;
                if !status.success() {
                    return Err(format!("game launcher exited with {status}"));
                }
            } else {
                command
                    .spawn()
                    .map_err(|error| format!("launch '{}' failed: {error}", path.display()))?;
            }
        }
        LauncherTarget::Cargo { workspace, package } => {
            let mut command = Command::new("cargo");
            command
                .current_dir(&workspace)
                .arg("run")
                .arg("-p")
                .arg(&package)
                .arg("--")
                .arg("--game-manifest")
                .arg(&project.manifest_path)
                .arg("--launch")
                .arg(&project.launch.preset_id)
                .env(GAME_MANIFEST_ENV, &project.manifest_path)
                .env(
                    newengine_project_api::PROJECT_LAUNCH_PRESET_ENV,
                    &project.launch.preset_id,
                );
            if env_bool("NEWENGINE_LAUNCHER_WAIT_CHILD", false) {
                let status = command
                    .status()
                    .map_err(|error| format!("cargo launch package '{package}' failed: {error}"))?;
                if !status.success() {
                    return Err(format!("cargo game launcher exited with {status}"));
                }
            } else {
                command
                    .spawn()
                    .map_err(|error| format!("cargo launch package '{package}' failed: {error}"))?;
            }
        }
    }
    Ok(())
}

fn project_request_from_cli() -> Option<PathBuf> {
    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--project" {
            return args.next().map(PathBuf::from);
        }
        if let Some(text) = arg.to_str() {
            if let Some(value) = text.strip_prefix("--project=") {
                if !value.trim().is_empty() {
                    return Some(PathBuf::from(value));
                }
            }
        }
    }
    None
}

fn run_project_selection(runtime_config_path: &Path) -> Result<(), String> {
    env::set_var("NEWENGINE_HEADLESS", "0");

    let (manifest_path, launch_request) = if let Some(request) = project_request_from_cli() {
        // Explicit CLI project selection is the only chooser bypass. Inherited
        // NEWENGINE_PROJECT is deliberately ignored in project-selection mode
        (
            normalize_manifest_request(request),
            project_launch_request_from_process(),
        )
    } else {
        let root = default_projects_root().ok_or_else(|| {
            "project browser cannot discover Projects root; set NEWENGINE_PROJECTS_ROOT or pass --project <game.toml>"
                .to_owned()
        })?;
        if env::args().any(|arg| arg == "--list-projects") {
            for project in newengine_project_runtime::discover_projects(&root) {
                println!(
                    "{}\t{}\t{}",
                    project.id,
                    project.name,
                    project.manifest_path.display()
                );
            }
            return Ok(());
        }
        let selection = present_project_browser_via_plugin(&root)?;
        if selection.cancelled {
            return Ok(());
        }
        (
            selection
                .manifest_path
                .ok_or_else(|| "project selection returned no game.toml".to_owned())?,
            selection.launch_id,
        )
    };

    let project = load_project_from_request_with_launch(&manifest_path, launch_request.as_deref())?;
    dispatch_project_launch(runtime_config_path, project)
}

fn present_project_browser_via_plugin(root: &Path) -> Result<ProjectBrowserSelection, String> {
    init_host_context();
    let host = default_host_api();
    let mut plugins = PluginManager::new();
    let plugin_id = env::var(PROJECT_BROWSER_PLUGIN_ID_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_PROJECT_BROWSER_PLUGIN_ID.to_owned());

    if !has_service(PROJECT_BROWSER_SERVICE_ID) {
        plugins
            .load_plugin_id_default_with_origin(
                &plugin_id,
                host,
                PluginLoadOrigin::FirstPartyPlugin,
            )
            .map_err(|error| format!("load Project Browser plugin '{plugin_id}': {error}"))?;
    }
    if !has_service(PROJECT_BROWSER_SERVICE_ID) {
        plugins.shutdown();
        return Err(format!(
            "Project Browser service '{}' unavailable after loading '{}'",
            PROJECT_BROWSER_SERVICE_ID, plugin_id,
        ));
    }

    let startup_config_path =
        env::var(newengine_runtime_host::runtime_config::ENGINE_STARTUP_CONFIG_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "config.json".to_owned());
    let startup_paths = newengine_core::ConfigPaths::from_startup_str(&startup_config_path);
    let startup_settings =
        newengine_core::StartupLoader::load_launch_settings_preview(&startup_paths)
            .unwrap_or_default();
    let payload = serde_json::to_vec(&serde_json::json!({
        "root": root.to_string_lossy(),
        "settings": {
            "preset": startup_settings.graphics.preset.as_str(),
            "lod_quality": startup_settings.graphics.lod_quality.as_str(),
            "lod_distance_scale": startup_settings.graphics.lod_distance_scale,
            "shadows_enabled": startup_settings.graphics.shadows_enabled,
            "shadow_quality": startup_settings.graphics.shadow_quality.as_str(),
            "shadow_cascade_count": startup_settings.graphics.shadow_cascade_count,
            "shadow_map_resolution": startup_settings.graphics.shadow_map_resolution,
            "shadow_advanced_override": startup_settings.graphics.shadow_advanced_override,
            "shadow_filter": startup_settings.graphics.shadow_filter.as_str(),
            "shadow_max_distance": startup_settings.graphics.shadow_max_distance,
            "shadow_softness": startup_settings.graphics.shadow_softness,
            "shadow_bias": startup_settings.graphics.shadow_bias,
            "shadow_normal_bias": startup_settings.graphics.shadow_normal_bias,
            "shadow_contact_strength": startup_settings.graphics.shadow_contact_strength,
            "shadow_pcss_light_radius_degrees": startup_settings.graphics.shadow_pcss_light_radius_degrees,
            "shadow_pcss_blocker_radius_texels": startup_settings.graphics.shadow_pcss_blocker_radius_texels,
            "shadow_pcss_max_filter_radius_texels": startup_settings.graphics.shadow_pcss_max_filter_radius_texels,
            "shadow_pcss_blocker_samples": startup_settings.graphics.shadow_pcss_blocker_samples,
            "shadow_pcss_filter_samples": startup_settings.graphics.shadow_pcss_filter_samples,
            "shadow_pcss_min_filter_radius_texels": startup_settings.graphics.shadow_pcss_min_filter_radius_texels,
            "shadow_pcss_stable_kernel_texels": startup_settings.graphics.shadow_pcss_stable_kernel_texels,
        }
    }))
    .map_err(|error| format!("encode project-browser request: {error}"))?;
    let response = call_service_v1(
        PROJECT_BROWSER_SERVICE_ID.into(),
        PROJECT_BROWSER_PRESENT_METHOD_V1.into(),
        Blob::from(payload),
    )
    .into_result()
    .map_err(|error| format!("Project Browser failed: {error}"))?;
    let value: serde_json::Value = serde_json::from_slice(response.as_slice())
        .map_err(|error| format!("decode project-browser response: {error}"))?;
    if !value
        .get("cancelled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        let mut selected_settings = startup_settings.clone();
        apply_project_browser_graphics_settings(&value, &mut selected_settings);
        newengine_core::StartupLoader::persist_launch_settings(&startup_paths, &selected_settings)
            .map_err(|error| format!("persist Project Browser launch settings: {error}"))?;
        // Settings were already confirmed in the project-selection window. Do not
        // present a second PreStart settings window for this browser-selected launch.
        env::set_var("NEWENGINE_STARTUP_WINDOW_SKIP", "1");
    }
    let selection = ProjectBrowserSelection {
        manifest_path: value
            .get("manifest_path")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from),
        launch_id: value
            .get("launch_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        cancelled: value
            .get("cancelled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    };
    plugins.shutdown();
    Ok(selection)
}

fn apply_project_browser_graphics_settings(
    value: &serde_json::Value,
    launch_settings: &mut newengine_core::StartupLaunchSettings,
) {
    let Some(settings) = value.get("settings") else {
        return;
    };
    let graphics = &mut launch_settings.graphics;
    let mut explicit_preset = false;
    if let Some(v) = settings.get("preset").and_then(serde_json::Value::as_str) {
        graphics.preset = match v.trim().to_ascii_lowercase().as_str() {
            "low" => newengine_core::GraphicsPreset::Low,
            "balanced" | "medium" => newengine_core::GraphicsPreset::Balanced,
            "high" => newengine_core::GraphicsPreset::High,
            "ultra" => newengine_core::GraphicsPreset::Ultra,
            _ => newengine_core::GraphicsPreset::Custom,
        };
        explicit_preset = true;
    }
    if let Some(v) = settings
        .get("lod_quality")
        .and_then(serde_json::Value::as_str)
    {
        graphics.lod_quality = match v.trim().to_ascii_lowercase().as_str() {
            "low" => newengine_core::LodQuality::Low,
            "medium" => newengine_core::LodQuality::Medium,
            "high" => newengine_core::LodQuality::High,
            "ultra" => newengine_core::LodQuality::Ultra,
            "cinematic" => newengine_core::LodQuality::Cinematic,
            _ => newengine_core::LodQuality::Custom,
        };
    }
    if let Some(v) = settings
        .get("lod_distance_scale")
        .and_then(serde_json::Value::as_f64)
    {
        graphics.lod_distance_scale = v as f32;
    }
    if let Some(v) = settings
        .get("shadows_enabled")
        .and_then(serde_json::Value::as_bool)
    {
        graphics.shadows_enabled = v;
    }
    if let Some(v) = settings
        .get("shadow_quality")
        .and_then(serde_json::Value::as_str)
    {
        graphics.shadow_quality = match v.trim().to_ascii_lowercase().as_str() {
            "performance" => newengine_core::ShadowQuality::Performance,
            "quality" => newengine_core::ShadowQuality::Quality,
            "cinematic" => newengine_core::ShadowQuality::Cinematic,
            "off" => newengine_core::ShadowQuality::Off,
            _ => newengine_core::ShadowQuality::Balanced,
        };
    }
    if let Some(v) = settings
        .get("shadow_cascade_count")
        .and_then(serde_json::Value::as_u64)
    {
        graphics.shadow_cascade_count = v as u32;
    }
    if let Some(v) = settings
        .get("shadow_map_resolution")
        .and_then(serde_json::Value::as_u64)
    {
        graphics.shadow_map_resolution = v as u32;
    }
    if let Some(v) = settings
        .get("shadow_advanced_override")
        .and_then(serde_json::Value::as_bool)
    {
        graphics.shadow_advanced_override = v;
    }
    if let Some(v) = settings
        .get("shadow_filter")
        .and_then(serde_json::Value::as_str)
    {
        graphics.shadow_filter = match v.trim().to_ascii_lowercase().as_str() {
            "hard" => newengine_core::ShadowFilterMode::Hard,
            "pcf" => newengine_core::ShadowFilterMode::Pcf,
            _ => newengine_core::ShadowFilterMode::Pcss,
        };
    }
    macro_rules! f32_setting {
        ($name:literal, $field:ident) => {
            if let Some(v) = settings.get($name).and_then(serde_json::Value::as_f64) {
                graphics.$field = v as f32;
            }
        };
    }
    macro_rules! u32_setting {
        ($name:literal, $field:ident) => {
            if let Some(v) = settings.get($name).and_then(serde_json::Value::as_u64) {
                graphics.$field = v as u32;
            }
        };
    }
    f32_setting!("shadow_max_distance", shadow_max_distance);
    f32_setting!("shadow_softness", shadow_softness);
    f32_setting!("shadow_bias", shadow_bias);
    f32_setting!("shadow_normal_bias", shadow_normal_bias);
    f32_setting!("shadow_contact_strength", shadow_contact_strength);
    f32_setting!(
        "shadow_pcss_light_radius_degrees",
        shadow_pcss_light_radius_degrees
    );
    f32_setting!(
        "shadow_pcss_blocker_radius_texels",
        shadow_pcss_blocker_radius_texels
    );
    f32_setting!(
        "shadow_pcss_max_filter_radius_texels",
        shadow_pcss_max_filter_radius_texels
    );
    u32_setting!("shadow_pcss_blocker_samples", shadow_pcss_blocker_samples);
    u32_setting!("shadow_pcss_filter_samples", shadow_pcss_filter_samples);
    f32_setting!(
        "shadow_pcss_min_filter_radius_texels",
        shadow_pcss_min_filter_radius_texels
    );
    f32_setting!(
        "shadow_pcss_stable_kernel_texels",
        shadow_pcss_stable_kernel_texels
    );
    if !explicit_preset {
        graphics.mark_custom();
    }
    launch_settings.normalize();
    launch_settings.publish_environment_snapshot();
}

fn launch_runtime_profile_via_plugins(
    runtime_profile: &str,
    project: &newengine_project_runtime::ProjectRuntimeContext,
) -> Result<(), String> {
    init_host_context();
    let host = default_host_api();
    let mut plugins = PluginManager::new();

    // Game-owned compositions win over engine defaults. This is what allows
    // fpsGame.dll, topDownGame.dll and thirdPersonGame.dll to select the same
    // reusable runtime profile without recompiling NewEngine.exe.
    load_game_runtime_plugins(&mut plugins, project, host.clone())?;

    let service_id = runtime_profile_service_id_for_game(
        runtime_profile,
        project.manifest.game_module.as_deref(),
    );

    if !has_service(&service_id) {
        // Runtime composition identity is intentionally aligned with the selected
        // game_module (or runtime_profile for a generic composition). Load only
        // that plugin from pluginsRuntime; renderer/physics/UI remain owned by the
        // runtime's own PluginManager and are not initialized by the launcher.
        let composition_plugin_id = project
            .manifest
            .game_module
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(runtime_profile);
        plugins
            .load_plugin_id_default_with_origin(
                composition_plugin_id,
                host.clone(),
                PluginLoadOrigin::FirstPartyPlugin,
            )
            .map_err(|error| {
                format!(
                    "load runtime composition plugin '{}' from pluginsRuntime: {error}",
                    composition_plugin_id
                )
            })?;
    }

    if !has_service(&service_id) {
        let available = plugins
            .snapshot()
            .into_iter()
            .flat_map(|plugin| {
                plugin
                    .capabilities
                    .into_iter()
                    .map(move |capability| format!("{}:{}", plugin.id, capability.id))
            })
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "runtime composition service '{}' is unavailable for profile='{}' game_module='{}'; loaded capabilities=[{}]",
            service_id,
            runtime_profile,
            project.manifest.game_module.as_deref().unwrap_or("<none>"),
            available,
        ));
    }

    let composition_plugin_id = project
        .manifest
        .game_module
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(runtime_profile);
    append_env_list_unique("NEWENGINE_PLUGIN_EXCLUDE_IDS", composition_plugin_id);

    let payload = serde_json::to_vec(&serde_json::json!({
        // Project launches selected by the startup browser are dev launches.
        // Keep `manifest_path` as the canonical project-manifest field expected
        // by runtime-profile services, while `game_manifest_path` remains an
        // explicit compatibility alias for game-module services. Neither field
        // is `runtime_manifest_path`: that name is reserved for packaged runtime
        // configuration and must never point at game.toml.
        "manifest_path": project.manifest_path.to_string_lossy(),
        "game_manifest_path": project.manifest_path.to_string_lossy(),
        "launch_id": project.launch.preset_id,
        "runtime_profile": runtime_profile,
        "game_module": project.manifest.game_module,
    }))
    .map_err(|error| format!("encode runtime-profile launch request: {error}"))?;

    let result = call_service_v1(
        service_id.clone().into(),
        RUNTIME_PROFILE_LAUNCH_METHOD_V1.into(),
        Blob::from(payload),
    )
    .into_result()
    .map_err(|error| {
        format!(
            "runtime profile '{}' launch failed: {error}",
            runtime_profile
        )
    });

    plugins.shutdown();
    result.map(|_| ())
}

fn load_game_runtime_plugins(
    plugins: &mut PluginManager,
    project: &newengine_project_runtime::ProjectRuntimeContext,
    host: newengine_plugin_api::HostApiV1,
) -> Result<(), String> {
    let conventional = project.project_root.join("plugins");
    if conventional.is_dir() {
        plugins
            .load_from_dir_with_policy_and_origin(
                &conventional,
                host.clone(),
                false,
                PluginLoadOrigin::GamePlugin,
            )
            .map_err(|error| {
                format!(
                    "load game plugin directory '{}': {error}",
                    conventional.display()
                )
            })?;
    }

    for plugin in &project.manifest.plugins {
        let Some(path) = plugin.path.as_ref() else {
            continue;
        };
        let path = if path.is_absolute() {
            path.clone()
        } else {
            project.project_root.join(path)
        };

        let result = if path.is_dir() {
            plugins.load_from_dir_with_policy_and_origin(
                &path,
                host.clone(),
                plugin.required,
                PluginLoadOrigin::GamePlugin,
            )
        } else {
            plugins.load_path_with_origin(&path, host.clone(), PluginLoadOrigin::GamePlugin)
        };

        if let Err(error) = result {
            if plugin.required {
                return Err(format!(
                    "required game plugin '{}' failed from '{}': {error}",
                    plugin.id,
                    path.display(),
                ));
            }
            eprintln!(
                "NewEngine: optional game plugin '{}' skipped from '{}': {error}",
                plugin.id,
                path.display(),
            );
        }
    }
    Ok(())
}

fn normalize_manifest_request(mut path: PathBuf) -> PathBuf {
    if path.is_dir() {
        path.push(PROJECT_MANIFEST_FILE);
    }
    path
}

#[derive(Debug)]
enum LauncherTarget {
    Executable(PathBuf),
    Cargo { workspace: PathBuf, package: String },
}

fn resolve_launcher_target(launcher: &str) -> Result<LauncherTarget, String> {
    if let Some(bin_dir) = env::var_os("NEWENGINE_LAUNCHER_BIN_DIR") {
        if let Some(path) = executable_candidate(&PathBuf::from(bin_dir), launcher) {
            return Ok(LauncherTarget::Executable(path));
        }
    }

    if let Ok(current) = env::current_exe() {
        if let Some(dir) = current.parent() {
            for candidate_dir in [dir.to_path_buf(), dir.join("bin")] {
                if let Some(path) = executable_candidate(&candidate_dir, launcher) {
                    return Ok(LauncherTarget::Executable(path));
                }
            }
        }
    }

    if let Some(workspace) = find_cargo_workspace() {
        return Ok(LauncherTarget::Cargo {
            workspace,
            package: launcher.to_owned(),
        });
    }

    Err(format!(
        "cannot resolve launcher '{launcher}'; install its executable next to NewEngine, set NEWENGINE_LAUNCHER_BIN_DIR, or run from the Cargo workspace"
    ))
}

fn executable_candidate(dir: &Path, launcher: &str) -> Option<PathBuf> {
    let filename = if cfg!(windows) {
        format!("{launcher}.exe")
    } else {
        launcher.to_owned()
    };
    let direct = dir.join(&filename);
    if direct.is_file() {
        return Some(direct);
    }
    let normalized = launcher.replace('_', "-");
    let alternate = if cfg!(windows) {
        dir.join(format!("{normalized}.exe"))
    } else {
        dir.join(normalized)
    };
    alternate.is_file().then_some(alternate)
}

fn find_cargo_workspace() -> Option<PathBuf> {
    let mut seeds = Vec::new();
    if let Ok(cwd) = env::current_dir() {
        seeds.push(cwd);
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            seeds.push(parent.to_path_buf());
        }
    }
    for seed in seeds {
        for ancestor in seed.ancestors().take(8) {
            if ancestor.join("Cargo.toml").is_file()
                && ancestor.join("crates").is_dir()
                && ancestor.join("apps").is_dir()
            {
                return Some(ancestor.to_path_buf());
            }
        }
    }
    None
}

fn exclude_project_browser_from_runtime() {
    let browser_plugin_id = env::var(PROJECT_BROWSER_PLUGIN_ID_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_PROJECT_BROWSER_PLUGIN_ID.to_owned());
    append_env_list_unique("NEWENGINE_PLUGIN_EXCLUDE_IDS", &browser_plugin_id);
}

fn append_env_list_unique(name: &str, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    let mut entries = env::var(name)
        .ok()
        .into_iter()
        .flat_map(|current| {
            current
                .split(|ch: char| ch == ',' || ch == ';' || ch.is_ascii_whitespace())
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    if !entries.iter().any(|entry| entry == value) {
        entries.push(value.to_owned());
    }
    env::set_var(name, entries.join(","));
}

fn env_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}
