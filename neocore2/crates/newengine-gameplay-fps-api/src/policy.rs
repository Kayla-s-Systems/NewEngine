#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use newengine_gameplay_script_api::GameplayCommandBuffer;
use serde::{Deserialize, Serialize};

pub const FPS_GAMEPLAY_POLICY_SCHEMA: &str = "newengine.gameplay.fps.policy.v1";
pub const FPS_GAMEPLAY_POLICY_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsGameplayPolicySnapshot {
    pub schema: String,
    pub version: u32,
    /// Authored item/weapon/loadout package. The FPS content compiler owns the
    /// conversion from this DTO into generic engine inventory components.
    pub content: serde_json::Value,
    pub required_content: FpsRequiredContentPolicy,
    pub player: FpsPlayerPolicy,
    pub combat: FpsCombatPolicy,
    pub mission: FpsMissionPolicy,
    pub callbacks: FpsCallbackExports,
}

impl Default for FpsGameplayPolicySnapshot {
    fn default() -> Self {
        Self {
            schema: FPS_GAMEPLAY_POLICY_SCHEMA.to_owned(),
            version: FPS_GAMEPLAY_POLICY_VERSION,
            content: serde_json::json!({
                "schema": "newengine.items.package.v1",
                "version": 1,
                "items": [],
                "loadouts": []
            }),
            required_content: FpsRequiredContentPolicy::default(),
            player: FpsPlayerPolicy::default(),
            combat: FpsCombatPolicy::default(),
            mission: FpsMissionPolicy::default(),
            callbacks: FpsCallbackExports::default(),
        }
    }
}

impl FpsGameplayPolicySnapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FPS_GAMEPLAY_POLICY_SCHEMA {
            return Err(format!(
                "FPS gameplay policy schema mismatch: expected '{}' got '{}'",
                FPS_GAMEPLAY_POLICY_SCHEMA, self.schema
            ));
        }
        if self.version != FPS_GAMEPLAY_POLICY_VERSION {
            return Err(format!(
                "FPS gameplay policy version mismatch: expected {} got {}",
                FPS_GAMEPLAY_POLICY_VERSION, self.version
            ));
        }
        if !self.content.is_object() {
            return Err("FPS gameplay policy content must be an item-package object".to_owned());
        }
        self.required_content.validate()?;
        self.player.validate()?;
        self.combat.validate()?;
        self.mission.validate()?;
        self.callbacks.validate()?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsRequiredContentPolicy {
    pub default_loadout: String,
    pub primary_weapon: String,
    pub primary_ammo: String,
    pub medkit: String,
}

impl Default for FpsRequiredContentPolicy {
    fn default() -> Self {
        Self {
            default_loadout: "loadout.fps.default".to_owned(),
            primary_weapon: "weapon.rifle.standard".to_owned(),
            primary_ammo: "ammo.rifle.standard".to_owned(),
            medkit: "consumable.medkit.standard".to_owned(),
        }
    }
}

