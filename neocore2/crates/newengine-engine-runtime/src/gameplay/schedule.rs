use newengine_ecs::World;
use newengine_sim::{default_schedule, SimFrame, SimSchedule, SimStage};

use super::fps_demo::step_fps_demo_gameplay;
use super::physics::step_service_physics;
use newengine_core::physics::PhysicsApiRef;

use crate::authority::{current_entity_authority_map, current_world_authority_frame};

#[inline]
pub fn run_schedule(
    schedule: &mut SimSchedule,
    world: &mut World,
    dt: f32,
    physics_api: Option<&PhysicsApiRef>,
) {
    let frame = SimFrame::new(dt.max(0.0001), 0);

    if let Some(authority) = current_world_authority_frame(world) {
        let provider_entities = current_entity_authority_map(world)
            .map(|map| map.native_to_provider.len())
            .unwrap_or(0);
        log::trace!(
            "gameplay schedule: world authority mode='{}' owner='{}' native_entities={} provider_entities={} source='authority-frame'",
            authority.mode.as_str(),
            authority.route_snapshot.authority_label(),
            authority.native_entity_count,
            provider_entities
        );
    }

    // Service-backed physics owns integration for PhysicsBodyDesc entities.
    // Do not run the default in-process SimStage::Physics velocity integrator
    // here, otherwise controlled characters are moved once by ECS and again by
    // the backend provider.
    schedule.run_stage(world, SimStage::Input, frame);
    schedule.run_stage(world, SimStage::Controllers, frame);
    schedule.run_stage(world, SimStage::ApplyIntents, frame);
    step_service_physics(world, frame.dt, physics_api);
    schedule.run_stage(world, SimStage::Derived, frame);

    step_fps_demo_gameplay(world, frame.dt);
}

#[inline]
pub fn default_sim_schedule() -> SimSchedule {
    default_schedule()
}
