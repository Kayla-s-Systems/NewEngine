use newengine_ecs::World;
use newengine_sim::{default_schedule, SimFrame, SimSchedule, SimStage, SimulationJobTelemetry};

use super::fps_demo::step_fps_demo_gameplay;
use super::physics::step_service_physics;
use newengine_core::physics::PhysicsApiRef;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicsIntegrationMode {
    ServiceBackend,
    EcsFallback,
}

pub fn run_schedule(
    schedule: &mut SimSchedule,
    world: &mut World,
    dt: f32,
    physics_api: Option<&PhysicsApiRef>,
) {
    run_schedule_with_physics_mode(
        schedule,
        world,
        dt,
        physics_api,
        PhysicsIntegrationMode::ServiceBackend,
    );
}

pub fn run_schedule_with_physics_mode(
    schedule: &mut SimSchedule,
    world: &mut World,
    dt: f32,
    physics_api: Option<&PhysicsApiRef>,
    physics_mode: PhysicsIntegrationMode,
) {
    run_schedule_with_physics_mode_and_telemetry(schedule, world, dt, physics_api, physics_mode, None);
}

pub fn run_schedule_with_physics_mode_and_telemetry(
    schedule: &mut SimSchedule,
    world: &mut World,
    dt: f32,
    physics_api: Option<&PhysicsApiRef>,
    physics_mode: PhysicsIntegrationMode,
    telemetry: Option<&SimulationJobTelemetry<'_>>,
) {
    let frame = SimFrame::new(dt.max(0.0001), 0);

    schedule.run_stage_with_telemetry(world, SimStage::Input, frame, telemetry);
    schedule.run_stage_with_telemetry(world, SimStage::Controllers, frame, telemetry);
    schedule.run_stage_with_telemetry(world, SimStage::ApplyIntents, frame, telemetry);
    match physics_mode {
        PhysicsIntegrationMode::ServiceBackend => {
            // Service-backed physics owns integration for PhysicsBodyDesc entities.
            // Do not run the default in-process SimStage::Physics velocity integrator
            // here, otherwise controlled characters are moved once by ECS and again by
            // the backend provider.
            step_service_physics(world, frame.dt, physics_api);
        }
        PhysicsIntegrationMode::EcsFallback => {
            // Declarative safe-profile fallback: keep gameplay controls responsive
            // without entering the native physics provider path. This is a capability
            // downgrade, not a game-specific shortcut.
            schedule.run_stage_with_telemetry(world, SimStage::Physics, frame, telemetry);
        }
    }
    schedule.run_stage_with_telemetry(world, SimStage::Derived, frame, telemetry);

    step_fps_demo_gameplay(world, frame.dt);
}

#[inline]
pub fn default_sim_schedule() -> SimSchedule {
    default_schedule()
}
