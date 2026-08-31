use newengine_ecs::{EntityId, World};
use serde::{Deserialize, Serialize};

/// Stable semantic weapon events published by engine mechanics and consumed by project policy.
/// Event IDs describe facts, never a particular audio/VFX implementation.
pub const GAMEPLAY_EVENT_WEAPON_FIRED: &str = "gameplay.weapon.fired";
pub const GAMEPLAY_EVENT_WEAPON_EMPTY: &str = "gameplay.weapon.empty";
pub const GAMEPLAY_EVENT_WEAPON_RELOAD_STARTED: &str = "gameplay.weapon.reload.started";
pub const GAMEPLAY_EVENT_WEAPON_RELOAD_COMPLETED: &str = "gameplay.weapon.reload.completed";
pub const GAMEPLAY_EVENT_WEAPON_MELEE_ATTACKED: &str = "gameplay.weapon.melee.attacked";
pub const GAMEPLAY_EVENT_WEAPON_HIT: &str = "gameplay.weapon.hit";
pub const GAMEPLAY_EVENT_WEAPON_EQUIPPED: &str = "gameplay.weapon.equipped";
pub const GAMEPLAY_EVENT_WEAPON_UNEQUIPPED: &str = "gameplay.weapon.unequipped";
pub const GAMEPLAY_EVENT_WEAPON_SHELL_EJECTED: &str = "gameplay.weapon.shell.ejected";
pub const GAMEPLAY_EVENT_WEAPON_SHELL_CONTACT: &str = "gameplay.weapon.shell.contact";
pub const GAMEPLAY_EVENT_WEAPON_SHELL_ROLLING: &str = "gameplay.weapon.shell.rolling";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GameplayEvent {
    pub id: String,
    pub source: Option<u64>,
    pub payload: serde_json::Value,
}

impl Default for GameplayEvent {
    fn default() -> Self {
        Self {
            id: String::new(),
            source: None,
            payload: serde_json::Value::Null,
        }
    }
}

impl GameplayEvent {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Self::default()
        }
    }
    pub fn with_source(mut self, source: EntityId) -> Self {
        self.source = Some(source.stable_u64());
        self
    }
    pub fn with_stable_source(mut self, source: u64) -> Self {
        self.source = Some(source);
        self
    }
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        let id = self.id.trim();
        if id.is_empty() || id.len() > 256 {
            return Err("gameplay event id must contain 1..=256 bytes".to_owned());
        }
        if id.chars().any(char::is_control) {
            return Err(format!(
                "gameplay event id contains control characters: '{id}'"
            ));
        }
        let payload_bytes = serde_json::to_vec(&self.payload)
            .map_err(|error| format!("serialize gameplay event '{id}' payload: {error}"))?;
        if payload_bytes.len() > 64 * 1024 {
            return Err(format!(
                "gameplay event '{id}' payload exceeds 65536 bytes: {}",
                payload_bytes.len()
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct GameplayEventBus {
    events: Vec<GameplayEvent>,
    dropped_events: u64,
}

impl GameplayEventBus {
    pub const MAX_RETAINED_EVENTS: usize = 1024;
    pub fn publish(&mut self, event: GameplayEvent) -> Result<(), String> {
        event.validate()?;
        if self.events.len() >= Self::MAX_RETAINED_EVENTS {
            let overflow = self.events.len() + 1 - Self::MAX_RETAINED_EVENTS;
            self.events.drain(0..overflow);
            self.dropped_events = self.dropped_events.saturating_add(overflow as u64);
        }
        self.events.push(event);
        Ok(())
    }
    pub fn drain(&mut self) -> Vec<GameplayEvent> {
        std::mem::take(&mut self.events)
    }
    pub fn len(&self) -> usize {
        self.events.len()
    }
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
    pub fn dropped_events(&self) -> u64 {
        self.dropped_events
    }
}

pub fn publish_gameplay_event(world: &mut World, event: GameplayEvent) -> Result<(), String> {
    world
        .resource_mut_or_insert_default::<GameplayEventBus>()
        .publish(event)
}

pub fn emit_gameplay_event(
    world: &mut World,
    id: impl Into<String>,
    source: Option<EntityId>,
    payload: serde_json::Value,
) -> Result<(), String> {
    let mut event = GameplayEvent::new(id).with_payload(payload);
    if let Some(source) = source {
        event = event.with_source(source);
    }
    publish_gameplay_event(world, event)
}

pub fn drain_gameplay_events(world: &mut World) -> Vec<GameplayEvent> {
    world
        .resource_mut::<GameplayEventBus>()
        .map(GameplayEventBus::drain)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arbitrary_event_ids_and_payloads_round_trip() {
        let mut world = World::new();
        let source = world.spawn();
        emit_gameplay_event(
            &mut world,
            "project.avatar.inspect.head_turn",
            Some(source),
            serde_json::json!({"yaw": 0.42, "authored": true}),
        )
        .unwrap();
        let events = drain_gameplay_events(&mut world);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, "project.avatar.inspect.head_turn");
        assert_eq!(events[0].source, Some(source.stable_u64()));
        assert_eq!(events[0].payload["authored"], true);
    }
}
