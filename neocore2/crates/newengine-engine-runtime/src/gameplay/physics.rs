use newengine_ecs::World;
use newengine_physics_contracts::PhysicsWorldDesc;
use newengine_physics_runtime::{PhysicsWorldService, PhysicsWorldStepSettings};

use super::FpsDemoRules;

#[inline]
pub(super) fn step_runtime_physics(world: &mut World, dt: f32) {
    let player_tuning = world
        .resource::<FpsDemoRules>()
        .map(|rules| rules.player.sanitized())
        .unwrap_or_default();

    if world.resource::<PhysicsWorldService>().is_none() {
        world.insert_resource(PhysicsWorldService::new(PhysicsWorldDesc::default()));
    }

    let Some(mut service) = world.remove_resource::<PhysicsWorldService>() else {
        return;
    };

    let report = service.step(
        world,
        dt,
        PhysicsWorldStepSettings {
            gravity: player_tuning.gravity,
            contact_skin: player_tuning.contact_skin,
        },
    );

    if report.active_bodies > 128 || report.contacts > 32 {
        log::debug!(
            "physics step: tick={} bodies={} static={} dynamic={} contacts={} dt_ms={:.3}",
            report.fixed_tick,
            report.active_bodies,
            report.static_bodies,
            report.dynamic_bodies,
            report.contacts,
            report.dt * 1000.0,
        );
    }

    world.insert_resource(service);
}
