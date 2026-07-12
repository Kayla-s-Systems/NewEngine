use super::*;

impl InputBindingsProfile {
    /// Canonicalizes only this profile. It does not inject product/gameplay defaults.
    #[inline]
    pub fn canonicalized(self) -> Self {
        self.canonicalized_with_defaults(&InputBindingsProfile::empty(
            "newengine.input.defaults.empty",
        ))
    }

    /// Canonicalizes this profile over a profile-owned default layer.
    /// Generic bindings API owns merging rules; a product/profile crate owns the default data.
    pub fn canonicalized_with_defaults(mut self, defaults: &InputBindingsProfile) -> Self {
        self.id = newengine_input_actions_api::normalize_id_like(&self.id)
            .unwrap_or_else(|| defaults.id.clone());
        self.version = self.version.max(defaults.version).max(4);

        let mut keys: Vec<_> = defaults
            .keys
            .clone()
            .into_iter()
            .filter_map(InputKeyRegistration::normalized)
            .collect();
        for key in self
            .keys
            .into_iter()
            .filter_map(InputKeyRegistration::normalized)
        {
            upsert_key_registration(&mut keys, key);
        }
        keys.sort_by(|a, b| a.code.cmp(&b.code).then_with(|| a.id.cmp(&b.id)));
        self.keys = keys;

        let mut actions: Vec<_> = defaults
            .actions
            .clone()
            .into_iter()
            .filter_map(InputActionDefinition::normalized)
            .collect();
        for action in self
            .actions
            .into_iter()
            .filter_map(InputActionDefinition::normalized)
        {
            upsert_action_definition(&mut actions, action);
        }
        self.actions = actions;

        let mut bindings = Vec::new();
        for binding in self
            .bindings
            .into_iter()
            .filter_map(InputBinding::normalized)
        {
            if !bindings
                .iter()
                .any(|existing| bindings_equivalent(existing, &binding))
            {
                bindings.push(binding);
            }
        }
        for binding in defaults
            .bindings
            .clone()
            .into_iter()
            .filter_map(InputBinding::normalized)
        {
            if !bindings
                .iter()
                .any(|existing| bindings_equivalent(existing, &binding))
            {
                bindings.push(binding);
            }
        }
        self.bindings = bindings;

        let mut listeners: Vec<_> = defaults
            .listeners
            .clone()
            .into_iter()
            .filter_map(InputActionListenerRegistration::normalized)
            .collect();
        for listener in self
            .listeners
            .into_iter()
            .filter_map(InputActionListenerRegistration::normalized)
        {
            upsert_listener_registration(&mut listeners, listener);
        }
        listeners.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        self.listeners = listeners;

        let mut gamepad_axes: Vec<_> = self
            .gamepad_axes
            .into_iter()
            .filter_map(canonical_axis_binding)
            .collect();
        for axis in defaults
            .gamepad_axes
            .clone()
            .into_iter()
            .filter_map(canonical_axis_binding)
        {
            if !gamepad_axes
                .iter()
                .any(|existing| existing.axis == axis.axis && existing.target == axis.target)
            {
                gamepad_axes.push(axis);
            }
        }
        self.gamepad_axes = gamepad_axes;
        self
    }
}
