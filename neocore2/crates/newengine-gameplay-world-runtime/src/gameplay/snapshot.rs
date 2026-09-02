use newengine_audio_api::{
    AcousticSurface, AudioAmbienceBed, AudioEmitter, AudioEnvironmentZone, AudioPortal,
};
use newengine_audio_world_api::{
    AudioAmbienceBedRuntime, AudioEmitterRuntime, AudioOcclusionObservation,
};
use newengine_bounds::Bounds;
use newengine_ecs::{Component, EntityId, World};
use newengine_math::collections::FxHashSet;
use newengine_sim::{
    AngularVelocity, CameraRigComp, CharacterFacingTurnStepRequest, CharacterMotor,
    FollowTargetCameraController, FollowTargetCameraMotor, MotorInput, Velocity,
};
use newengine_transform::Transform;

use super::combat::{PendingHitscan, PendingInteraction};
use super::inventory::{
    EquippedWeaponBinding, InventoryEventBus, InventoryLoadoutCatalog, ItemCatalog, ItemPickup,
    PlayerInventory, WorldItemPresentation, WorldItemRuntime,
};
use super::{
    AIController, AINavigationState, AINavigationTuning, AIPatrolRoute, AIPatrolState,
    AIPerceptionProbe, CharacterControlState, CharacterDamageResponseTuning, CharacterDeathPolicy,
    CharacterDeathTransitionState, CharacterExertionState, CharacterHitReactionState,
    CharacterInjuryState, CharacterLifeState, CombatActuationState, CombatIntent, CombatTeam,
    DisplayVisibility, GameplayCapabilityBus, GameplayEventBus, GameplayModalState, Health,
    HitscanWeaponTuning, Interactable, InteractionEventBus, PerceptionState, PerceptionTuning,
    PhysicsBodyDesc, PhysicsSurface, PlayerCommandFrame, PlayerEventBus, PlayerFallState,
    PlayerGroundState, PlayerInteractionTuning, PlayerLandingState, PlayerLocomotionState,
    PlayerStanceState, PlayerWeaponState, Stamina, StaminaTuning, TargetMemory, WeaponEventBus,
};

