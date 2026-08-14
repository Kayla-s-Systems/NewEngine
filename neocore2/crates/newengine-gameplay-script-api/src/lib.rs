#![forbid(unsafe_op_in_unsafe_fn)]

//! Provider-neutral scripted gameplay contracts.
//! Scripts return declarative command buffers; native runtime code validates and applies them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const GAMEPLAY_COMMAND_BUFFER_SCHEMA: &str = "newengine.gameplay.command_buffer.v1";
pub const GAMEPLAY_COMMAND_BUFFER_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GameplayCommandBuffer {
    pub schema: String,
    pub version: u32,
    pub transaction_id: String,
    pub commands: Vec<GameplayCommand>,
}

impl Default for GameplayCommandBuffer {
    fn default() -> Self {
        Self {
            schema: GAMEPLAY_COMMAND_BUFFER_SCHEMA.to_owned(),
            version: GAMEPLAY_COMMAND_BUFFER_VERSION,
            transaction_id: String::new(),
            commands: Vec::new(),
        }
    }
}

impl GameplayCommandBuffer {
    pub fn validate_envelope(&self, max_commands: usize) -> Result<(), String> {
        if self.schema != GAMEPLAY_COMMAND_BUFFER_SCHEMA {
            return Err(format!(
                "gameplay command buffer schema mismatch: expected '{}' got '{}'",
                GAMEPLAY_COMMAND_BUFFER_SCHEMA, self.schema
            ));
        }
        if self.version != GAMEPLAY_COMMAND_BUFFER_VERSION {
            return Err(format!(
                "gameplay command buffer version mismatch: expected {} got {}",
                GAMEPLAY_COMMAND_BUFFER_VERSION, self.version
            ));
        }
        if self.transaction_id.trim().is_empty() || self.transaction_id.len() > 128 {
            return Err("gameplay command transaction_id must contain 1..=128 bytes".to_owned());
        }
        if self.commands.len() > max_commands {
            return Err(format!(
                "gameplay command buffer contains {} commands; max is {}",
                self.commands.len(),
                max_commands
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GameplayCommand {
    DealDamage {
        target: u64,
        amount: f32,
        #[serde(default)]
        source: Option<u64>,
        #[serde(default)]
        damage_type: String,
    },
    GiveItem {
        owner: u64,
        item: String,
        quantity: u32,
    },
    SetObjective {
        objective: String,
        state: GameplayObjectiveState,
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        progress: Option<f32>,
    },
    SpawnArchetype {
        archetype: String,
        #[serde(default = "default_spawn_count")]
        count: u32,
        #[serde(default)]
        transform: Option<GameplaySpawnTransform>,
        #[serde(default)]
        properties: serde_json::Value,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        owner: Option<String>,
    },
    PlayEffect {
        effect: String,
        #[serde(default)]
        position: Option<[f32; 3]>,
        #[serde(default)]
        source: Option<u64>,
        #[serde(default)]
        target: Option<u64>,
        #[serde(default = "default_effect_intensity")]
        intensity: f32,
        #[serde(default)]
        parameters: BTreeMap<String, serde_json::Value>,
    },
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GameplayObjectiveState {
    Hidden,
    #[default]
    Active,
    Completed,
    Failed,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GameplaySpawnTransform {
    pub position: [f32; 3],
    pub rotation_xyzw: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for GameplaySpawnTransform {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GameplayCommandReceipt {
    pub transaction_id: String,
    pub applied_commands: usize,
    pub spawned_entities: Vec<u64>,
    pub total_damage: f32,
    pub items_given: u64,
    pub objectives_touched: Vec<String>,
    pub effects_enqueued: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ScriptedActionRequest {
    pub action: String,
    pub actor: u64,
    pub target: Option<u64>,
    pub context: serde_json::Value,
}

impl Default for ScriptedActionRequest {
    fn default() -> Self {
        Self {
            action: String::new(),
            actor: 0,
            target: None,
            context: serde_json::Value::Null,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ScriptedAbilityRequest {
    pub ability: String,
    pub actor: u64,
    pub target: Option<u64>,
    pub origin: Option<[f32; 3]>,
    pub direction: Option<[f32; 3]>,
    pub context: serde_json::Value,
}

impl Default for ScriptedAbilityRequest {
    fn default() -> Self {
        Self {
            ability: String::new(),
            actor: 0,
            target: None,
            origin: None,
            direction: None,
            context: serde_json::Value::Null,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ScriptedStateMachineStepRequest {
    pub machine: String,
    pub state: String,
    pub actor: Option<u64>,
    pub target: Option<u64>,
    pub event: String,
    pub context: serde_json::Value,
    pub variables: BTreeMap<String, serde_json::Value>,
}

impl Default for ScriptedStateMachineStepRequest {
    fn default() -> Self {
        Self {
            machine: String::new(),
            state: String::new(),
            actor: None,
            target: None,
            event: String::new(),
            context: serde_json::Value::Null,
            variables: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ScriptedStateMachineStepResponse {
    pub next_state: String,
    pub commands: GameplayCommandBuffer,
    pub variables: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ScriptedStateMachineEventRequest {
    pub instance_id: String,
    pub event: String,
    pub context: serde_json::Value,
}

impl Default for ScriptedStateMachineEventRequest {
    fn default() -> Self {
        Self {
            instance_id: String::new(),
            event: String::new(),
            context: serde_json::Value::Null,
        }
    }
}

pub trait ScriptedGameplayProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn invoke_action(
        &self,
        request: &ScriptedActionRequest,
    ) -> Result<GameplayCommandBuffer, String>;
    fn invoke_ability(
        &self,
        request: &ScriptedAbilityRequest,
    ) -> Result<GameplayCommandBuffer, String>;
    fn step_state_machine(
        &self,
        request: &ScriptedStateMachineStepRequest,
    ) -> Result<ScriptedStateMachineStepResponse, String>;
}

#[inline]
fn default_spawn_count() -> u32 {
    1
}

#[inline]
fn default_effect_intensity() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_buffer_schema_is_explicit() {
        let buffer = GameplayCommandBuffer {
            transaction_id: "tx-1".to_owned(),
            ..GameplayCommandBuffer::default()
        };
        buffer.validate_envelope(64).unwrap();
    }

    #[test]
    fn command_buffer_rejects_unbounded_command_count() {
        let mut buffer = GameplayCommandBuffer {
            transaction_id: "tx-many".to_owned(),
            ..GameplayCommandBuffer::default()
        };
        buffer.commands = (0..3)
            .map(|_| GameplayCommand::SetObjective {
                objective: "objective.test".to_owned(),
                state: GameplayObjectiveState::Active,
                status: None,
                progress: None,
            })
            .collect();
        assert!(buffer.validate_envelope(2).is_err());
    }
}
