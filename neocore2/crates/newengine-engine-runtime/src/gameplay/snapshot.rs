use newengine_bounds::Bounds;
use newengine_ecs::{Component, EntityId, World};
use newengine_math::collections::FxHashSet;
use newengine_sim::{
    AngularVelocity, CameraRigComp, CharacterMotor, FollowTargetCameraController,
    FollowTargetCameraMotor, MotorInput, Velocity,
};
use newengine_transform::Transform;

use super::combat::{PendingHitscan, PendingInteraction};
use super::inventory::{
    EquippedWeaponBinding, InventoryEventBus, InventoryLoadoutCatalog, ItemCatalog, ItemPickup,
    PlayerInventory, WorldItemPresentation, WorldItemRuntime,
};
use super::{
    DisplayVisibility, Health, HitscanWeaponTuning, Interactable, InteractionEventBus,
    InventoryHudState, PhysicsBodyDesc, PhysicsSurface, PlayerCommandFrame, PlayerEventBus,
    PlayerGroundState, PlayerInteractionTuning, PlayerLocomotionState, PlayerStanceState,
    PlayerWeaponState, WeaponEventBus,
};

#[derive(Clone, Debug)]
pub struct RuntimeEntitySnapshot {
    pub entity: EntityId,
    pub transform: Option<Transform>,
    pub velocity: Option<Velocity>,
    pub angular_velocity: Option<AngularVelocity>,
    pub motor_input: Option<MotorInput>,
    pub character_motor: Option<CharacterMotor>,
    pub camera_rig: Option<CameraRigComp>,
    pub follow_controller: Option<FollowTargetCameraController>,
    pub follow_motor: Option<FollowTargetCameraMotor>,
    pub physics_body: Option<PhysicsBodyDesc>,
    pub bounds: Option<Bounds>,
    pub display_visibility: Option<DisplayVisibility>,
    pub player_commands: Option<PlayerCommandFrame>,
    pub player_ground: Option<PlayerGroundState>,
    pub player_locomotion: Option<PlayerLocomotionState>,
    pub player_stance: Option<PlayerStanceState>,
    pub weapon_tuning: Option<HitscanWeaponTuning>,
    pub weapon_state: Option<PlayerWeaponState>,
    pub interaction_tuning: Option<PlayerInteractionTuning>,
    pub health: Option<Health>,
    pub physics_surface: Option<PhysicsSurface>,
    pub interactable: Option<Interactable>,
    pub inventory: Option<PlayerInventory>,
    pub equipped_weapon: Option<EquippedWeaponBinding>,
    pub item_pickup: Option<ItemPickup>,
    pub world_item_presentation: Option<WorldItemPresentation>,
    pub world_item_runtime: Option<WorldItemRuntime>,
    pub(crate) pending_hitscan: Option<PendingHitscan>,
    pub(crate) pending_interaction: Option<PendingInteraction>,
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeWorldSnapshot {
    pub entities: Vec<RuntimeEntitySnapshot>,
    pub player_events: Option<PlayerEventBus>,
    pub weapon_events: Option<WeaponEventBus>,
    pub interaction_events: Option<InteractionEventBus>,
    pub inventory_events: Option<InventoryEventBus>,
    pub inventory_hud: Option<InventoryHudState>,
    pub item_catalog: Option<ItemCatalog>,
    pub loadout_catalog: Option<InventoryLoadoutCatalog>,
}

#[inline]
pub fn capture_runtime_world_snapshot(world: &World) -> RuntimeWorldSnapshot {
    let mut entities: Vec<RuntimeEntitySnapshot> = world
        .iter_entities()
        .map(|entity| RuntimeEntitySnapshot {
            entity,
            transform: world.get::<Transform>(entity).copied(),
            velocity: world.get::<Velocity>(entity).copied(),
            angular_velocity: world.get::<AngularVelocity>(entity).copied(),
            motor_input: world.get::<MotorInput>(entity).copied(),
            character_motor: world.get::<CharacterMotor>(entity).copied(),
            camera_rig: world.get::<CameraRigComp>(entity).copied(),
            follow_controller: world.get::<FollowTargetCameraController>(entity).copied(),
            follow_motor: world.get::<FollowTargetCameraMotor>(entity).copied(),
            physics_body: world.get::<PhysicsBodyDesc>(entity).copied(),
            bounds: world.get::<Bounds>(entity).copied(),
            display_visibility: world.get::<DisplayVisibility>(entity).copied(),
            player_commands: world.get::<PlayerCommandFrame>(entity).copied(),
            player_ground: world.get::<PlayerGroundState>(entity).copied(),
            player_locomotion: world.get::<PlayerLocomotionState>(entity).copied(),
            player_stance: world.get::<PlayerStanceState>(entity).copied(),
            weapon_tuning: world.get::<HitscanWeaponTuning>(entity).copied(),
            weapon_state: world.get::<PlayerWeaponState>(entity).copied(),
            interaction_tuning: world.get::<PlayerInteractionTuning>(entity).copied(),
            health: world.get::<Health>(entity).copied(),
            physics_surface: world.get::<PhysicsSurface>(entity).cloned(),
            interactable: world.get::<Interactable>(entity).cloned(),
            inventory: world.get::<PlayerInventory>(entity).cloned(),
            equipped_weapon: world.get::<EquippedWeaponBinding>(entity).copied(),
            item_pickup: world.get::<ItemPickup>(entity).copied(),
            world_item_presentation: world.get::<WorldItemPresentation>(entity).cloned(),
            world_item_runtime: world.get::<WorldItemRuntime>(entity).copied(),
            pending_hitscan: world.get::<PendingHitscan>(entity).copied(),
            pending_interaction: world.get::<PendingInteraction>(entity).copied(),
        })
        .collect();
    entities.sort_by_key(|it| it.entity.stable_u64());
    RuntimeWorldSnapshot {
        entities,
        player_events: world.resource::<PlayerEventBus>().cloned(),
        weapon_events: world.resource::<WeaponEventBus>().cloned(),
        interaction_events: world.resource::<InteractionEventBus>().cloned(),
        inventory_events: world.resource::<InventoryEventBus>().cloned(),
        inventory_hud: world.resource::<InventoryHudState>().cloned(),
        item_catalog: world.resource::<ItemCatalog>().cloned(),
        loadout_catalog: world.resource::<InventoryLoadoutCatalog>().cloned(),
    }
}

#[inline]
fn restore_component_opt<T: Component + Copy>(
    world: &mut World,
    entity: EntityId,
    value: Option<T>,
) {
    if let Some(v) = value {
        let _ = world.insert(entity, v);
    } else {
        let _ = world.remove::<T>(entity);
    }
}

#[inline]
fn restore_component_clone<T: Component + Clone>(
    world: &mut World,
    entity: EntityId,
    value: Option<T>,
) {
    if let Some(value) = value {
        let _ = world.insert(entity, value);
    } else {
        let _ = world.remove::<T>(entity);
    }
}

#[inline]
fn restore_resource_clone<T: Clone + Send + Sync + 'static>(world: &mut World, value: Option<T>) {
    if let Some(value) = value {
        world.insert_resource(value);
    } else {
        let _ = world.remove_resource::<T>();
    }
}

