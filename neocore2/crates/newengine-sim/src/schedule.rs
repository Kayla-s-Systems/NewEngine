#![forbid(unsafe_op_in_unsafe_fn)]

use core::cmp::Ordering;

use newengine_ecs::World;

use crate::{access::AccessMask, commands::CommandBuffer, systems, SimFrame};

/// Simulation stages.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SimStage {
    /// Inputs are produced externally (winit/plugin) and written into components/resources.
    Input = 0,
    /// Controllers translate inputs to desired motion / camera.
    Controllers = 1,
    /// Kinematic integration / physics.
    Physics = 2,
    /// Derived world state (transforms, bounds, scene caches).
    Derived = 3,
}

impl SimStage {
    pub const COUNT: usize = 4;

    #[inline]
    pub const fn as_usize(self) -> usize {
        self as usize
    }
}

/// System function signature.
///
/// Systems must be deterministic and side-effect free outside of the provided command buffer.
pub type SystemFn = fn(&World, SimFrame, &mut CommandBuffer);

#[derive(Clone, Copy)]
struct SystemEntry {
    order: i32,
    seq: u32,
    name: &'static str,
    access: AccessMask,
    f: SystemFn,
}

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
        self.sort_if_needed();

        let systems = &self.stages[stage.as_usize()];
        if systems.is_empty() {
            return;
        }

        #[cfg(feature = "parallel")]
        {
            run_stage_parallel(world, systems, frame);
            return;
        }

        #[cfg(not(feature = "parallel"))]
        {
            run_stage_single_thread(world, systems, frame);
        }
    }

    #[inline]
    pub fn run_default_pipeline(&mut self, world: &mut World, frame: SimFrame) {
        self.run_stage(world, SimStage::Input, frame);
        self.run_stage(world, SimStage::Controllers, frame);
        self.run_stage(world, SimStage::Physics, frame);
        self.run_stage(world, SimStage::Derived, frame);
    }
}

#[inline]
fn run_stage_single_thread(world: &mut World, systems: &[SystemEntry], frame: SimFrame) {
    for s in systems {
        #[cfg(debug_assertions)]
        {
            // Keep metadata alive for debugging/profiling builds.
            let _ = (s.name, s.access);
        }
        let mut cb = CommandBuffer::new();
        (s.f)(world, frame, &mut cb);
        if !cb.is_empty() {
            cb.apply_all(world);
        }
    }
}

#[cfg(feature = "parallel")]
fn run_stage_parallel(world: &mut World, systems: &[SystemEntry], frame: SimFrame) {
    use std::sync::mpsc;

    let batches = build_batches(systems);

    for batch in batches {
        // World snapshot for this batch.
        // Systems are required to only read from `world` and write to their command buffers.
        let wref: &World = world;

        let (tx, rx) = mpsc::channel::<((i32, u32), CommandBuffer)>();

        rayon::scope(|scope| {
            for sys in batch {
                let tx = tx.clone();
                scope.spawn(move |_| {
                    let mut cb = CommandBuffer::new();
                    (sys.f)(wref, frame, &mut cb);
                    let _ = tx.send(((sys.order, sys.seq), cb));
                });
            }
        });

        drop(tx);

        let mut collected: Vec<((i32, u32), CommandBuffer)> = rx.into_iter().collect();
        collected.sort_by(|a, b| a.0.0.cmp(&b.0.0).then(a.0.1.cmp(&b.0.1)));

        for (_key, cb) in collected {
            if !cb.is_empty() {
                cb.apply_all(world);
            }
        }
    }
}

#[cfg(feature = "parallel")]
fn build_batches<'a>(systems: &'a [SystemEntry]) -> Vec<Vec<&'a SystemEntry>> {
    let mut batches: Vec<Vec<&'a SystemEntry>> = Vec::new();
    let mut masks: Vec<AccessMask> = Vec::new();

    'sys: for sys in systems {
        for (i, m) in masks.iter_mut().enumerate() {
            if !m.conflicts(sys.access) {
                *m = m.union(sys.access);
                batches[i].push(sys);
                continue 'sys;
            }
        }

        batches.push(vec![sys]);
        masks.push(sys.access);
    }

    batches
}

/// A production-lean default schedule.
///
/// You can extend it with gameplay systems without forking the engine.
#[inline]
pub fn default_schedule() -> SimSchedule {
    let mut s = SimSchedule::new();

    // Controllers.
    s.add_system(
        SimStage::Controllers,
        10,
        "character_motor",
        AccessMask::write(crate::Subsystem::Gameplay as u32),
        systems::sys_character_motor,
    );
    s.add_system(
        SimStage::Controllers,
        20,
        "orbit_camera",
        AccessMask::write(crate::Subsystem::Camera as u32),
        systems::sys_orbit_camera,
    );
    // Depends on orbit_camera -> same subsystem => serialized.
    s.add_system(
        SimStage::Controllers,
        30,
        "camera_rig_to_transform",
        AccessMask::write(crate::Subsystem::Camera as u32),
        systems::sys_camera_rig_to_transform,
    );

    // Physics.
    s.add_system(
        SimStage::Physics,
        10,
        "integrate_velocities",
        AccessMask::write(crate::Subsystem::Gameplay as u32),
        systems::sys_integrate_velocities,
    );

    // Derived.
    s.add_system(
        SimStage::Derived,
        10,
        "scene_derived",
        AccessMask::write(crate::Subsystem::Scene as u32),
        systems::sys_scene_derived,
    );

    s
}
