#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::{RResult, RString};
use newengine_input_actions_api::{InputActionDefinition, InputActionFrame, InputActionListenerRegistration, InputFrameSource};
use newengine_input_bindings_api::{
    InputBindingRegistration, InputKeyRegistration, InputBindingsManifest, InputBindingsProfile, InputBindingsServiceInfo, ENGINE_INPUT_BINDINGS_SERVICE_ID,
    INPUT_BINDINGS_BACKEND_CAPABILITY_ID, INPUT_BINDINGS_METHOD_ACTION_CATALOG_JSON_V1,
    INPUT_BINDINGS_METHOD_KEY_CATALOG_JSON_V1, INPUT_BINDINGS_METHOD_INVOKE, INPUT_BINDINGS_METHOD_PROFILE_JSON_V1,
    INPUT_BINDINGS_METHOD_REGISTER_ACTION_JSON_V1, INPUT_BINDINGS_METHOD_REGISTER_BINDING_JSON_V1,
    INPUT_BINDINGS_METHOD_REGISTER_KEY_JSON_V1,
    INPUT_BINDINGS_METHOD_REGISTER_LISTENER_JSON_V1, INPUT_BINDINGS_METHOD_REGISTER_MANIFEST_JSON_V1,
    INPUT_BINDINGS_METHOD_RESET_PROFILE_JSON_V1, INPUT_BINDINGS_METHOD_SAVE_PROFILE_JSON_V1,
    INPUT_BINDINGS_METHOD_SHUTDOWN_V1,
};
use newengine_plugin_api::Blob;
use newengine_service_kit::{
    decode_json_payload, engine_owned_service_description, ok_empty_blob, ok_json,
    register_engine_owned_gateway_service, EngineOwnedGatewayDecl, JsonServiceRouter,
};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

static INPUT_BINDINGS_GATEWAY: OnceLock<Arc<Mutex<InputBindingsGatewayState>>> = OnceLock::new();

#[derive(Clone, Debug)]
struct InputBindingsGatewayState {
    profile: InputBindingsProfile,
    default_profile: InputBindingsProfile,
    profile_path: PathBuf,
}

impl InputBindingsGatewayState {
    #[inline]
    fn new(default_profile: InputBindingsProfile) -> Self {
        let profile_path = profile_path();
        let default_profile = default_profile.canonicalized();
        let profile = load_profile_from_config(&profile_path)
            .map(|profile| profile.canonicalized_with_defaults(&default_profile))
            .unwrap_or_else(|| {
                let default = default_profile.clone();
                let _ = save_profile_to_config(&profile_path, &default);
                default
            });
        Self { profile, default_profile, profile_path }
    }
}


const INPUT_BINDINGS_GATEWAY_OWNER: &str = "newengine-input-bindings-runtime.bindings-gateway";