#[inline]
pub fn restore_runtime_world_snapshot(world: &mut World, snapshot: RuntimeWorldSnapshot) {
    let RuntimeWorldSnapshot {
        entities,
        player_events,
        weapon_events,
        interaction_events,
        inventory_events,
        inventory_hud,
        item_catalog,
        loadout_catalog,
    } = snapshot;
    let live_ids: Vec<EntityId> = world.iter_entities().collect();
    let original_ids: FxHashSet<EntityId> = entities.iter().map(|it| it.entity).collect();

    for entity in live_ids {
        if !original_ids.contains(&entity) {
            let _ = world.despawn(entity);
        }
    }

    for entry in entities {
        if !world.exists(entry.entity) {
            continue;
        }

        restore_component_opt(world, entry.entity, entry.transform);
        restore_component_opt(world, entry.entity, entry.velocity);
        restore_component_opt(world, entry.entity, entry.angular_velocity);
        restore_component_opt(world, entry.entity, entry.motor_input);
        restore_component_opt(world, entry.entity, entry.character_motor);
        restore_component_opt(world, entry.entity, entry.camera_rig);
        restore_component_opt(world, entry.entity, entry.follow_controller);
        restore_component_opt(world, entry.entity, entry.follow_motor);
        restore_component_opt(world, entry.entity, entry.physics_body);
        restore_component_opt(world, entry.entity, entry.bounds);
        restore_component_opt(world, entry.entity, entry.display_visibility);
        restore_component_opt(world, entry.entity, entry.player_commands);
        restore_component_opt(world, entry.entity, entry.player_ground);
        restore_component_opt(world, entry.entity, entry.player_locomotion);
        restore_component_opt(world, entry.entity, entry.player_stance);
        restore_component_opt(world, entry.entity, entry.weapon_tuning);
        restore_component_opt(world, entry.entity, entry.weapon_state);
        restore_component_opt(world, entry.entity, entry.interaction_tuning);
        restore_component_opt(world, entry.entity, entry.health);
        restore_component_clone(world, entry.entity, entry.physics_surface);
        restore_component_clone(world, entry.entity, entry.interactable);
        restore_component_clone(world, entry.entity, entry.inventory);
        restore_component_opt(world, entry.entity, entry.equipped_weapon);
        restore_component_opt(world, entry.entity, entry.item_pickup);
        restore_component_clone(world, entry.entity, entry.world_item_presentation);
        restore_component_opt(world, entry.entity, entry.world_item_runtime);
        restore_component_opt(world, entry.entity, entry.pending_hitscan);
        restore_component_opt(world, entry.entity, entry.pending_interaction);
    }

    restore_resource_clone(world, player_events);
    restore_resource_clone(world, weapon_events);
    restore_resource_clone(world, interaction_events);
    restore_resource_clone(world, inventory_events);
    restore_resource_clone(world, inventory_hud);
    restore_resource_clone(world, item_catalog);
    restore_resource_clone(world, loadout_catalog);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::{
        apply_player_stance_geometry, default_medkit_item_id, default_rifle_ammo_id,
        inventory_quantity, remove_item, spawn_default_player, spawn_persistent_item_pickup,
        FpsPlayerTuning, PlayerStanceKind,
    };
    use newengine_math::Vec3;

    #[test]
    fn runtime_snapshot_restores_stance_bounds_weapon_and_health_state() {
        let mut world = World::new();
        let tuning = FpsPlayerTuning::default().sanitized();
        let player = spawn_default_player(
            &mut world,
            None,
            "snapshot-player",
            Vec3::new(0.0, tuning.body_half_height + tuning.body_radius, 0.0),
        );
        let source_pickup = spawn_persistent_item_pickup(
            &mut world,
            None,
            default_rifle_ammo_id(),
            4,
            Vec3::new(3.0, 1.0, 0.0),
            "snapshot.pickup.ammo",
            5.0,
        )
        .expect("persistent pickup");
        let medkit = default_medkit_item_id();
        let medkits_before = inventory_quantity(&world, player, medkit);
        let before = capture_runtime_world_snapshot(&world);
        let standing_y = world
            .get::<Transform>(player)
            .expect("transform")
            .position
            .y;

        apply_player_stance_geometry(&mut world, player, PlayerStanceKind::Crouched, tuning, 5);
        if let Some(weapon) = world.get_mut::<PlayerWeaponState>(player) {
            weapon.ammo_in_magazine = 1;
            weapon.reserve_ammo = 0;
        }
        if let Some(health) = world.get_mut::<Health>(player) {
            health.current = 7.0;
        }
        if let Some(hud) = world.resource_mut::<InventoryHudState>() {
            hud.open = true;
        }
        remove_item(&mut world, player, medkit, medkits_before).expect("remove medkits");
        assert_eq!(inventory_quantity(&world, player, medkit), 0);
        assert!(crate::gameplay::inventory::try_collect_item_pickup(
            &mut world,
            player,
            source_pickup,
        ));
        assert_eq!(
            world
                .get::<DisplayVisibility>(source_pickup)
                .expect("dormant visibility")
                .mode,
            crate::gameplay::DisplayMode::RuntimeHidden
        );

        restore_runtime_world_snapshot(&mut world, before);

        assert_eq!(
            world
                .get::<PlayerStanceState>(player)
                .expect("stance")
                .current,
            PlayerStanceKind::Standing
        );
        assert!(
            (world
                .get::<Transform>(player)
                .expect("transform")
                .position
                .y
                - standing_y)
                .abs()
                < 1.0e-6
        );
        assert_eq!(
            world
                .get::<PlayerWeaponState>(player)
                .expect("weapon")
                .ammo_in_magazine,
            HitscanWeaponTuning::default().magazine_capacity
        );
        assert_eq!(world.get::<Health>(player).expect("health").current, 100.0);
        assert_eq!(inventory_quantity(&world, player, medkit), medkits_before);
        assert!(world.get::<EquippedWeaponBinding>(player).is_some());
        assert!(
            !world
                .resource::<InventoryHudState>()
                .expect("inventory HUD")
                .open
        );
        assert!(world.exists(source_pickup));
        assert_eq!(
            world
                .get::<ItemPickup>(source_pickup)
                .expect("restored pickup")
                .quantity,
            4
        );
        assert_eq!(
            world
                .get::<DisplayVisibility>(source_pickup)
                .expect("restored pickup visibility")
                .mode,
            crate::gameplay::DisplayMode::Both
        );
        assert!(world.get::<PhysicsBodyDesc>(source_pickup).is_some());
        assert!(world.get::<WorldItemPresentation>(source_pickup).is_some());
        assert!(world.get::<WorldItemRuntime>(source_pickup).is_some());
        let bounds = world.get::<Bounds>(player).expect("bounds");
        assert!(
            (bounds.local_aabb.half_extents().y - (tuning.body_half_height + tuning.body_radius))
                .abs()
                < 1.0e-6
        );
    }
}
