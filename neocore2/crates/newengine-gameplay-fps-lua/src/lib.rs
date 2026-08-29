#![forbid(unsafe_op_in_unsafe_fn)]

//! Lua-backed FPS policy adapter. The implementation talks only to the generic
//! `engine.scripting` client and never exposes Lua or ECS handles to gameplay.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use newengine_gameplay_fps_api::{
    FpsGameplayPolicyProvider, FpsGameplayPolicySnapshot, FpsPolicyDecision, FpsPolicyEvent,
    FPS_GAMEPLAY_POLICY_SCHEMA, FPS_GAMEPLAY_POLICY_VERSION,
};
use newengine_gameplay_script_api::{
    GameplayCommandBuffer, ScriptedAbilityRequest, ScriptedActionRequest, ScriptedGameplayProvider,
    ScriptedStateMachineStepRequest, ScriptedStateMachineStepResponse,
};
use newengine_scripting_client::AssetBackedScriptClient;

pub const SCRIPT_FPS_GAMEPLAY_PROVIDER_ID: &str = "newengine.gameplay.fps.script-policy";
pub const LUA_FPS_GAMEPLAY_PROVIDER_ID: &str = "newengine.gameplay.fps.lua-policy";

pub struct LuaFpsGameplayPolicyProvider {
    client: AssetBackedScriptClient,
    policy_operation: String,
    snapshot: OnceLock<Arc<FpsGameplayPolicySnapshot>>,
    request_seq: AtomicU64,
}

impl LuaFpsGameplayPolicyProvider {
    pub fn new(script_ref: impl Into<String>) -> Self {
        let script_ref = script_ref.into();
        Self {
            client: AssetBackedScriptClient::new(script_ref, "fps-gameplay-policy"),
            policy_operation: String::new(),
            snapshot: OnceLock::new(),
            request_seq: AtomicU64::new(1),
        }
    }

    #[inline]
    pub fn script_ref(&self) -> &str {
        self.client.script_ref()
    }

    pub fn with_policy_operation(mut self, operation: impl Into<String>) -> Self {
        self.policy_operation = operation.into();
        self
    }

    fn load_uncached(&self) -> Result<Arc<FpsGameplayPolicySnapshot>, String> {
        if self.policy_operation.trim().is_empty() {
            return Err(format!(
                "Script FPS policy provider '{}' has no configured bootstrap operation; bind one in the project scripting registry",
                self.client.script_ref()
            ));
        }
        self.client.load_module()?;
        let snapshot: FpsGameplayPolicySnapshot = self.client.invoke_json_unit(
            "fps-gameplay-policy.bootstrap.v1",
            &self.policy_operation,
            BTreeMap::from([
                (
                    "expected_schema".to_owned(),
                    FPS_GAMEPLAY_POLICY_SCHEMA.to_owned(),
                ),
                (
                    "expected_version".to_owned(),
                    FPS_GAMEPLAY_POLICY_VERSION.to_string(),
                ),
            ]),
        )?;
        snapshot
            .validate()
            .map_err(|error| format!("Script FPS gameplay policy validation failed: {error}"))?;
        Ok(Arc::new(snapshot))
    }
}

impl FpsGameplayPolicyProvider for LuaFpsGameplayPolicyProvider {
    #[inline]
    fn id(&self) -> &'static str {
        SCRIPT_FPS_GAMEPLAY_PROVIDER_ID
    }

    fn load_snapshot(&self) -> Result<Arc<FpsGameplayPolicySnapshot>, String> {
        if let Some(snapshot) = self.snapshot.get() {
            return Ok(Arc::clone(snapshot));
        }
        let loaded = self.load_uncached()?;
        let _ = self.snapshot.set(Arc::clone(&loaded));
        Ok(self.snapshot.get().cloned().unwrap_or(loaded))
    }

    fn invoke_event(
        &self,
        export: &str,
        event: &FpsPolicyEvent,
    ) -> Result<FpsPolicyDecision, String> {
        if export.trim().is_empty() {
            return Ok(FpsPolicyDecision::default());
        }
        // Loading the policy also guarantees that this selectorless module is compiled and cached.
        let _ = self.load_snapshot()?;
        let seq = self.request_seq.fetch_add(1, Ordering::Relaxed);
        let decision: FpsPolicyDecision = self.client.invoke_json(
            format!("fps-gameplay-event-{seq}"),
            export,
            event,
            BTreeMap::from([(
                "event_contract".to_owned(),
                "fps-policy-event-v1".to_owned(),
            )]),
        )?;
        decision.validate().map_err(|error| {
            format!("Script FPS policy callback '{export}' returned invalid decision: {error}")
        })?;
        Ok(decision)
    }
}

impl ScriptedGameplayProvider for LuaFpsGameplayPolicyProvider {
    fn id(&self) -> &'static str {
        SCRIPT_FPS_GAMEPLAY_PROVIDER_ID
    }

    fn invoke_action(
        &self,
        request: &ScriptedActionRequest,
    ) -> Result<GameplayCommandBuffer, String> {
        let _ = self.load_snapshot()?;
        let seq = self.request_seq.fetch_add(1, Ordering::Relaxed);
        let commands: GameplayCommandBuffer = self.client.invoke_json(
            format!("fps-scripted-action-{seq}"),
            "action",
            request,
            BTreeMap::from([(
                "request_contract".to_owned(),
                "scripted-action-v1".to_owned(),
            )]),
        )?;
        if !commands.commands.is_empty() {
            commands.validate_envelope(64)?;
        }
        Ok(commands)
    }

    fn invoke_ability(
        &self,
        request: &ScriptedAbilityRequest,
    ) -> Result<GameplayCommandBuffer, String> {
        let _ = self.load_snapshot()?;
        let seq = self.request_seq.fetch_add(1, Ordering::Relaxed);
        let commands: GameplayCommandBuffer = self.client.invoke_json(
            format!("fps-scripted-ability-{seq}"),
            "ability",
            request,
            BTreeMap::from([(
                "request_contract".to_owned(),
                "scripted-ability-v1".to_owned(),
            )]),
        )?;
        if !commands.commands.is_empty() {
            commands.validate_envelope(64)?;
        }
        Ok(commands)
    }

    fn step_state_machine(
        &self,
        request: &ScriptedStateMachineStepRequest,
    ) -> Result<ScriptedStateMachineStepResponse, String> {
        let _ = self.load_snapshot()?;
        let seq = self.request_seq.fetch_add(1, Ordering::Relaxed);
        let response: ScriptedStateMachineStepResponse = self.client.invoke_json(
            format!("fps-scripted-machine-{seq}"),
            "state_machine_step",
            request,
            BTreeMap::from([(
                "request_contract".to_owned(),
                "scripted-state-machine-v1".to_owned(),
            )]),
        )?;
        if response.next_state.trim().is_empty() {
            return Err("Lua scripted state machine returned empty next_state".to_owned());
        }
        if !response.commands.commands.is_empty() {
            response.commands.validate_envelope(64)?;
        }
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_provider_targets_selectorless_fps_module() {
        let provider = LuaFpsGameplayPolicyProvider::new("scripts/custom_policy.ysc")
            .with_policy_operation("custom_policy_export");
        assert_eq!(
            newengine_gameplay_fps_api::FpsGameplayPolicyProvider::id(&provider),
            LUA_FPS_GAMEPLAY_PROVIDER_ID
        );
        assert_eq!(provider.script_ref(), "scripts/custom_policy.ysc");
        assert!(!provider.script_ref().contains('@'));
    }
}