fn input_bindings_gateway_service(
    state: Arc<Mutex<InputBindingsGatewayState>>,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let info = InputBindingsServiceInfo::default();
    let description = engine_owned_service_description(
        ENGINE_INPUT_BINDINGS_SERVICE_ID,
        INPUT_BINDINGS_GATEWAY_OWNER,
        INPUT_BINDINGS_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .version(2)
    .protocol(info.protocol.clone())
    .features(info.features.clone())
    .gateway("engine-owned engine.input.bindings profile service");

    JsonServiceRouter::with_shared_state(ENGINE_INPUT_BINDINGS_SERVICE_ID, state)
        .describe_json(&description)
        .info(InputBindingsServiceInfo::default)
        .get_json(INPUT_BINDINGS_METHOD_PROFILE_JSON_V1, |state| state.profile.clone())
        .get_json(INPUT_BINDINGS_METHOD_ACTION_CATALOG_JSON_V1, |state| state.profile.actions.clone())
        .get_json(INPUT_BINDINGS_METHOD_KEY_CATALOG_JSON_V1, |state| state.profile.keys.clone())
        .blob(INPUT_BINDINGS_METHOD_INVOKE, save_profile_payload)
        .blob(INPUT_BINDINGS_METHOD_SAVE_PROFILE_JSON_V1, save_profile_payload)
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
                mutate_profile_state_result(state, |profile| profile.register_listener(registration))
            },
        )
        .post_json_result::<InputBindingsManifest, InputBindingsProfile, _>(
            INPUT_BINDINGS_METHOD_REGISTER_MANIFEST_JSON_V1,
            |state, manifest| mutate_profile_state_result(state, |profile| manifest.apply_to(profile)),
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
        .blob(INPUT_BINDINGS_METHOD_SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
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

fn profile_path() -> PathBuf {
    newengine_core::config_child("input/bindings.gameplay.json")
}

fn load_profile_from_config(path: &PathBuf) -> Option<InputBindingsProfile> {
    let txt = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<InputBindingsProfile>(&txt).ok()
}

fn save_profile_to_config(path: &PathBuf, profile: &InputBindingsProfile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let txt = serde_json::to_string_pretty(profile).map_err(|e| e.to_string())?;
    std::fs::write(path, txt).map_err(|e| e.to_string())
}

fn gateway_state_with_default(default_profile: InputBindingsProfile) -> Arc<Mutex<InputBindingsGatewayState>> {
    Arc::clone(INPUT_BINDINGS_GATEWAY.get_or_init(|| Arc::new(Mutex::new(InputBindingsGatewayState::new(default_profile)))))
}

fn gateway_state() -> Arc<Mutex<InputBindingsGatewayState>> {
    gateway_state_with_default(InputBindingsProfile::default())
}

#[inline]
pub fn input_bindings_profile_snapshot() -> InputBindingsProfile {
    gateway_state().lock().profile.clone()
}

#[inline]
pub fn resolve_input_actions<T: InputFrameSource>(
    input: &T,
) -> InputActionFrame {
    let state_ref = gateway_state();
    let state = state_ref.lock();
    state.profile.resolve(input)
}

pub fn save_input_bindings_profile(profile: InputBindingsProfile) -> Result<InputBindingsProfile, String> {
    let path = {
        let state_ref = gateway_state();
        let mut state = state_ref.lock();
        let profile = profile.canonicalized_with_defaults(&state.default_profile);
        state.profile = profile.clone();
        state.profile_path.clone()
    };
    let profile = gateway_state().lock().profile.clone();
    save_profile_to_config(&path, &profile)?;
    Ok(profile)
}

pub fn reset_input_bindings_profile() -> Result<InputBindingsProfile, String> {
    let profile = gateway_state().lock().default_profile.clone().canonicalized();
    save_input_bindings_profile(profile)
}

fn mutate_profile_state_result<F>(
    state: &mut InputBindingsGatewayState,
    mutate: F,
) -> Result<InputBindingsProfile, String>
where
    F: FnOnce(&mut InputBindingsProfile) -> Result<(), String>,
{
    mutate(&mut state.profile)
        .map_err(|e| format!("engine.input.bindings: registration failed: {}", e))?;
    state.profile = state.profile.clone().canonicalized_with_defaults(&state.default_profile);
    let profile = state.profile.clone();
    let path = state.profile_path.clone();
    save_profile_to_config(&path, &profile).map_err(|e| {
        format!(
            "engine.input.bindings: save failed path='{}' err='{}'",
            path.display(),
            e
        )
    })?;
    Ok(profile)
}

fn mutate_profile_result<F>(
    state: &Arc<Mutex<InputBindingsGatewayState>>,
    mutate: F,
) -> Result<InputBindingsProfile, String>
where
    F: FnOnce(&mut InputBindingsProfile) -> Result<(), String>,
{
    let mut state = state.lock();
    mutate_profile_state_result(&mut state, mutate)
}

pub fn register_input_key(key: InputKeyRegistration) -> Result<InputBindingsProfile, String> {
    let state = gateway_state();
    mutate_profile_result(&state, |profile| profile.register_key(key))
}

pub fn register_input_action(action: InputActionDefinition) -> Result<InputBindingsProfile, String> {
    let state = gateway_state();
    mutate_profile_result(&state, |profile| profile.register_action(action))
}

pub fn register_input_binding(registration: InputBindingRegistration) -> Result<InputBindingsProfile, String> {
    let state = gateway_state();
    mutate_profile_result(&state, |profile| profile.register_binding(registration))
}

pub fn register_input_listener(listener: InputActionListenerRegistration) -> Result<InputBindingsProfile, String> {
    let state = gateway_state();
    mutate_profile_result(&state, |profile| profile.register_listener(listener))
}

pub fn register_input_manifest(manifest: InputBindingsManifest) -> Result<InputBindingsProfile, String> {
    let state = gateway_state();
    mutate_profile_result(&state, |profile| manifest.apply_to(profile))
}

pub fn register_input_bindings_gateway_best_effort(default_profile: InputBindingsProfile) {
    if newengine_plugin_host::has_service(ENGINE_INPUT_BINDINGS_SERVICE_ID) {
        return;
    }

    let service = input_bindings_gateway_service(gateway_state_with_default(default_profile));
    match register_engine_owned_gateway_service(EngineOwnedGatewayDecl {
        gateway: ENGINE_INPUT_BINDINGS_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::InputBindings,
        provider_service: ENGINE_INPUT_BINDINGS_SERVICE_ID,
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
            log::info!(
                "input bindings gateway: engine-owned route registered id='{}' capability='{}' config='{}'",
                ENGINE_INPUT_BINDINGS_SERVICE_ID,
                INPUT_BINDINGS_BACKEND_CAPABILITY_ID,
                path.display()
            );
        }
        Err(e) => log::warn!(
            "input bindings gateway: registration skipped id='{}' err='{}'",
            ENGINE_INPUT_BINDINGS_SERVICE_ID,
            e
        ),
    }
}
