use newengine_ecs::World;

#[path = "combat/types.rs"]
mod types;

pub use types::{
    Health, HitscanWeaponTuning, Interactable, InteractionEvent, InteractionEventBus,
    PendingHitscan, PendingInteraction, PlayerInteractionTuning, PlayerWeaponState, WeaponEvent,
    WeaponEventBus, WeaponEventKind, WeaponObstructionState,
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
