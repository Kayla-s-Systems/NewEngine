use std::collections::BTreeMap;

use newengine_animation_api::{AnimationSemanticEventKind, AnimationSemanticEventV1};
use newengine_ecs::{EntityId, World};
use newengine_entity_api::EntityHandle;

#[derive(Clone, Debug, Default)]
pub struct AnimationSemanticEventBus {
    events: Vec<AnimationSemanticEventV1>,
    retained: BTreeMap<(u64, String), AnimationSemanticEventV1>,
    next_sequence: u64,
    dropped_events: u64,
}

impl AnimationSemanticEventBus {
    pub const MAX_RETAINED_EVENTS: usize = 2048;

    pub fn publish(&mut self, mut event: AnimationSemanticEventV1) -> Result<u64, String> {
        event.validate()?;
        self.next_sequence = self.next_sequence.wrapping_add(1).max(1);
        event.sequence = self.next_sequence;
        if matches!(event.kind, AnimationSemanticEventKind::State) {
            self.retained.insert(
                (event.entity.stable_id, event.channel.clone()),
                event.clone(),
            );
        }
        if self.events.len() >= Self::MAX_RETAINED_EVENTS {
            let overflow = self.events.len() + 1 - Self::MAX_RETAINED_EVENTS;
            self.events.drain(0..overflow);
            self.dropped_events = self.dropped_events.saturating_add(overflow as u64);
        }
        let sequence = event.sequence;
        self.events.push(event);
        Ok(sequence)
    }

    pub fn drain(&mut self) -> Vec<AnimationSemanticEventV1> {
        std::mem::take(&mut self.events)
    }

    pub fn retained_for_entity(&self, entity: EntityId) -> Vec<AnimationSemanticEventV1> {
        let stable_id = entity.stable_u64();
        self.retained
            .iter()
            .filter_map(|((owner, _), event)| (*owner == stable_id).then_some(event.clone()))
            .collect()
    }

    pub fn clear_entity(&mut self, entity: EntityId) {
        let stable_id = entity.stable_u64();
        self.retained.retain(|(owner, _), _| *owner != stable_id);
        self.events
            .retain(|event| event.entity.stable_id != stable_id);
    }

    pub fn dropped_events(&self) -> u64 {
        self.dropped_events
    }
}

pub fn publish_animation_semantic_event(
    world: &mut World,
    event: AnimationSemanticEventV1,
) -> Result<u64, String> {
    world
        .resource_mut_or_insert_default::<AnimationSemanticEventBus>()
        .publish(event)
}

pub fn emit_animation_state(
    world: &mut World,
    entity: EntityId,
    channel: impl Into<String>,
    event: impl Into<String>,
    parameters: serde_json::Value,
) -> Result<u64, String> {
    publish_animation_semantic_event(
        world,
        AnimationSemanticEventV1::state(
            EntityHandle::new(entity.stable_u64()),
            channel,
            event,
            parameters,
        ),
    )
}

pub fn emit_animation_pulse(
    world: &mut World,
    entity: EntityId,
    channel: impl Into<String>,
    event: impl Into<String>,
    parameters: serde_json::Value,
) -> Result<u64, String> {
    publish_animation_semantic_event(
        world,
        AnimationSemanticEventV1::pulse(
            EntityHandle::new(entity.stable_u64()),
            channel,
            event,
            parameters,
        ),
    )
}

pub fn drain_animation_semantic_events(world: &mut World) -> Vec<AnimationSemanticEventV1> {
    world
        .resource_mut::<AnimationSemanticEventBus>()
        .map(AnimationSemanticEventBus::drain)
        .unwrap_or_default()
}

pub fn retained_animation_states(world: &World, entity: EntityId) -> Vec<AnimationSemanticEventV1> {
    world
        .resource::<AnimationSemanticEventBus>()
        .map(|bus| bus.retained_for_entity(entity))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_is_retained_but_pulse_is_not_replayed() {
        let mut world = World::new();
        let entity = world.spawn();
        emit_animation_state(
            &mut world,
            entity,
            "character.locomotion",
            "character.locomotion.walk",
            serde_json::json!({"normalized_speed": 0.6}),
        )
        .unwrap();
        emit_animation_pulse(
            &mut world,
            entity,
            "character.action",
            "character.action.attack",
            serde_json::json!({"sequence": 2}),
        )
        .unwrap();
        let drained = drain_animation_semantic_events(&mut world);
        assert_eq!(drained.len(), 2);
        let retained = retained_animation_states(&world, entity);
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].event, "character.locomotion.walk");
    }

    #[test]
    fn later_state_replaces_same_channel_without_replaying_old_state() {
        let mut world = World::new();
        let entity = world.spawn();
        emit_animation_state(
            &mut world,
            entity,
            "character.locomotion",
            "character.locomotion.idle",
            serde_json::Value::Null,
        )
        .unwrap();
        emit_animation_state(
            &mut world,
            entity,
            "character.locomotion",
            "character.locomotion.run",
            serde_json::Value::Null,
        )
        .unwrap();
        let retained = retained_animation_states(&world, entity);
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].event, "character.locomotion.run");
    }
}
