use super::*;

#[inline]
pub fn input_bindings_profile_snapshot() -> InputBindingsProfile {
    gateway_state().lock().profile.clone()
}

#[inline]
pub fn resolve_input_actions<T: InputFrameSource>(input: &T) -> InputActionFrame {
    let state_ref = gateway_state();
    let state = state_ref.lock();
    state.profile.resolve(input)
}

pub fn save_input_bindings_profile(
    profile: InputBindingsProfile,
) -> Result<InputBindingsProfile, String> {
    let (path, profile) = {
        let state_ref = gateway_state();
        let mut state = state_ref.lock();
        let profile = profile.canonicalized_with_defaults(&state.default_profile);
        state.profile = profile.clone();
        (state.profile_path.clone(), profile)
    };
    save_profile_to_config(&path, &profile)?;
    Ok(profile)
}

pub fn reset_input_bindings_profile() -> Result<InputBindingsProfile, String> {
    let profile = gateway_state()
        .lock()
        .default_profile
        .clone()
        .canonicalized();
    save_input_bindings_profile(profile)
}

pub(crate) fn mutate_profile_state_result<F>(
    state: &mut InputBindingsGatewayState,
    mutate: F,
) -> Result<InputBindingsProfile, String>
where
    F: FnOnce(&mut InputBindingsProfile) -> Result<(), String>,
{
    mutate(&mut state.profile)
        .map_err(|e| format!("engine.input.bindings: registration failed: {}", e))?;
    state.profile = state
        .profile
        .clone()
        .canonicalized_with_defaults(&state.default_profile);
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

pub fn register_input_action(
    action: InputActionDefinition,
) -> Result<InputBindingsProfile, String> {
    let state = gateway_state();
    mutate_profile_result(&state, |profile| profile.register_action(action))
}

pub fn register_input_binding(
    registration: InputBindingRegistration,
) -> Result<InputBindingsProfile, String> {
    let state = gateway_state();
    mutate_profile_result(&state, |profile| profile.register_binding(registration))
}

pub fn register_input_listener(
    listener: InputActionListenerRegistration,
) -> Result<InputBindingsProfile, String> {
    let state = gateway_state();
    mutate_profile_result(&state, |profile| profile.register_listener(listener))
}

pub fn register_input_manifest(
    manifest: InputBindingsManifest,
) -> Result<InputBindingsProfile, String> {
    let state = gateway_state();
    mutate_profile_result(&state, |profile| manifest.apply_to(profile))
}
