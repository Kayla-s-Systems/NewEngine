fn project_scripting_backend_tag(runtime: &str) -> Option<&'static str> {
    match runtime.trim().to_ascii_lowercase().as_str() {
        "typescript" | "ts" | "typescript-v8" | "v8" => Some("backend.typescript.v8"),
        "lua" | "lua54" | "lua-5.4" => Some("backend.lua54"),
        _ => None,
    }
}

fn install_project_scripting_provider_policy(
    host: &newengine_plugin_host::HostContextHandle,
    project: Option<&ProjectRuntimeContext>,
) -> EngineResult<()> {
    let Some(runtime) = project.and_then(|project| project.scripts.runtime()) else {
        return Ok(());
    };
    let Some(tag) = project_scripting_backend_tag(runtime) else {
        newengine_ulog_api::ulog::warn!(
            "project scripting: runtime hint '{}' has no provider-tag mapping; composition remains provider-selected",
            runtime
        );
        return Ok(());
    };
    newengine_plugin_host::with_host_context(host, || {
        newengine_plugin_host::install_engine_gateway_selection_policy(
            newengine_plugin_host::EngineGatewaySelectionPolicy::new(
                newengine_scripting_api::ENGINE_SCRIPTING_SERVICE_ID,
                "newengine-runtime-host.project-scripting",
            )
            .prefer_tags([tag])
            .preference_bonus(10_000),
        )
    })
    .map_err(|error| {
        EngineError::Other(format!(
            "project scripting provider policy install failed runtime='{runtime}' tag='{tag}': {error}"
        ))
    })?;
    self_contained_scripting_policy_log(runtime, tag);
    Ok(())
}

#[inline]
fn self_contained_scripting_policy_log(runtime: &str, tag: &str) {
    newengine_ulog_api::ulog::info!(
        "project scripting: runtime='{}' prefers provider tag='{}' gateway='{}'",
        runtime,
        tag,
        newengine_scripting_api::ENGINE_SCRIPTING_SERVICE_ID
    );
}
