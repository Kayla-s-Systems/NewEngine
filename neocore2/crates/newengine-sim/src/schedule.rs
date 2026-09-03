#![forbid(unsafe_op_in_unsafe_fn)]

mod execution;
mod planning;
mod types;

pub use types::*;

use core::cmp::Ordering;

use newengine_ecs::World;

use crate::{
    access::{AccessDomain, AccessMask},
    systems, SimFrame,
};

use execution::{run_stage_parallel, run_stage_single_thread};
#[cfg(test)]
use planning::plan_conflict_free_batches;

/// A minimal deterministic scheduler.
///
/// - stable ordering by `(order, seq)`
/// - deterministic parallel batching by access mask
pub struct SimSchedule {
    stages: [Vec<SystemEntry>; SimStage::COUNT],
    is_sorted: [bool; SimStage::COUNT],
    next_seq: u32,
}

impl Default for SimSchedule {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl SimSchedule {
    #[inline]
    pub fn new() -> Self {
        Self {
            stages: core::array::from_fn(|_| Vec::new()),
            is_sorted: [false; SimStage::COUNT],
            next_seq: 1,
        }
    }

    #[inline]
    pub fn add_system(
        &mut self,
        stage: SimStage,
        order: i32,
        name: &'static str,
        access: AccessMask,
        f: SystemFn,
    ) {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1).max(1);

        let idx = stage.as_usize();
        self.stages[idx].push(SystemEntry {
            order,
            seq,
            name,
            access,
            f,
        });
        self.is_sorted[idx] = false;
    }

    #[inline]
    fn sort_if_needed(&mut self) {
        for (i, v) in self.stages.iter_mut().enumerate() {
            if self.is_sorted[i] {
                continue;
            }
            v.sort_unstable_by(|a, b| match a.order.cmp(&b.order) {
                Ordering::Equal => a.seq.cmp(&b.seq),
                o => o,
            });
            self.is_sorted[i] = true;
        }
    }

    #[inline]
    pub fn run_stage(&mut self, world: &mut World, stage: SimStage, frame: SimFrame) {
        self.run_stage_with_telemetry_and_executor(world, stage, frame, None, None);
    }

    #[inline]
    pub fn run_stage_with_telemetry(
        &mut self,
        world: &mut World,
        stage: SimStage,
        frame: SimFrame,
        telemetry: Option<&SimulationJobTelemetry<'_>>,
    ) {
        self.run_stage_with_telemetry_and_executor(world, stage, frame, telemetry, None);
    }

    pub fn run_stage_with_telemetry_and_executor(
        &mut self,
        world: &mut World,
        stage: SimStage,
        frame: SimFrame,
        telemetry: Option<&SimulationJobTelemetry<'_>>,
        executor: Option<&dyn SimReadBatchExecutor>,
    ) {
        self.sort_if_needed();

        let systems = &self.stages[stage.as_usize()];
        if systems.is_empty() {
            return;
        }

        run_stage_single_thread(world, stage, systems, frame, telemetry, executor);
    }

    /// Executes conflict-free system batches through the host executor. Systems
    /// that conflict according to `AccessMask` are separated by an owner-thread
    /// commit barrier, so later conflicting systems observe earlier writes exactly
    /// as they did in the serial scheduler.
    pub fn run_stage_with_parallel_executor(
        &mut self,
        world: &mut World,
        stage: SimStage,
        frame: SimFrame,
        telemetry: Option<&SimulationJobTelemetry<'_>>,
        executor: &dyn SimSystemBatchExecutor,
    ) -> Vec<SimBatchDiagnostics> {
        self.sort_if_needed();

        let systems = &self.stages[stage.as_usize()];
        if systems.is_empty() {
            return Vec::new();
        }

        run_stage_parallel(world, stage, systems, frame, telemetry, executor)
    }

    #[inline]
    pub fn run_default_pipeline(&mut self, world: &mut World, frame: SimFrame) {
        self.run_default_pipeline_with_telemetry(world, frame, None);
    }

