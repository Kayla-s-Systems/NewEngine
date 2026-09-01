use newengine_ecs::{EntityId, World};
use newengine_primitives::{Primitive, PrimitiveId};

use super::{
    DisplayMode, DisplayVisibility, PlayerEventBus, PlayerEventKind,
    PlayerFirstPersonPrimitiveVariant, PlayerViewState, PlayerViewVisibility,
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
    world.insert_resource(PlayerViewState {
        first_person_active,
    });

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

    // Full-body FPP uses the same skinned owner entity, but mixed torso meshes may contain a
    // camera-near neck shell. Swap only topology; material, skin palette and entity identity stay
    // unchanged. Restoring third person puts the exact authored world primitive back.
    let primitive_updates = world
        .query::<PlayerFirstPersonPrimitiveVariant>()
        .filter_map(|(entity, variant)| {
            let desired = if first_person_active {
                variant.first_person_primitive
            } else {
                variant.world_primitive
            };
            let current = world.get::<Primitive>(entity).map(|primitive| primitive.id);
            (current != Some(desired)).then_some((entity, desired))
        })
        .collect::<Vec<(EntityId, PrimitiveId)>>();
    for (entity, desired) in primitive_updates {
        if let Some(primitive) = world.get_mut::<Primitive>(entity) {
            primitive.id = desired;
        }
    }
}

#[inline]
pub fn drain_player_events(world: &mut World) -> Vec<super::PlayerEvent> {
    world
        .resource_mut::<PlayerEventBus>()
        .map(PlayerEventBus::drain)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::{PlayerVisualKind, PlayerVisualPart};

    #[test]
    fn first_person_primitive_variant_swaps_without_hiding_the_torso() {
        let mut world = World::new();
        let owner = world.spawn();
        let visual = world.spawn();
        let world_id = PrimitiveId(0x1001);
        let first_person_id = PrimitiveId(0x1002);
        let _ = world.insert(
            visual,
            Primitive {
                id: world_id,
                color: [1.0; 4],
            },
        );
        let _ = world.insert(
            visual,
            PlayerVisualPart {
                owner,
                part_index: 0,
                kind: PlayerVisualKind::RuntimeModelPart,
                material_slot: "torso".to_owned(),
            },
        );
        let _ = world.insert(visual, PlayerViewVisibility::runtime_model_default());
        let _ = world.insert(
            visual,
            DisplayVisibility {
                mode: DisplayMode::GameOnly,
            },
        );
        let _ = world.insert(
            visual,
            PlayerFirstPersonPrimitiveVariant {
                world_primitive: world_id,
                first_person_primitive: first_person_id,
            },
        );

        sync_player_view_listeners(&mut world, true);
        assert_eq!(world.get::<Primitive>(visual).unwrap().id, first_person_id);
        assert_eq!(
            world.get::<DisplayVisibility>(visual).unwrap().mode,
            DisplayMode::GameOnly
        );

        sync_player_view_listeners(&mut world, false);
        assert_eq!(world.get::<Primitive>(visual).unwrap().id, world_id);
    }
}
