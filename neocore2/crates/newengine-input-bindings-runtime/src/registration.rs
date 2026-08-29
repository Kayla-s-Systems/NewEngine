use super::*;

pub fn register_input_bindings_gateway_best_effort(default_profile: InputBindingsProfile) {
    let state_ref = install_or_update_default_profile(default_profile);
    if newengine_plugin_host::has_service(ENGINE_INPUT_BINDINGS_SERVICE_ID) {
        let state = state_ref.lock();
        newengine_ulog_api::ulog::info!(
            "input bindings gateway: service already registered id='{}' profile='{}' actions={} bindings={} axes={} config='{}'",
            ENGINE_INPUT_BINDINGS_SERVICE_ID,
            state.profile.id,
            state.profile.actions.len(),
            state.profile.bindings.len(),
            state.profile.gamepad_axes.len(),
            state.profile_path.display(),
        );
        return;
    }

    let service = input_bindings_gateway_service(state_ref);
    match register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: ENGINE_INPUT_BINDINGS_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::InputBindings,
        provider_service: ENGINE_INPUT_BINDINGS_SERVICE_ID,
        provider_route: "engine.input.compass.bindings",
        capability: INPUT_BINDINGS_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: INPUT_BINDINGS_GATEWAY_OWNER,
        service,
    }) {
        Ok(()) => {
            let path = INPUT_BINDINGS_GATEWAY
                .get()
                .map(|s| s.lock().profile_path.clone())
                .unwrap_or_else(profile_path);
            newengine_ulog_api::ulog::info!(
                "input bindings gateway: engine-runtime route registered id='{}' capability='{}' config='{}'",
                ENGINE_INPUT_BINDINGS_SERVICE_ID,
                INPUT_BINDINGS_BACKEND_CAPABILITY_ID,
                path.display()
            );
        }
        Err(e) => newengine_ulog_api::ulog::warn!(
            "input bindings gateway: registration skipped id='{}' err='{}'",
            ENGINE_INPUT_BINDINGS_SERVICE_ID,
            e
        ),
    }
}
