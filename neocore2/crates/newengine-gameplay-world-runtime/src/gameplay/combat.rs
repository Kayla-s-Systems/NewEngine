use newengine_ecs::World;

#[path = "combat/types.rs"]
mod types;

pub use types::{
    BallisticShotProfile, CombatActuationState, HitscanWeaponTuning, Interactable,
    InteractionEvent, InteractionEventBus, PendingHitscan, PendingInteraction,
    PlayerInteractionTuning, PlayerWeaponState, WeaponAccuracyModifiers, WeaponAccuracyState,
    WeaponActionKind, WeaponActionRuntime, WeaponActionTimingSource, WeaponAttackKind, WeaponEvent,
    WeaponEventBus, WeaponEventKind, WeaponFireControllerState, WeaponObstructionState,
    WeaponReloadAnimationAuthority, WeaponReloadAnimationMarker, WeaponReloadAnimationMarkerInbox,
    WeaponReloadPhase, WEAPON_RELOAD_ANIMATION_REQUIRED_MARKER_MASK,
    WEAPON_RELOAD_MARKER_AMMO_COMMITTED, WEAPON_RELOAD_MARKER_CHAMBERED,
    WEAPON_RELOAD_MARKER_COMPLETE, WEAPON_RELOAD_MARKER_MAGAZINE_DETACHED,
    WEAPON_RELOAD_MARKER_MAGAZINE_INSERTED,
};

pub fn drain_weapon_events(world: &mut World) -> Vec<WeaponEvent> {
    world
        .resource_mut::<WeaponEventBus>()
        .map(WeaponEventBus::drain)
        .unwrap_or_default()
}

pub fn drain_interaction_events(world: &mut World) -> Vec<InteractionEvent> {
    world
        .resource_mut::<InteractionEventBus>()
        .map(InteractionEventBus::drain)
        .unwrap_or_default()
}

pub fn queue_weapon_reload_animation_marker(
    world: &mut World,
    owner: newengine_ecs::EntityId,
    marker: WeaponReloadAnimationMarker,
) {
    if let Some(inbox) = world.get_mut::<WeaponReloadAnimationMarkerInbox>(owner) {
        inbox.push(marker);
        return;
    }
    let mut inbox = WeaponReloadAnimationMarkerInbox::default();
    inbox.push(marker);
    let _ = world.insert(owner, inbox);
}

pub fn drain_weapon_reload_animation_markers(
    world: &mut World,
    owner: newengine_ecs::EntityId,
    weapon_instance_id: crate::gameplay::ItemInstanceId,
) -> Vec<WeaponReloadAnimationMarker> {
    world
        .get_mut::<WeaponReloadAnimationMarkerInbox>(owner)
        .map(|inbox| inbox.drain_for_instance(weapon_instance_id))
        .unwrap_or_default()
}
