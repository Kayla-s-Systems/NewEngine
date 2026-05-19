use newengine_ecs::{EntityId, World};

use super::{
    DisplayMode, DisplayVisibility, PlayerEventBus, PlayerEventKind, PlayerViewVisibility,
    PlayerViewVisibilityPolicy, PlayerVisualPart,
};

#[inline]
pub fn emit_player_event(
    world: &mut World,
    entity: EntityId,
    kind: PlayerEventKind,
    message: impl Into<String>,
) {
    world
        .resource_mut_or_insert_default::<PlayerEventBus>()
        .emit(entity, kind, message);
}

#[inline]
fn effective_display_mode(binding: PlayerViewVisibility, first_person_active: bool) -> DisplayMode {
    match (binding.policy, first_person_active) {
        (PlayerViewVisibilityPolicy::HideInFirstPerson, true) => DisplayMode::RuntimeHidden,
        _ => binding.base_mode,
    }
}

/// Listener system for player visual presentation.
///
/// It deliberately operates only on ordinary ECS components. Camera mode is an
/// input signal; the system does not know about a special player singleton and
/// does not reach into renderer state.
pub fn sync_player_view_listeners(world: &mut World, first_person_active: bool) {
    let mut updates: Vec<(EntityId, DisplayMode, EntityId)> = Vec::new();
    for (entity, visual, visibility) in world.query2::<PlayerVisualPart, PlayerViewVisibility>() {
        let mode = effective_display_mode(*visibility, first_person_active);
        let current = world
            .get::<DisplayVisibility>(entity)
            .copied()
            .unwrap_or_default()
            .mode;
        if current != mode {
            updates.push((entity, mode, visual.owner));
        }
    }

    for (entity, mode, owner) in updates {
        let _ = world.insert(entity, DisplayVisibility { mode });
        emit_player_event(
            world,
            owner,
            PlayerEventKind::VisualVisibilityChanged,
            format!("visual_entity={} mode={:?}", entity.stable_u64(), mode),
        );
    }
}

#[inline]
pub fn drain_player_events(world: &mut World) -> Vec<super::PlayerEvent> {
    world
        .resource_mut::<PlayerEventBus>()
        .map(PlayerEventBus::drain)
        .unwrap_or_default()
}
