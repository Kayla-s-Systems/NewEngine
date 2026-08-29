use super::*;

impl InputBindingsProfile {
    pub fn register_key(&mut self, key: InputKeyRegistration) -> Result<(), String> {
        let key = key
            .normalized()
            .ok_or_else(|| "invalid input key registration".to_owned())?;
        upsert_key_registration(&mut self.keys, key);
        self.keys
            .sort_by(|a, b| a.code.cmp(&b.code).then_with(|| a.id.cmp(&b.id)));
        Ok(())
    }

    pub fn register_action(&mut self, action: InputActionDefinition) -> Result<(), String> {
        let action = action
            .normalized()
            .ok_or_else(|| "invalid input action registration".to_owned())?;
        upsert_action_definition(&mut self.actions, action);
        Ok(())
    }

    pub fn register_binding(
        &mut self,
        registration: InputBindingRegistration,
    ) -> Result<(), String> {
        let binding = registration
            .binding
            .normalized()
            .ok_or_else(|| "invalid input binding registration".to_owned())?;
        if !self
            .actions
            .iter()
            .any(|action| action.id == binding.action)
        {
            return Err(format!(
                "input binding references undeclared action '{}'",
                binding.action
            ));
        }
        if registration.replace_existing_for_action_device {
            self.bindings.retain(|existing| {
                !(existing.action == binding.action && existing.device == binding.device)
            });
        } else if self
            .bindings
            .iter()
            .any(|existing| bindings_equivalent(existing, &binding))
        {
            return Ok(());
        }
        self.bindings.push(binding);
        Ok(())
    }

    pub fn register_listener(
        &mut self,
        listener: InputActionListenerRegistration,
    ) -> Result<(), String> {
        let listener = listener
            .normalized()
            .ok_or_else(|| "invalid input listener registration".to_owned())?;
        upsert_listener_registration(&mut self.listeners, listener);
        Ok(())
    }

    pub fn register_gamepad_axis(&mut self, axis: GamepadAxisBinding) -> Result<(), String> {
        let axis = canonical_axis_binding(axis)
            .ok_or_else(|| "invalid empty gamepad axis binding".to_owned())?;
        self.gamepad_axes
            .retain(|existing| !(existing.axis == axis.axis && existing.target == axis.target));
        self.gamepad_axes.push(axis);
        Ok(())
    }
}
