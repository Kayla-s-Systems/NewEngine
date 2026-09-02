fn set_default_environment(
    host: &newengine_plugin_host::HostContextHandle,
    key: &str,
    value: impl Into<std::ffi::OsString>,
) {
    if host.environment_var_os(key).is_none() {
        host.set_environment_var(key, value);
    }
}

fn apply_project_environment(
    host: &newengine_plugin_host::HostContextHandle,
    context: &ProjectRuntimeContext,
    editor_owned: bool,
) {
    host.set_environment_var(
        PROJECT_DIR_ENV,
        context.project_root.as_os_str().to_os_string(),
    );
    if editor_owned {
        host.set_environment_var(
            PROJECT_ROOT_ENV,
            context.project_root.as_os_str().to_os_string(),
        );
        host.set_environment_var(
            PROJECT_MANIFEST_ENV,
            context.manifest_path.as_os_str().to_os_string(),
        );
    } else {
        host.set_environment_var(
            GAME_ROOT_ENV,
            context.project_root.as_os_str().to_os_string(),
        );
        host.set_environment_var(
            newengine_project_api::GAME_MANIFEST_ENV,
            context.manifest_path.as_os_str().to_os_string(),
        );
    }

    if let Some(startup_scene) = context
        .launch
        .startup_scene
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        host.set_environment_var(PROJECT_STARTUP_SCENE_ENV, startup_scene);
    } else {
        host.remove_environment_var(PROJECT_STARTUP_SCENE_ENV);
    }

    set_default_environment(
        host,
        "NEWENGINE_LAUNCH_PROFILE",
        context.launch.profile.id(),
    );
    match context.launch.profile {
        RuntimeLaunchProfile::Game => {
            set_default_environment(host, "NEWENGINE_HEADLESS", "0");
            set_default_environment(
                host,
                newengine_project_runtime::UI_SCREEN_PROFILE_ENV,
                "game",
            );
        }
        RuntimeLaunchProfile::Server => {
            set_default_environment(host, "NEWENGINE_HEADLESS", "1");
            set_default_environment(
                host,
                newengine_project_runtime::UI_SCREEN_PROFILE_ENV,
                "headless",
            );
            set_default_environment(host, "NEWENGINE_PLUGIN_TARGET", "runtime");
        }
        RuntimeLaunchProfile::Test => {
            set_default_environment(host, "NEWENGINE_HEADLESS", "1");
            set_default_environment(
                host,
                newengine_project_runtime::UI_SCREEN_PROFILE_ENV,
                "headless",
            );
            set_default_environment(host, "NEWENGINE_HEADLESS_FRAMES", "1");
            set_default_environment(host, "NEWENGINE_PLUGIN_TARGET", "runtime");
        }
    }

    if let Some(state) = context
        .launch
        .startup_presentation_state
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        set_default_environment(
            host,
            newengine_project_runtime::UI_PRESENTATION_INITIAL_STATE_ENV,
            state,
        );
    }

    let ui = &context.manifest.ui;
    if let Some(value) = ui
        .screen_profile
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        host.set_environment_var(newengine_project_runtime::UI_SCREEN_PROFILE_ENV, value);
    }
    if let Some(value) = ui
        .root_surface
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        host.set_environment_var(newengine_project_runtime::UI_ROOT_SURFACE_ENV, value);
    }
    if let Some(value) = ui
        .document
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        host.set_environment_var(newengine_project_runtime::UI_DOCUMENT_ENV, value);
    }
    let requested_initial_state = host
        .environment_var(newengine_project_runtime::UI_PRESENTATION_INITIAL_STATE_ENV)
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if let Some(flow) = newengine_project_runtime::effective_project_ui_presentation_flow(
        &context.manifest,
        requested_initial_state.as_deref(),
    ) {
        host.remove_environment_var(newengine_project_runtime::UI_PRESENTATION_INITIAL_STATE_ENV);
        if let Ok(encoded) = serde_json::to_string(&flow) {
            host.set_environment_var(newengine_project_runtime::UI_PRESENTATION_FLOW_ENV, encoded);
        }
    } else {
        host.remove_environment_var(newengine_project_runtime::UI_PRESENTATION_FLOW_ENV);
    }
    if let Some(value) = ui.publish_editor_shell {
        host.set_environment_var(
            newengine_project_runtime::UI_PUBLISH_EDITOR_SHELL_ENV,
            if value { "true" } else { "false" },
        );
    }
}

fn configure_game_plugin_roots(
    engine: &mut Engine<()>,
    project: &ProjectRuntimeContext,
) -> EngineResult<()> {
    let project_id = project.manifest.id.trim();
    let conventional_plugins = project.paths().conventional_plugins_dir();
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
        let path = project.resolve_authored_path(path);
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
