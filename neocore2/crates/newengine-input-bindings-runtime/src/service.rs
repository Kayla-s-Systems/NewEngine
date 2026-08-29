use super::*;

pub(crate) const INPUT_BINDINGS_GATEWAY_OWNER: &str =
    "newengine-input-bindings-runtime.bindings-gateway";

pub(crate) fn input_bindings_gateway_service(
    state: Arc<Mutex<InputBindingsGatewayState>>,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let info = InputBindingsServiceInfo::default();
    let description = engine_gateway_provider_service_description(
        ENGINE_INPUT_BINDINGS_SERVICE_ID,
        INPUT_BINDINGS_GATEWAY_OWNER,
        INPUT_BINDINGS_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .version(2)
    .protocol(info.protocol.clone())
    .features(info.features.clone())
    .gateway("engine.input.compass.bindings profile service");

    JsonServiceRouter::with_shared_state(ENGINE_INPUT_BINDINGS_SERVICE_ID, state)
        .describe_json(&description)
        .info(InputBindingsServiceInfo::default)
        .get_json(INPUT_BINDINGS_METHOD_PROFILE_JSON_V1, |state| {
            state.profile.clone()
        })
        .get_json(INPUT_BINDINGS_METHOD_ACTION_CATALOG_JSON_V1, |state| {
            state.profile.actions.clone()
        })
        .get_json(INPUT_BINDINGS_METHOD_KEY_CATALOG_JSON_V1, |state| {
            state.profile.keys.clone()
        })
        .blob(INPUT_BINDINGS_METHOD_INVOKE, save_profile_payload)
        .blob(
            INPUT_BINDINGS_METHOD_SAVE_PROFILE_JSON_V1,
            save_profile_payload,
        )
        .post_json_result::<InputKeyRegistration, InputBindingsProfile, _>(
            INPUT_BINDINGS_METHOD_REGISTER_KEY_JSON_V1,
            |state, registration| {
                mutate_profile_state_result(state, |profile| profile.register_key(registration))
            },
        )
        .post_json_result::<InputActionDefinition, InputBindingsProfile, _>(
            INPUT_BINDINGS_METHOD_REGISTER_ACTION_JSON_V1,
            |state, registration| {
                mutate_profile_state_result(state, |profile| profile.register_action(registration))
            },
        )
        .post_json_result::<InputBindingRegistration, InputBindingsProfile, _>(
            INPUT_BINDINGS_METHOD_REGISTER_BINDING_JSON_V1,
            |state, registration| {
                mutate_profile_state_result(state, |profile| profile.register_binding(registration))
            },
        )
        .post_json_result::<InputActionListenerRegistration, InputBindingsProfile, _>(
            INPUT_BINDINGS_METHOD_REGISTER_LISTENER_JSON_V1,
            |state, registration| {
                mutate_profile_state_result(state, |profile| {
                    profile.register_listener(registration)
                })
            },
        )
        .post_json_result::<InputBindingsManifest, InputBindingsProfile, _>(
            INPUT_BINDINGS_METHOD_REGISTER_MANIFEST_JSON_V1,
            |state, manifest| {
                mutate_profile_state_result(state, |profile| manifest.apply_to(profile))
            },
        )
        .get_json_result(INPUT_BINDINGS_METHOD_RESET_PROFILE_JSON_V1, |state| {
            let profile = state.default_profile.clone().canonicalized();
            state.profile = profile.clone();
            let path = state.profile_path.clone();
            save_profile_to_config(&path, &profile).map_err(|e| {
                format!(
                    "engine.input.bindings: reset save failed path='{}' err='{}'",
                    path.display(),
                    e
                )
            })?;
            Ok(profile)
        })
        .blob(INPUT_BINDINGS_METHOD_SHUTDOWN_V1, |_state, _payload| {
            ok_empty_blob()
        })
        .into_service_v1()
}

fn save_profile_payload(
    state: &mut InputBindingsGatewayState,
    payload: Blob,
) -> RResult<Blob, RString> {
    if payload.is_empty() {
        return ok_json(&state.profile);
    }
    let profile = match decode_json_payload::<InputBindingsProfile>(
        ENGINE_INPUT_BINDINGS_SERVICE_ID,
        INPUT_BINDINGS_METHOD_SAVE_PROFILE_JSON_V1,
        &payload,
    ) {
        Ok(profile) => profile.canonicalized_with_defaults(&state.default_profile),
        Err(e) => return RResult::RErr(e),
    };
    state.profile = profile.clone();
    let path = state.profile_path.clone();
    if let Err(e) = save_profile_to_config(&path, &profile) {
        return RResult::RErr(RString::from(format!(
            "engine.input.bindings: save failed path='{}' err='{}'",
            path.display(),
            e
        )));
    }
    ok_json(&profile)
}
