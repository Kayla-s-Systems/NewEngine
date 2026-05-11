use newengine_ecs::World;
use newengine_sim::{default_schedule, SimFrame, SimSchedule};

use super::fps_demo::step_fps_demo_gameplay;
use super::physics::step_runtime_physics;

#[inline]
pub fn run_schedule(schedule: &mut SimSchedule, world: &mut World, dt: f32) {
    let frame = SimFrame::new(dt.max(0.0001), 0);
    schedule.run_default_pipeline(world, frame);
    step_runtime_physics(world, frame.dt);
    step_fps_demo_gameplay(world, frame.dt);
}

#[inline]
pub fn default_sim_schedule() -> SimSchedule {
    default_schedule()
}
