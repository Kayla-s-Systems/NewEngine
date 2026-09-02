#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct FpsProjectEventSubscription {
    pub event: String,
    pub operation: String,
}

impl FpsProjectEventSubscription {
    pub fn validate(&self) -> Result<(), String> {
        let event = self.event.trim();
        if event.is_empty() || event.len() > 256 || event.chars().any(char::is_control) {
            return Err(format!(
                "invalid FPS project event subscription id '{}'",
                self.event
            ));
        }
        let wildcard_count = event.matches('*').count();
        let allowed_wildcards = usize::from(event.ends_with('*'));
        if wildcard_count > allowed_wildcards {
            return Err(format!(
                "subscription wildcard is only allowed as trailing '*': '{}'",
                self.event
            ));
        }
        let operation = self.operation.trim();
        if operation.is_empty() || operation.len() > 256 {
            return Err(
                "FPS project event subscription operation must contain 1..=256 bytes".to_owned(),
            );
        }
        if operation.contains('@') || operation.contains('/') || operation.contains('\\') {
            return Err(format!(
                "subscription operation must be a name, not selector/path: '{}'",
                self.operation
            ));
        }
        Ok(())
    }
    pub fn matches(&self, event_id: &str) -> bool {
        let pattern = self.event.trim();
        if let Some(prefix) = pattern.strip_suffix('*') {
            event_id
                .to_ascii_lowercase()
                .starts_with(&prefix.to_ascii_lowercase())
        } else {
            pattern.eq_ignore_ascii_case(event_id)
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FpsCallbackExports {
    pub interaction: String,
    pub hit: String,
    pub mission_event: String,
}

impl FpsCallbackExports {
    fn validate(&self) -> Result<(), String> {
        for (label, value) in [
            ("interaction", &self.interaction),
            ("hit", &self.hit),
            ("mission_event", &self.mission_event),
        ] {
            if value.trim().is_empty() {
                continue;
            }
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
    Project {
        id: String,
        source: Option<u64>,
        payload: serde_json::Value,
    },
    Interaction {
        player: u64,
        target: u64,
        prompt: String,
        fixed_tick: u64,
        point: [f32; 3],
    },
    Hit {
        shooter: u64,
        /// Concrete inventory weapon instance captured when the shot was authored.
        weapon_instance_id: u64,
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
