#![forbid(unsafe_op_in_unsafe_fn)]

use core::cmp::Ordering;

use glam::{EulerRot, Quat, Vec2, Vec3};
use hashbrown::HashMap;
use newengine_ecs::{EntityId, World};
use newengine_scene::update_scene_world;
use newengine_transform::Transform;
use slotmap::Key;

mod physics;
pub use physics::*;

#[derive(Clone, Copy, Debug)]
pub struct SimFrame {
    pub dt: f32,
    pub fixed_tick: u64,
}

impl SimFrame {
    #[inline]
    pub fn new(dt: f32, fixed_tick: u64) -> Self {
        Self { dt, fixed_tick }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Velocity(pub Vec3);

#[derive(Clone, Copy, Debug, Default)]
pub struct AngularVelocity(pub Vec3);

#[derive(Clone, Copy, Debug, Default)]
pub struct MotorInput {
    pub move_axis: Vec3,
    pub look_delta: Vec2,
    pub look_active: bool,
    pub speed_mul: f32,
    pub zoom_delta: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct CharacterMotor {
    pub yaw: f32,
    pub pitch: f32,

    pub look_sens: f32,
    pub move_speed: f32,

    pub pitch_limit: f32,
}

impl Default for CharacterMotor {
    #[inline]
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            look_sens: 0.0025,
            move_speed: 6.0,
            pitch_limit: 1.54,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SimStage {
    Input,
    Controllers,
    Physics,
    Derived,
}

pub type SystemFn = fn(&mut World, SimFrame);

#[derive(Clone, Copy)]
struct SystemEntry {
    order: i32,
    name: &'static str,
    f: SystemFn,
}

pub struct SimSchedule {
    stages: HashMap<SimStage, Vec<SystemEntry>>,
    is_sorted: bool,
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
            stages: HashMap::new(),
            is_sorted: false,
        }
    }

    #[inline]
    pub fn add_system(&mut self, stage: SimStage, order: i32, name: &'static str, f: SystemFn) {
        self.stages
            .entry(stage)
            .or_default()
            .push(SystemEntry { order, name, f });
        self.is_sorted = false;
    }

    #[inline]
    pub fn sort_if_needed(&mut self) {
        if self.is_sorted {
            return;
        }
        for v in self.stages.values_mut() {
            v.sort_unstable_by(|a, b| match a.order.cmp(&b.order) {
                Ordering::Equal => a.name.cmp(b.name),
                o => o,
            });
        }
        self.is_sorted = true;
    }

    #[inline]
    pub fn run_stage(&mut self, world: &mut World, stage: SimStage, frame: SimFrame) {
        self.sort_if_needed();
        let Some(v) = self.stages.get(&stage) else { return; };
        for s in v {
            (s.f)(world, frame);
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
pub fn default_schedule() -> SimSchedule {
    let mut s = SimSchedule::new();

    s.add_system(SimStage::Controllers, 10, "character_motor", sys_character_motor);

    // Physics pipeline:
    // 1) Bake bodies from ECS authoring components.
    s.add_system(SimStage::Physics, 0, "physics_bake_bodies", physics_bake_bodies);
    // 2) Step Jolt.
    s.add_system(SimStage::Physics, 10, "physics_step_jolt", physics_step_jolt);
    // 3) Sync transforms (Dynamic: Jolt->ECS, Kinematic/Static: ECS->Jolt).
    s.add_system(
        SimStage::Physics,
        20,
        "physics_sync_transforms",
        physics_sync_transforms,
    );
    // 4) Cleanup orphans (despawn/remove components).
    s.add_system(
        SimStage::Physics,
        30,
        "physics_cleanup_bodies",
        physics_cleanup_bodies,
    );

    // Legacy integration only for non-physics entities.
    s.add_system(SimStage::Physics, 40, "integrate_velocities", sys_integrate_velocities);

    s.add_system(SimStage::Derived, 10, "scene_derived", sys_scene_derived);
    s
}

pub fn sys_character_motor(world: &mut World, frame: SimFrame) {
    let dt = frame.dt;
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }

    let mut ids = world.query2_ids::<CharacterMotor, MotorInput>();
    ids.sort_unstable_by_key(|&id| id.data().as_ffi());

    for id in ids {
        let Some(mut motor) = world.remove::<CharacterMotor>(id) else { continue; };
        let Some(input) = world.get::<MotorInput>(id).copied() else {
            let _ = world.insert(id, motor);
            continue;
        };

        let speed_mul = if input.speed_mul.is_finite() && input.speed_mul > 0.0 {
            input.speed_mul
        } else {
            1.0
        };

        if input.look_active {
            if input.look_delta.x.is_finite() {
                motor.yaw += input.look_delta.x * motor.look_sens;
            }
            if input.look_delta.y.is_finite() {
                motor.pitch += input.look_delta.y * motor.look_sens;
            }
        }
        motor.pitch = motor.pitch.clamp(-motor.pitch_limit, motor.pitch_limit);

        if let Some(t) = world.get_mut::<Transform>(id) {
            t.rotation = Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0);
        }

        let local = Vec3::new(input.move_axis.x, input.move_axis.y, -input.move_axis.z);
        let len = local.length();
        let vel = if len > 1e-6 {
            let dir = local / len;
            let rot = world
                .get::<Transform>(id)
                .map(|t| t.rotation)
                .unwrap_or(Quat::IDENTITY);
            (rot * dir) * (motor.move_speed * speed_mul)
        } else {
            Vec3::ZERO
        };

        let _ = world.insert(id, Velocity(vel));
        let _ = world.insert(id, motor);
    }
}

pub fn sys_integrate_velocities(world: &mut World, frame: SimFrame) {
    let dt = frame.dt;
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }

    let mut ids: Vec<EntityId> = world.query::<Transform>().map(|(id, _)| id).collect();
    ids.sort_unstable_by_key(|&id| id.data().as_ffi());

    for id in ids {
        // Physics-driven entities must not be integrated here.
        if world.has::<PhysicsBody>(id) {
            continue;
        }

        if let (Some(v), Some(t)) = (world.get::<Velocity>(id).copied(), world.get_mut::<Transform>(id)) {
            t.position += v.0 * dt;
        }

        if let (Some(w), Some(t)) =
            (world.get::<AngularVelocity>(id).copied(), world.get_mut::<Transform>(id))
        {
            let d = w.0 * dt;
            if d.is_finite() && d.length_squared() > 1e-12 {
                let dq = Quat::from_euler(EulerRot::YXZ, d.y, d.x, d.z);
                t.rotation = (t.rotation * dq).normalize();
            }
        }
    }
}

pub fn sys_scene_derived(world: &mut World, _frame: SimFrame) {
    update_scene_world(world);
}