#[derive(Clone, Debug)]
pub struct RuntimeEntitySnapshot {
    pub entity: EntityId,
    pub transform: Option<Transform>,
    pub audio_emitter: Option<AudioEmitter>,
    pub acoustic_surface: Option<AcousticSurface>,
    pub audio_environment_zone: Option<AudioEnvironmentZone>,
    pub audio_portal: Option<AudioPortal>,
    pub audio_ambience_bed: Option<AudioAmbienceBed>,
    pub velocity: Option<Velocity>,
    pub angular_velocity: Option<AngularVelocity>,
    pub motor_input: Option<MotorInput>,
    pub character_motor: Option<CharacterMotor>,
    pub character_facing_turn: Option<CharacterFacingTurnStepRequest>,
    pub camera_rig: Option<CameraRigComp>,
    pub follow_controller: Option<FollowTargetCameraController>,
    pub follow_motor: Option<FollowTargetCameraMotor>,
    pub physics_body: Option<PhysicsBodyDesc>,
    pub bounds: Option<Bounds>,
    pub display_visibility: Option<DisplayVisibility>,
    pub player_commands: Option<PlayerCommandFrame>,
    pub player_ground: Option<PlayerGroundState>,
    pub player_locomotion: Option<PlayerLocomotionState>,
    pub player_fall: Option<PlayerFallState>,
    pub player_landing: Option<PlayerLandingState>,
    pub player_stance: Option<PlayerStanceState>,
    pub weapon_tuning: Option<HitscanWeaponTuning>,
    pub weapon_state: Option<PlayerWeaponState>,
    pub interaction_tuning: Option<PlayerInteractionTuning>,
    pub health: Option<Health>,
    pub life_state: Option<CharacterLifeState>,
    pub character_control: Option<CharacterControlState>,
    pub damage_response_tuning: Option<CharacterDamageResponseTuning>,
    pub hit_reaction: Option<CharacterHitReactionState>,
    pub injury_state: Option<CharacterInjuryState>,
    pub death_policy: Option<CharacterDeathPolicy>,
    pub death_transition: Option<CharacterDeathTransitionState>,
    pub stamina: Option<Stamina>,
    pub stamina_tuning: Option<StaminaTuning>,
    pub exertion: Option<CharacterExertionState>,
    pub combat_team: Option<CombatTeam>,
    pub ai_controller: Option<AIController>,
    pub perception_tuning: Option<PerceptionTuning>,
    pub perception_state: Option<PerceptionState>,
    pub target_memory: Option<TargetMemory>,
    pub combat_intent: Option<CombatIntent>,
    pub ai_perception_probe: Option<AIPerceptionProbe>,
    pub ai_navigation_tuning: Option<AINavigationTuning>,
    pub ai_navigation_state: Option<AINavigationState>,
    pub ai_patrol_route: Option<AIPatrolRoute>,
    pub ai_patrol_state: Option<AIPatrolState>,
    pub combat_actuation: Option<CombatActuationState>,
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
    pub gameplay_capabilities: Option<GameplayCapabilityBus>,
    pub gameplay_events: Option<GameplayEventBus>,
    pub player_events: Option<PlayerEventBus>,
    pub weapon_events: Option<WeaponEventBus>,
    pub interaction_events: Option<InteractionEventBus>,
    pub inventory_events: Option<InventoryEventBus>,
    pub gameplay_modal: Option<GameplayModalState>,
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
            audio_emitter: world.get::<AudioEmitter>(entity).cloned(),
            acoustic_surface: world.get::<AcousticSurface>(entity).cloned(),
            audio_environment_zone: world.get::<AudioEnvironmentZone>(entity).cloned(),
            audio_portal: world.get::<AudioPortal>(entity).cloned(),
            audio_ambience_bed: world.get::<AudioAmbienceBed>(entity).cloned(),
            velocity: world.get::<Velocity>(entity).copied(),
            angular_velocity: world.get::<AngularVelocity>(entity).copied(),
            motor_input: world.get::<MotorInput>(entity).copied(),
            character_motor: world.get::<CharacterMotor>(entity).copied(),
            character_facing_turn: world.get::<CharacterFacingTurnStepRequest>(entity).copied(),
            camera_rig: world.get::<CameraRigComp>(entity).copied(),
            follow_controller: world.get::<FollowTargetCameraController>(entity).copied(),
            follow_motor: world.get::<FollowTargetCameraMotor>(entity).copied(),
            physics_body: world.get::<PhysicsBodyDesc>(entity).copied(),
            bounds: world.get::<Bounds>(entity).copied(),
            display_visibility: world.get::<DisplayVisibility>(entity).copied(),
            player_commands: world.get::<PlayerCommandFrame>(entity).cloned(),
            player_ground: world.get::<PlayerGroundState>(entity).copied(),
            player_locomotion: world.get::<PlayerLocomotionState>(entity).copied(),
            player_fall: world.get::<PlayerFallState>(entity).copied(),
            player_landing: world.get::<PlayerLandingState>(entity).copied(),
            player_stance: world.get::<PlayerStanceState>(entity).copied(),
            weapon_tuning: world.get::<HitscanWeaponTuning>(entity).copied(),
            weapon_state: world.get::<PlayerWeaponState>(entity).copied(),
            interaction_tuning: world.get::<PlayerInteractionTuning>(entity).copied(),
            health: world.get::<Health>(entity).copied(),
            life_state: world.get::<CharacterLifeState>(entity).copied(),
            character_control: world.get::<CharacterControlState>(entity).copied(),
            damage_response_tuning: world.get::<CharacterDamageResponseTuning>(entity).copied(),
            hit_reaction: world.get::<CharacterHitReactionState>(entity).cloned(),
            injury_state: world.get::<CharacterInjuryState>(entity).copied(),
            death_policy: world.get::<CharacterDeathPolicy>(entity).copied(),
            death_transition: world.get::<CharacterDeathTransitionState>(entity).cloned(),
            stamina: world.get::<Stamina>(entity).copied(),
            stamina_tuning: world.get::<StaminaTuning>(entity).copied(),
            exertion: world.get::<CharacterExertionState>(entity).copied(),
            combat_team: world.get::<CombatTeam>(entity).copied(),
            ai_controller: world.get::<AIController>(entity).copied(),
            perception_tuning: world.get::<PerceptionTuning>(entity).copied(),
            perception_state: world.get::<PerceptionState>(entity).copied(),
            target_memory: world.get::<TargetMemory>(entity).copied(),
            combat_intent: world.get::<CombatIntent>(entity).copied(),
            ai_perception_probe: world.get::<AIPerceptionProbe>(entity).copied(),
            ai_navigation_tuning: world.get::<AINavigationTuning>(entity).copied(),
            ai_navigation_state: world.get::<AINavigationState>(entity).cloned(),
            ai_patrol_route: world.get::<AIPatrolRoute>(entity).cloned(),
            ai_patrol_state: world.get::<AIPatrolState>(entity).copied(),
            combat_actuation: world.get::<CombatActuationState>(entity).copied(),
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
        gameplay_capabilities: world.resource::<GameplayCapabilityBus>().cloned(),
        gameplay_events: world.resource::<GameplayEventBus>().cloned(),
        player_events: world.resource::<PlayerEventBus>().cloned(),
        weapon_events: world.resource::<WeaponEventBus>().cloned(),
        interaction_events: world.resource::<InteractionEventBus>().cloned(),
        inventory_events: world.resource::<InventoryEventBus>().cloned(),