impl FpsRequiredContentPolicy {
    fn validate(&self) -> Result<(), String> {
        for (label, value) in [
            ("default_loadout", &self.default_loadout),
            ("primary_weapon", &self.primary_weapon),
            ("primary_ammo", &self.primary_ammo),
            ("medkit", &self.medkit),
        ] {
            if value.trim().is_empty() {
                return Err(format!(
                    "FPS required content id '{label}' must not be empty"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsPlayerPolicy {
    pub allow_jump: bool,
    pub allow_crouch: bool,
    pub allow_sprint: bool,
    pub allow_interact: bool,
    pub allow_projectile_launch: bool,
}

impl Default for FpsPlayerPolicy {
    fn default() -> Self {
        Self {
            allow_jump: true,
            allow_crouch: true,
            allow_sprint: true,
            allow_interact: true,
            allow_projectile_launch: true,
        }
    }
}

impl FpsPlayerPolicy {
    fn validate(&self) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsCombatPolicy {
    pub allow_fire: bool,
    pub allow_reload: bool,
    pub damage_multiplier: f32,
    pub interaction_range_multiplier: f32,
}

impl Default for FpsCombatPolicy {
    fn default() -> Self {
        Self {
            allow_fire: true,
            allow_reload: true,
            damage_multiplier: 1.0,
            interaction_range_multiplier: 1.0,
        }
    }
}

impl FpsCombatPolicy {
    fn validate(&self) -> Result<(), String> {
        validate_finite_range(
            "combat.damage_multiplier",
            self.damage_multiplier,
            0.0,
            100.0,
        )?;
        validate_finite_range(
            "combat.interaction_range_multiplier",
            self.interaction_range_multiplier,
            0.0,
            10.0,
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsMissionStateMachinePolicy {
    pub enabled: bool,
    pub instance_id: String,
    pub machine_id: String,
    pub initial_state: String,
    pub activate_event: String,
}

impl Default for FpsMissionStateMachinePolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            instance_id: String::new(),
            machine_id: String::new(),
            initial_state: String::new(),
            activate_event: String::new(),
        }
    }
}

impl FpsMissionStateMachinePolicy {
    fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        for (label, value) in [
            ("instance_id", &self.instance_id),
            ("machine_id", &self.machine_id),
            ("initial_state", &self.initial_state),
            ("activate_event", &self.activate_event),
        ] {
            if value.trim().is_empty() {
                return Err(format!(
                    "FPS mission state-machine '{label}' must not be empty when enabled"
                ));
            }
            if value.contains('@') || value.contains('\\') {
                return Err(format!(
                    "FPS mission state-machine '{label}' must be a stable authored id/event, got '{value}'"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsMissionPolicy {
    pub require_pickups: bool,
    pub require_targets: bool,
    pub hazard_fails: bool,
    pub goal_requires_objectives: bool,
    pub state_machine: FpsMissionStateMachinePolicy,
    pub default_status: String,
    pub pickup_status: String,
    pub target_status: String,
    pub hazard_status: String,
    pub goal_locked_status: String,
    pub goal_complete_status: String,
    pub failed_progress_label: String,
    pub completed_progress_label: String,
}

impl Default for FpsMissionPolicy {
    fn default() -> Self {
        Self {
            require_pickups: true,
            require_targets: true,
            hazard_fails: true,
            goal_requires_objectives: true,
            state_machine: FpsMissionStateMachinePolicy::default(),
            default_status:
                "Collect field cores, neutralize targets, avoid hazards, reach extraction."
                    .to_owned(),
            pickup_status: "Core acquired.".to_owned(),
            target_status: "Target neutralized.".to_owned(),
            hazard_status: "You touched a hazard. Relaunch the demo to retry.".to_owned(),
            goal_locked_status: "Beacon locked: collect all cores first.".to_owned(),
            goal_complete_status: "Extraction complete. Stable runtime loop is playable."
                .to_owned(),
            failed_progress_label: "FAILED - touch a hazard to retry scene".to_owned(),
            completed_progress_label: "EXTRACTED".to_owned(),
        }
    }
}

impl FpsMissionPolicy {
    fn validate(&self) -> Result<(), String> {
        self.state_machine.validate()?;
        for (label, value) in [
            ("default_status", &self.default_status),
            ("pickup_status", &self.pickup_status),
            ("target_status", &self.target_status),
            ("hazard_status", &self.hazard_status),
            ("goal_locked_status", &self.goal_locked_status),
            ("goal_complete_status", &self.goal_complete_status),
            ("failed_progress_label", &self.failed_progress_label),
            ("completed_progress_label", &self.completed_progress_label),
        ] {
            if value.trim().is_empty() {
                return Err(format!("FPS mission policy '{label}' must not be empty"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsCallbackExports {
    pub interaction: String,
    pub hit: String,
    pub mission_event: String,
}

impl Default for FpsCallbackExports {
    fn default() -> Self {
        Self {
            interaction: "on_interaction".to_owned(),
            hit: "on_hit".to_owned(),
            mission_event: "on_mission_event".to_owned(),
        }
    }
}

impl FpsCallbackExports {
    fn validate(&self) -> Result<(), String> {
        for (label, value) in [
            ("interaction", &self.interaction),
            ("hit", &self.hit),
            ("mission_event", &self.mission_event),
        ] {
            if value.contains('@') || value.contains('/') || value.contains('\\') {
                return Err(format!(
                    "FPS callback export '{label}' must be an operation name, not a script selector/path: '{value}'"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FpsPolicyEvent {
    Interaction {
        player: u64,
        target: u64,
        prompt: String,
        fixed_tick: u64,
        point: [f32; 3],
    },
    Hit {
        shooter: u64,
        target: Option<u64>,
        shot_sequence: u64,
        base_damage: f32,
        fixed_tick: u64,
        point: [f32; 3],
        normal: [f32; 3],
    },
    Mission {
        pickups_collected: u32,
        pickups_total: u32,
        targets_destroyed: u32,
        targets_total: u32,
        collected_delta: u32,
        destroyed_delta: u32,
        hit_hazard: bool,
        reached_goal: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsPolicyDecision {
    pub allow_default: bool,
    pub collect_item: Option<bool>,
    pub damage_multiplier: f32,
    pub completed: Option<bool>,
    pub failed: Option<bool>,
    pub status: Option<String>,
    pub commands: GameplayCommandBuffer,
}

impl Default for FpsPolicyDecision {
    fn default() -> Self {
        Self {
            allow_default: true,
            collect_item: None,
            damage_multiplier: 1.0,
            completed: None,
            failed: None,
            status: None,
            commands: GameplayCommandBuffer::default(),
        }
    }
}

impl FpsPolicyDecision {
    pub fn validate(&self) -> Result<(), String> {
        validate_finite_range(
            "decision.damage_multiplier",
            self.damage_multiplier,
            0.0,
            100.0,
        )?;
        if self.status.as_ref().is_some_and(|value| value.len() > 4096) {
            return Err("FPS policy callback status exceeds 4096 bytes".to_owned());
        }
        if !self.commands.commands.is_empty() {
            self.commands.validate_envelope(64)?;
        }
        Ok(())
    }
}

pub trait FpsGameplayPolicyProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn load_snapshot(&self) -> Result<Arc<FpsGameplayPolicySnapshot>, String>;
    fn invoke_event(
        &self,
        export: &str,
        event: &FpsPolicyEvent,
    ) -> Result<FpsPolicyDecision, String>;
}

fn validate_finite_range(label: &str, value: f32, min: f32, max: f32) -> Result<(), String> {
    if !value.is_finite() || !(min..=max).contains(&value) {
        return Err(format!(
            "FPS policy '{label}' must be finite in [{min}, {max}], got {value}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_contract_is_valid() {
        FpsGameplayPolicySnapshot::default().validate().unwrap();
    }

    #[test]
    fn callback_export_is_not_a_ysc_selector() {
        let mut policy = FpsGameplayPolicySnapshot::default();
        policy.callbacks.hit = "scripts/foo.ysc@on_hit".to_owned();
        assert!(policy.validate().is_err());
    }

    #[test]
    fn callback_damage_multiplier_must_be_finite() {
        let decision = FpsPolicyDecision {
            damage_multiplier: f32::NAN,
            ..FpsPolicyDecision::default()
        };
        assert!(decision.validate().is_err());
    }
}
