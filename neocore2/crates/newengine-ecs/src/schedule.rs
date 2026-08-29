#![forbid(unsafe_op_in_unsafe_fn)]

use crate::{Commands, World};

/// High-level deterministic stages for a classic game frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    Input,
    PreSim,
    FixedSim,
    PostSim,
    Transform,
    RenderSync,
}

/// Minimal runtime context for systems.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameCtx {
    pub dt: f32,
    pub fixed_dt: f32,
    pub fixed_alpha: f32,
    pub is_fixed: bool,
    pub frame_index: u64,
    pub fixed_tick: u64,
}

pub trait System: Send {
    fn name(&self) -> &'static str {
        "system"
    }

    fn stage(&self) -> Stage;

    fn run(&mut self, world: &mut World, commands: &mut Commands, frame: FrameCtx);
}

struct Entry {
    stage: Stage,
    order: i32,
    seq: u64,
    sys: Box<dyn System>,
}

/// Deterministic system schedule.
///
/// Ordering is by `(stage, order, insertion_seq)`.
///
/// Notes:
/// - `insertion_seq` is a monotonic u64; we use saturating increment to guarantee stability even
///   under pathological long-running sessions.
/// - This schedule is intentionally minimal: no parallelism and no implicit dependencies.
///   Higher-level executors can be built on top while keeping the ordering contract.
pub struct Schedule {
    entries: Vec<Entry>,
    next_seq: u64,
}

impl Default for Schedule {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Schedule {
    #[inline]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_seq: 1,
        }
    }

    /// Adds a system with an explicit order within its stage.
    #[inline]
    pub fn add(&mut self, sys: Box<dyn System>, order: i32) {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1).max(1);

        let stage = sys.stage();
        self.entries.push(Entry {
            stage,
            order,
            seq,
            sys,
        });

        self.entries
            .sort_by(|a, b| (a.stage, a.order, a.seq).cmp(&(b.stage, b.order, b.seq)));
    }

    /// Runs a single stage.
    pub fn run_stage(
        &mut self,
        stage: Stage,
        world: &mut World,
        commands: &mut Commands,
        frame: FrameCtx,
    ) {
        for e in self.entries.iter_mut().filter(|e| e.stage == stage) {
            e.sys.run(world, commands, frame);
        }
    }

    /// Runs all stages in canonical order.
    pub fn run_all(&mut self, world: &mut World, commands: &mut Commands, frame: FrameCtx) {
        self.run_stage(Stage::Input, world, commands, frame);
        self.run_stage(Stage::PreSim, world, commands, frame);
        if frame.is_fixed {
            self.run_stage(Stage::FixedSim, world, commands, frame);
        }
        self.run_stage(Stage::PostSim, world, commands, frame);
        self.run_stage(Stage::Transform, world, commands, frame);
        self.run_stage(Stage::RenderSync, world, commands, frame);
    }
}