        gameplay_modal: world.resource::<GameplayModalState>().copied(),
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
        gameplay_capabilities,
        gameplay_events,
        player_events,
        weapon_events,
        interaction_events,
        inventory_events,
        gameplay_modal,
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
        restore_component_clone(world, entry.entity, entry.audio_emitter);
        restore_component_clone(world, entry.entity, entry.acoustic_surface);
        restore_component_clone(world, entry.entity, entry.audio_environment_zone);
        restore_component_clone(world, entry.entity, entry.audio_portal);
        restore_component_clone(world, entry.entity, entry.audio_ambience_bed);
        let _ = world.remove::<AudioAmbienceBedRuntime>(entry.entity);
        let _ = world.remove::<AudioEmitterRuntime>(entry.entity);
        let _ = world.remove::<AudioOcclusionObservation>(entry.entity);
        restore_component_opt(world, entry.entity, entry.velocity);
        restore_component_opt(world, entry.entity, entry.angular_velocity);
        restore_component_opt(world, entry.entity, entry.motor_input);
        restore_component_opt(world, entry.entity, entry.character_motor);
        restore_component_opt(world, entry.entity, entry.character_facing_turn);
        restore_component_opt(world, entry.entity, entry.camera_rig);
        restore_component_opt(world, entry.entity, entry.follow_controller);
        restore_component_opt(world, entry.entity, entry.follow_motor);
        restore_component_opt(world, entry.entity, entry.physics_body);
        restore_component_opt(world, entry.entity, entry.bounds);
        restore_component_opt(world, entry.entity, entry.display_visibility);
        restore_component_clone(world, entry.entity, entry.player_commands);
        restore_component_opt(world, entry.entity, entry.player_ground);
        restore_component_opt(world, entry.entity, entry.player_locomotion);
        restore_component_opt(world, entry.entity, entry.player_fall);
        restore_component_opt(world, entry.entity, entry.player_landing);
        restore_component_opt(world, entry.entity, entry.player_stance);
        restore_component_opt(world, entry.entity, entry.weapon_tuning);
        restore_component_opt(world, entry.entity, entry.weapon_state);
        restore_component_opt(world, entry.entity, entry.interaction_tuning);
        restore_component_opt(world, entry.entity, entry.health);
        restore_component_opt(world, entry.entity, entry.life_state);
        restore_component_opt(world, entry.entity, entry.character_control);
        restore_component_opt(world, entry.entity, entry.damage_response_tuning);
        restore_component_clone(world, entry.entity, entry.hit_reaction);
        restore_component_opt(world, entry.entity, entry.injury_state);
        restore_component_opt(world, entry.entity, entry.death_policy);
        restore_component_clone(world, entry.entity, entry.death_transition);
        restore_component_opt(world, entry.entity, entry.stamina);
        restore_component_opt(world, entry.entity, entry.stamina_tuning);
        restore_component_opt(world, entry.entity, entry.exertion);
        restore_component_opt(world, entry.entity, entry.combat_team);
        restore_component_opt(world, entry.entity, entry.ai_controller);
        restore_component_opt(world, entry.entity, entry.perception_tuning);
        restore_component_opt(world, entry.entity, entry.perception_state);
        restore_component_opt(world, entry.entity, entry.target_memory);
        restore_component_opt(world, entry.entity, entry.combat_intent);
        restore_component_opt(world, entry.entity, entry.ai_perception_probe);
        restore_component_opt(world, entry.entity, entry.ai_navigation_tuning);
        restore_component_clone(world, entry.entity, entry.ai_navigation_state);
        restore_component_opt(world, entry.entity, entry.combat_actuation);
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

    restore_resource_clone(world, gameplay_capabilities);
    restore_resource_clone(world, gameplay_events);
    restore_resource_clone(world, player_events);
    restore_resource_clone(world, weapon_events);
    restore_resource_clone(world, interaction_events);
    restore_resource_clone(world, inventory_events);
    restore_resource_clone(world, gameplay_modal);
    restore_resource_clone(world, item_catalog);
    restore_resource_clone(world, loadout_catalog);
}

#[cfg(test)]
mod tests {
    include!("snapshot/tests.rs");
}
