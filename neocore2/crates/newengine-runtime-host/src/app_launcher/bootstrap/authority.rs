fn install_root_dir_authority(
    host: &newengine_plugin_host::HostContextHandle,
) -> EngineResult<PathBuf> {
    if let Some(explicit) = host
        .environment_var_os(ROOT_DIR_ENV)
        .filter(|value| !value.as_os_str().is_empty())
    {
        let path = PathBuf::from(explicit);
        if !path.is_absolute() {
            return Err(EngineError::Other(format!(
                "{ROOT_DIR_ENV} must be absolute, got '{}'",
                path.display()
            )));
        }
        let normalized = newengine_core::storage_root::normalize_path(path, None);
        host.set_environment_var(ROOT_DIR_ENV, normalized.as_os_str().to_os_string());
        return Ok(normalized);
    }

    let mut probes = Vec::<PathBuf>::new();
    if let Ok(cwd) = std::env::current_dir() {
        probes.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            probes.push(parent.to_path_buf());
        }
    }
    for probe in probes {
        for ancestor in probe.ancestors() {
            if ancestor.join("NewEngine").is_dir()
                && ancestor.join("pluginsRuntime").is_dir()
                && ancestor.join("Projects").is_dir()
            {
                let root =
                    newengine_core::storage_root::normalize_path(ancestor.to_path_buf(), None);
                host.set_environment_var(ROOT_DIR_ENV, root.as_os_str().to_os_string());
                return Ok(root);
            }
        }
    }

    Err(EngineError::Other(format!(
        "{ROOT_DIR_ENV} is not set and NorthStar root auto-detection failed"
    )))
}

fn authority_root(
    host: &newengine_plugin_host::HostContextHandle,
    key: &str,
) -> EngineResult<PathBuf> {
    let raw = host
        .environment_var_os(key)
        .filter(|value| !value.as_os_str().is_empty())
        .ok_or_else(|| {
            EngineError::Other(format!("path authority variable '{key}' is not available"))
        })?;
    let root = PathBuf::from(raw);
    if !root.is_absolute() {
        return Err(EngineError::Other(format!(
            "path authority variable '{key}' must be absolute, got '{}'",
            root.display()
        )));
    }
    Ok(newengine_core::storage_root::normalize_path(root, None))
}

fn authority_token_suffix<'a>(raw: &'a str, token: &str) -> Option<&'a str> {
    if raw == token {
        return Some("");
    }
    raw.strip_prefix(token)
        .and_then(|rest| rest.strip_prefix('/').or_else(|| rest.strip_prefix('\\')))
}

fn expand_authority_path(
    host: &newengine_plugin_host::HostContextHandle,
    raw: &str,
) -> EngineResult<Option<PathBuf>> {
    for token in [ROOT_DIR_ENV, PROJECT_DIR_ENV] {
        let Some(suffix) = authority_token_suffix(raw.trim(), token) else {
            continue;
        };
        let root = authority_root(host, token)?;
        let suffix_path = Path::new(suffix);
        if suffix_path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(EngineError::Other(format!(
                "path authority value must not contain parent traversal: '{raw}'"
            )));
        }
        return Ok(Some(newengine_core::storage_root::normalize_path(
            root.join(suffix_path),
            None,
        )));
    }
    Ok(None)
}

fn expand_authority_json(
    host: &newengine_plugin_host::HostContextHandle,
    value: &mut serde_json::Value,
) -> EngineResult<()> {
    match value {
        serde_json::Value::String(raw) => {
            if let Some(path) = expand_authority_path(host, raw)? {
                *raw = path.to_string_lossy().into_owned();
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                expand_authority_json(host, value)?;
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values_mut() {
                expand_authority_json(host, value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn expand_startup_path_authorities(
    host: &newengine_plugin_host::HostContextHandle,
    startup: &mut StartupConfig,
) -> EngineResult<()> {
    for path in [
        &mut startup.modules_dir,
        &mut startup.cache_files,
        &mut startup.config,
    ] {
        let raw = path.to_string_lossy().into_owned();
        if let Some(expanded) = expand_authority_path(host, &raw)? {
            *path = expanded;
        }
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(EngineError::Other(format!(
                "startup filesystem path must not contain parent traversal: '{}'",
                path.display()
            )));
        }
    }
    for value in startup.plugins.values_mut() {
        expand_authority_json(host, value)?;
    }
    Ok(())
}

fn publish_expanded_storage_roots(
    host: &newengine_plugin_host::HostContextHandle,
    startup: &StartupConfig,
) {
    let cache = startup.resolved_cache_files_dir();
    host.set_environment_var(
        newengine_core::CACHE_FILES_ENV,
        cache.as_os_str().to_os_string(),
    );
    host.set_environment_var(
        newengine_core::CACHE_FILES_ALIAS_ENV,
        cache.as_os_str().to_os_string(),
    );
    host.set_environment_var(newengine_core::CACHE_FILES_READY_ENV, "1");

    let config = startup.resolved_config_dir();
    host.set_environment_var(
        newengine_core::CONFIG_ENV,
        config.as_os_str().to_os_string(),
    );
    host.set_environment_var(
        newengine_core::CONFIG_ALIAS_ENV,
        config.as_os_str().to_os_string(),
    );
    host.set_environment_var(newengine_core::CONFIG_READY_ENV, "1");
}

fn resolve_startup_config_path(
    host: &newengine_plugin_host::HostContextHandle,
    raw: &str,
) -> EngineResult<PathBuf> {
    if let Some(path) = expand_authority_path(host, raw)? {
        return Ok(path);
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        return Ok(path);
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(EngineError::Other(format!(
            "startup config path must use {ROOT_DIR_ENV} or {PROJECT_DIR_ENV}, not parent traversal: '{raw}'"
        )));
    }
    let root = authority_root(host, ROOT_DIR_ENV)?;
    Ok(root.join("NewEngine").join("neocore2").join(path))
}
