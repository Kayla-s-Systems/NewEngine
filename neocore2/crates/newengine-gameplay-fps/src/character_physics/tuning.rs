use newengine_ecs::World;
use newengine_engine_runtime::gameplay::PhysicsWorldSettings;
use newengine_gameplay_fps_api::{FpsDemoRules, FpsPlayerTuning};

#[inline]
pub(super) fn tuning(world: &World) -> FpsPlayerTuning {
    world
        .resource::<FpsDemoRules>()
        .map(|rules| rules.player.sanitized())
        .unwrap_or_else(|| FpsPlayerTuning::default().sanitized())
}

/// Projects FPS-authored gravity/contact policy into the provider-neutral physics world resource.
/// The engine physics bridge consumes only `PhysicsWorldSettings` and never reads FPS rules.
pub(crate) fn sync_physics_world_settings(world: &mut World) {
    let tuning = tuning(world);
    world.insert_resource(
        PhysicsWorldSettings {
            gravity: tuning.gravity,
            contact_skin: tuning.contact_skin,
            ..PhysicsWorldSettings::default()
        }
        .sanitized(),
    );
}
