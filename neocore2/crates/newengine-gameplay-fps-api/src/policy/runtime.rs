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
            allow_projectile_launch: false,
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
    pub allow_melee: bool,
    pub allow_reload: bool,
    pub damage_multiplier: f32,
    pub interaction_range_multiplier: f32,
}

impl Default for FpsCombatPolicy {
    fn default() -> Self {
        Self {
            allow_fire: true,
            allow_melee: true,
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsMissionStateMachinePolicy {
    pub enabled: bool,
    pub instance_id: String,
    pub machine_id: String,
    pub initial_state: String,
    pub activate_event: String,
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
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
