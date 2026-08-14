use serde::{Deserialize, Serialize};

pub const GAME_MESSAGE_CONTRACT: &str = "newengine.game-message.v1";
pub const GAME_MESSAGE_DESCRIPTOR_CONTRACT: &str = "newengine.game-message-descriptor.v1";
pub const ENGINE_GAME_EVENTS_SERVICE_ID: &str = "engine.game.events";

pub mod game_events_method {
    pub const INFO_JSON_V1: &str = "game_events.info_json_v1";
    pub const REGISTER_JSON_V1: &str = "game_events.register_json_v1";
    pub const UNREGISTER_JSON_V1: &str = "game_events.unregister_json_v1";
    pub const DESCRIBE_JSON_V1: &str = "game_events.describe_json_v1";
    pub const PUBLISH_JSON_V1: &str = "game_events.publish_json_v1";
    pub const DRAIN_JSON_V1: &str = "game_events.drain_json_v1";
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameMessageScope {
    #[default]
    Local,
    World,
    Entity,
    Player,
    Network,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameMessageReliability {
    #[default]
    BestEffort,
    Reliable,
    OrderedReliable,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameMessageDescriptor {
    pub id: String,
    pub version: u32,
    pub owner: String,
    pub description: String,
    pub payload_schema: serde_json::Value,
    pub scope: GameMessageScope,
    pub reliability: GameMessageReliability,
    pub max_payload_bytes: u32,
    pub tags: Vec<String>,
}

impl Default for GameMessageDescriptor {
    fn default() -> Self {
        Self {
            id: String::new(),
            version: 1,
            owner: String::new(),
            description: String::new(),
            payload_schema: serde_json::Value::Object(serde_json::Map::new()),
            scope: GameMessageScope::Local,
            reliability: GameMessageReliability::BestEffort,
            max_payload_bytes: 64 * 1024,
            tags: Vec::new(),
        }
    }
}

impl GameMessageDescriptor {
    pub fn validate(&self) -> Result<(), String> {
        validate_message_id(&self.id)?;
        if self.version == 0 {
            return Err(format!("game message '{}' version must be >= 1", self.id));
        }
        if self.max_payload_bytes == 0 || self.max_payload_bytes > 16 * 1024 * 1024 {
            return Err(format!(
                "game message '{}' max_payload_bytes must be 1..=16777216",
                self.id
            ));
        }
        if !self.payload_schema.is_object() && !self.payload_schema.is_null() {
            return Err(format!(
                "game message '{}' payload_schema must be an object or null",
                self.id
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameMessageEnvelope {
    pub contract: String,
    pub id: String,
    pub version: u32,
    pub sequence: u64,
    pub frame_index: u64,
    pub source: String,
    pub source_entity: Option<u64>,
    pub target_entity: Option<u64>,
    pub correlation_id: Option<String>,
    pub payload: serde_json::Value,
}

impl Default for GameMessageEnvelope {
    fn default() -> Self {
        Self {
            contract: GAME_MESSAGE_CONTRACT.to_owned(),
            id: String::new(),
            version: 1,
            sequence: 0,
            frame_index: 0,
            source: String::new(),
            source_entity: None,
            target_entity: None,
            correlation_id: None,
            payload: serde_json::Value::Null,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameMessageIdRequest {
    pub id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameMessageMutationResponse {
    pub ok: bool,
    pub id: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameMessageRegistrySnapshot {
    pub contract: String,
    pub generation: u64,
    pub descriptors: Vec<GameMessageDescriptor>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameMessageDrainRequest {
    pub max_messages: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameMessageDrainResponse {
    pub messages: Vec<GameMessageEnvelope>,
    pub remaining: usize,
    pub dropped: u64,
}

pub fn validate_message_id(id: &str) -> Result<(), String> {
    let id = id.trim();
    if id.is_empty() || id.len() > 128 {
        return Err("game message id must contain 1..=128 bytes".to_owned());
    }
    if !id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | ':'))
    {
        return Err(format!(
            "game message id contains unsupported characters: '{id}'"
        ));
    }
    if !id.contains('.') && !id.contains(':') {
        return Err(format!("game message id should be namespaced: '{id}'"));
    }
    Ok(())
}

pub fn validate_envelope_against_descriptor(
    envelope: &GameMessageEnvelope,
    descriptor: &GameMessageDescriptor,
) -> Result<(), String> {
    if envelope.contract != GAME_MESSAGE_CONTRACT {
        return Err(format!(
            "game message contract mismatch: expected '{}' got '{}'",
            GAME_MESSAGE_CONTRACT, envelope.contract
        ));
    }
    if envelope.id != descriptor.id {
        return Err(format!(
            "game message id mismatch: envelope='{}' descriptor='{}'",
            envelope.id, descriptor.id
        ));
    }
    if envelope.version != descriptor.version {
        return Err(format!(
            "game message '{}' version mismatch: expected {} got {}",
            envelope.id, descriptor.version, envelope.version
        ));
    }
    let payload_size = serde_json::to_vec(&envelope.payload)
        .map_err(|error| format!("game message payload serialize failed: {error}"))?
        .len();
    if payload_size > descriptor.max_payload_bytes as usize {
        return Err(format!(
            "game message '{}' payload {} bytes exceeds max {}",
            envelope.id, payload_size, descriptor.max_payload_bytes
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_namespaced_and_stable() {
        assert!(validate_message_id("game.player.spawned").is_ok());
        assert!(validate_message_id("spawned").is_err());
    }

    #[test]
    fn envelope_version_and_size_are_validated() {
        let descriptor = GameMessageDescriptor {
            id: "game.test.event".into(),
            max_payload_bytes: 8,
            ..Default::default()
        };
        let mut envelope = GameMessageEnvelope {
            id: descriptor.id.clone(),
            payload: serde_json::json!({"x":1}),
            ..Default::default()
        };
        assert!(validate_envelope_against_descriptor(&envelope, &descriptor).is_ok());
        envelope.version = 2;
        assert!(validate_envelope_against_descriptor(&envelope, &descriptor).is_err());
    }
}
