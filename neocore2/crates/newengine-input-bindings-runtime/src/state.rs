use super::*;

pub(crate) static INPUT_BINDINGS_GATEWAY: OnceLock<Arc<Mutex<InputBindingsGatewayState>>> =
    OnceLock::new();

#[derive(Clone, Debug)]
pub(crate) struct InputBindingsGatewayState {
    pub(crate) profile: InputBindingsProfile,
    pub(crate) default_profile: InputBindingsProfile,
    pub(crate) profile_path: PathBuf,
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
        newengine_ulog_api::ulog::info!(
            "input bindings: initialized profile id='{}' actions={} bindings={} axes={} listeners={} config='{}' default_id='{}' default_actions={} default_bindings={} default_axes={}",
            profile.id,
            profile.actions.len(),
            profile.bindings.len(),
            profile.gamepad_axes.len(),
            profile.listeners.len(),
            profile_path.display(),
            default_profile.id,
            default_profile.actions.len(),
            default_profile.bindings.len(),
            default_profile.gamepad_axes.len(),
        );
        if profile.actions.is_empty() || profile.bindings.is_empty() {
            newengine_ulog_api::ulog::warn!(
                "input bindings: semantic profile has empty action/binding catalog id='{}' actions={} bindings={} config='{}'",
                profile.id,
                profile.actions.len(),
                profile.bindings.len(),
                profile_path.display(),
            );
        }
        Self {
            profile,
            default_profile,
            profile_path,
        }
    }
}

pub(crate) fn gateway_state_with_default(
    default_profile: InputBindingsProfile,
) -> Arc<Mutex<InputBindingsGatewayState>> {
    Arc::clone(
        INPUT_BINDINGS_GATEWAY
            .get_or_init(|| Arc::new(Mutex::new(InputBindingsGatewayState::new(default_profile)))),
    )
}

pub(crate) fn install_or_update_default_profile(
    default_profile: InputBindingsProfile,
) -> Arc<Mutex<InputBindingsGatewayState>> {
    let default_profile = default_profile.canonicalized();
    if let Some(existing) = INPUT_BINDINGS_GATEWAY.get() {
        let state_ref = Arc::clone(existing);
        let mut state = state_ref.lock();
        let old_default_id = state.default_profile.id.clone();
        let old_actions = state.profile.actions.len();
        let old_bindings = state.profile.bindings.len();
        let old_axes = state.profile.gamepad_axes.len();
        state.default_profile = default_profile.clone();
        state.profile = state
            .profile
            .clone()
            .canonicalized_with_defaults(&default_profile);
        let path = state.profile_path.clone();
        if let Err(e) = save_profile_to_config(&path, &state.profile) {
            newengine_ulog_api::ulog::warn!(
                "input bindings: failed to persist updated default merge config='{}' err='{}'",
                path.display(),
                e
            );
        }
        newengine_ulog_api::ulog::info!(
            "input bindings: default profile installed existing_state=true old_default='{}' new_default='{}' profile='{}' actions {}->{} bindings {}->{} axes {}->{} config='{}'",
            old_default_id,
            default_profile.id,
            state.profile.id,
            old_actions,
            state.profile.actions.len(),
            old_bindings,
            state.profile.bindings.len(),
            old_axes,
            state.profile.gamepad_axes.len(),
            path.display(),
        );
        drop(state);
        return state_ref;
    }
    gateway_state_with_default(default_profile)
}

pub(crate) fn gateway_state() -> Arc<Mutex<InputBindingsGatewayState>> {
    gateway_state_with_default(InputBindingsProfile::default())
}