    pub fn run_default_pipeline_with_telemetry(
        &mut self,
        world: &mut World,
        frame: SimFrame,
        telemetry: Option<&SimulationJobTelemetry<'_>>,
    ) {
        self.run_stage_with_telemetry(world, SimStage::Input, frame, telemetry);
        self.run_stage_with_telemetry(world, SimStage::Controllers, frame, telemetry);
        self.run_stage_with_telemetry(world, SimStage::ApplyIntents, frame, telemetry);
        self.run_stage_with_telemetry(world, SimStage::Physics, frame, telemetry);
        self.run_stage_with_telemetry(world, SimStage::Derived, frame, telemetry);
    }
}

/// A production-lean default schedule.
///
/// You can extend it with gameplay systems without forking the engine.
#[inline]
pub fn default_schedule() -> SimSchedule {
    let mut s = SimSchedule::new();

    // Controllers emit intents only.
    s.add_system(
        SimStage::Controllers,
        10,
        "character_motor",
        AccessMask::write_domain(AccessDomain::CharacterControl)
            .union(AccessMask::read_domain(AccessDomain::CharacterInput)),
        systems::sys_character_motor,
    );
    s.add_system(
        SimStage::Controllers,
        20,
        "orbit_camera",
        AccessMask::write_domain(AccessDomain::CameraControl)
            .union(AccessMask::read_domain(AccessDomain::CameraInput))
            .union(AccessMask::read_domain(AccessDomain::CameraRig)),
        systems::sys_orbit_camera,
    );
    s.add_system(
        SimStage::Controllers,
        25,
        "camera_follow",
        AccessMask::write_domain(AccessDomain::CameraControl)
            .union(AccessMask::read_domain(AccessDomain::CameraRig))
            .union(AccessMask::read_domain(AccessDomain::FollowTarget))
            .union(AccessMask::read_domain(AccessDomain::Transform)),
        systems::sys_camera_follow,
    );

    // Single ordered apply stage.
    s.add_system(
        SimStage::ApplyIntents,
        10,
        "apply_controller_intents",
        AccessMask::write_domain(AccessDomain::CharacterControl)
            .union(AccessMask::write_domain(AccessDomain::CameraControl))
            .union(AccessMask::write_domain(AccessDomain::ControllerIntents)),
        systems::sys_apply_controller_intents,
    );
    s.add_system(
        SimStage::ApplyIntents,
        20,
        "camera_rig_to_transform",
        AccessMask::read_domain(AccessDomain::CameraRig)
            .union(AccessMask::write_domain(AccessDomain::Transform)),
        systems::sys_camera_rig_to_transform,
    );

    // Physics.
    s.add_system(
        SimStage::Physics,
        10,
        "integrate_velocities",
        AccessMask::read_domain(AccessDomain::Velocity)
            .union(AccessMask::write_domain(AccessDomain::Transform))
            .union(AccessMask::write_domain(AccessDomain::PhysicsState)),
        systems::sys_integrate_velocities,
    );

    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandBuffer;
    use std::sync::Arc;

    struct AppendLog(&'static str);

    impl crate::Command for AppendLog {
        fn apply(self: Box<Self>, world: &mut World) {
            world
                .resource_mut::<Vec<&'static str>>()
                .expect("commit log resource missing")
                .push(self.0);
        }
    }

    fn log_a(_world: &World, _frame: SimFrame, commands: &mut CommandBuffer) {
        commands.push(Box::new(AppendLog("a")));
    }

    fn log_b(_world: &World, _frame: SimFrame, commands: &mut CommandBuffer) {
        commands.push(Box::new(AppendLog("b")));
    }

    struct ReverseResultExecutor;

    impl SimSystemBatchExecutor for ReverseResultExecutor {
        fn run_system_batch(
            &self,
            _batch: &SimulationJobBatch,
            world: Arc<World>,
            frame: SimFrame,
            systems: Vec<SimSystemJob>,
        ) -> SimSystemBatchResult {
            let mut results = systems
                .into_iter()
                .map(|system| {
                    let mut commands = CommandBuffer::new();
                    (system.function)(world.as_ref(), frame, &mut commands);
                    SimSystemCommandBatch::new(system.system_index, system.name, commands)
                })
                .collect::<Vec<_>>();
            results.reverse();
            SimSystemBatchResult::new(results, 100, 180)
        }
    }

    #[test]
    fn parallel_results_commit_in_stable_system_order_even_if_workers_finish_reversed() {
        let mut schedule = SimSchedule::new();
        schedule.add_system(SimStage::Derived, 10, "log_a", AccessMask::write(0), log_a);
        schedule.add_system(SimStage::Derived, 20, "log_b", AccessMask::write(1), log_b);

        let mut world = World::new();
        world.insert_resource(Vec::<&'static str>::new());
        let diagnostics = schedule.run_stage_with_parallel_executor(
            &mut world,
            SimStage::Derived,
            SimFrame::new(0.016, 9),
            None,
            &ReverseResultExecutor,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].batch_width, 2);
        assert_eq!(diagnostics[0].worker_wall_time_ns, 100);
        assert_eq!(diagnostics[0].worker_cpu_time_ns, 180);
        assert!((diagnostics[0].parallel_efficiency_01 - 0.9).abs() < 0.001);
        assert_eq!(
            world
                .resource::<Vec<&'static str>>()
                .expect("commit log resource missing"),
            &vec!["a", "b"]
        );
    }

    #[test]
    fn character_turn_step_is_consumed_only_by_apply_intents_stage() {
        use crate::{
            CharacterFacingTurnStepRequest, CharacterMotor, ControllerIntentQueue, MotorInput,
        };
        use newengine_transform::Transform;

        let mut schedule = default_schedule();
        let mut world = World::new();
        let entity = world.spawn();
        let _ = world.insert(entity, Transform::default());
        let _ = world.insert(entity, CharacterMotor::default());
        let _ = world.insert(entity, MotorInput::default());
        let _ = world.insert(entity, CharacterFacingTurnStepRequest { yaw_delta: 0.25 });
        let frame = SimFrame::new(1.0 / 60.0, 1);

        schedule.run_stage(&mut world, SimStage::Controllers, frame);

        assert!(world
            .get::<CharacterFacingTurnStepRequest>(entity)
            .is_some());
        assert!(world
            .resource::<ControllerIntentQueue>()
            .is_some_and(|queue| !queue.is_empty()));

        schedule.run_stage(&mut world, SimStage::ApplyIntents, frame);

        assert!(world
            .get::<CharacterFacingTurnStepRequest>(entity)
            .is_none());
    }

    #[test]
    fn default_controller_stage_forms_access_mask_parallel_then_serial_batches() {
        let mut schedule = default_schedule();
        schedule.sort_if_needed();
        let systems = &schedule.stages[SimStage::Controllers.as_usize()];
        let batches = plan_conflict_free_batches(systems);

        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].indices, vec![0, 1]);
        assert_eq!(batches[1].indices, vec![2]);
        let conflict = batches[1]
            .conflict_before
            .as_ref()
            .expect("camera conflict diagnostic missing");
        assert_eq!(conflict.incoming_system, "camera_follow");
        assert_eq!(conflict.conflicting_systems, vec!["orbit_camera"]);
        assert_eq!(
            conflict.mask.write_write,
            AccessDomain::CameraControl.mask()
        );
        assert_eq!(conflict.mask.write_read, 0);
        assert_eq!(conflict.mask.read_write, 0);
        assert!(conflict
            .named_domains
            .contains(&"camera-control".to_owned()));
        assert!(!systems[0].access.conflicts(systems[1].access));
        assert!(systems[1].access.conflicts(systems[2].access));
    }
}
