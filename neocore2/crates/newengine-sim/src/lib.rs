#![forbid(unsafe_op_in_unsafe_fn)]

//! Game-ready simulation layer on top of `newengine-ecs`.
//!
//! This crate intentionally stays renderer/editor agnostic.

use core::cmp::Ordering;

use glam::{EulerRot, Quat, Vec2, Vec3};
use hashbrown::HashMap;

use newengine_camera::{CameraInput, CameraRig, OrbitController};
use newengine_ecs::{EntityId, World};
use newengine_scene::update_scene_world;
use newengine_transform::Transform;

// -----------------------------------------------------------------------------
// Time
// -----------------------------------------------------------------------------

/// Simulation frame data.
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

// -----------------------------------------------------------------------------
// Components
// -----------------------------------------------------------------------------

/// Linear velocity in world space (units/sec).
#[derive(Clone, Copy, Debug, Default)]
pub struct Velocity(pub Vec3);

/// Angular velocity in local space (rad/sec).
///
/// Conventions:
/// - x: pitch rate
/// - y: yaw rate
/// - z: roll rate
#[derive(Clone, Copy, Debug, Default)]
pub struct AngularVelocity(pub Vec3);

/// Input state for entity-local controllers (typically written by input/plugins).
#[derive(Clone, Copy, Debug, Default)]
pub struct MotorInput {
    /// Generic movement axes.
    /// Convention: x=right, y=up, z=forward.
    pub move_axis: Vec3,
    /// Look delta (mouse, stick).
    pub look_delta: Vec2,
    /// Whether look should affect yaw/pitch.
    pub look_active: bool,
    /// Additional speed multiplier (shift/sprint).
    pub speed_mul: f32,
    /// Mouse wheel / zoom delta.
    pub zoom_delta: f32,
}

/// FPS / Free-fly style motor.
///
/// This is meant to be a small, deterministic building block.
/// Character collision/physics should live in a separate plugin/system.
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

/// ECS-bridge for `newengine-camera` orbit controller.
///
/// The camera crate is pure math; this component wires it to ECS entities.
#[derive(Clone, Copy, Debug)]
pub struct OrbitCameraMotor {
    pub controller: OrbitController,
}

impl Default for OrbitCameraMotor {
    #[inline]
    fn default() -> Self {
        Self {
            controller: OrbitController::default(),
        }
    }
}

/// Camera rig stored as a component.
#[derive(Clone, Copy, Debug, Default)]
pub struct CameraRigComp(pub CameraRig);

/// Camera input stored as a component (written by input/editor).
#[derive(Clone, Copy, Debug, Default)]
pub struct CameraInputComp(pub CameraInput);

// -----------------------------------------------------------------------------
// Schedule
// -----------------------------------------------------------------------------

/// Deterministic simulation stages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SimStage {
    /// Inputs are produced externally (winit/plugin) and written into components/resources.
    Input,
    /// Controllers translate inputs to desired motion / camera.
    Controllers,
    /// Kinematic integration / physics.
    Physics,
    /// Derived world state (transforms, bounds, scene caches).
    Derived,
}

pub type SystemFn = fn(&mut World, SimFrame);

#[derive(Clone, Copy)]
struct SystemEntry {
    order: i32,
    name: &'static str,
    f: SystemFn,
}

/// A minimal deterministic scheduler.
///
/// - stable ordering by (order, name)
/// - no dynamic dispatch in the hot loop (plain fn pointers)
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
        self.stages.entry(stage).or_default().push(SystemEntry { order, name, f });
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

/// A production-lean default schedule.
///
/// You can extend it with gameplay systems without forking the engine.
#[inline]
pub fn default_schedule() -> SimSchedule {
    let mut s = SimSchedule::new();

    // Controllers.
    s.add_system(SimStage::Controllers, 10, "character_motor", sys_character_motor);
    s.add_system(SimStage::Controllers, 20, "orbit_camera", sys_orbit_camera);
    s.add_system(SimStage::Controllers, 30, "camera_rig_to_transform", sys_camera_rig_to_transform);

    // Physics.
    s.add_system(SimStage::Physics, 10, "integrate_velocities", sys_integrate_velocities);

    // Derived.
    s.add_system(SimStage::Derived, 10, "scene_derived", sys_scene_derived);
    s
}

// -----------------------------------------------------------------------------
// Systems
// -----------------------------------------------------------------------------

/// Applies `MotorInput` to `CharacterMotor` and updates `Transform`/`Velocity`.
pub fn sys_character_motor(world: &mut World, frame: SimFrame) {
    let dt = frame.dt;
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }

    let ids: Vec<EntityId> = world.query2_ids::<CharacterMotor, MotorInput>().collect();
    for id in ids {
        // Update motor + transform in-place (no remove/insert copy churn).
        // Velocity is written as a separate component (insert/replace).
        let mut out_vel = None;

        world.with3_mut::<CharacterMotor, MotorInput, Transform, _>(id, |motor_opt, input_opt, tr_opt| {
            let (Some(motor), Some(input), Some(t)) = (motor_opt, input_opt.copied(), tr_opt) else {
                return;
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

            // Update orientation.
            t.rotation = Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0);

            // Convert input axes to world velocity. Convention: forward is -Z.
            let local = Vec3::new(input.move_axis.x, input.move_axis.y, -input.move_axis.z);
            let len = local.length();
            let vel = if len > 1e-6 {
                let dir = local / len;
                (t.rotation * dir) * (motor.move_speed * speed_mul)
            } else {
                Vec3::ZERO
            };

            out_vel = Some(Velocity(vel));
        });

        if let Some(v) = out_vel {
            let _ = world.insert(id, v);
        }
    }
}

/// Applies orbit controller input to `CameraRigComp`.
pub fn sys_orbit_camera(world: &mut World, frame: SimFrame) {
    let dt = frame.dt;
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }

    let ids: Vec<EntityId> = world
        .query2_ids::<OrbitCameraMotor, CameraRigComp>()
        .collect();

    for id in ids {
        let input = world.get::<CameraInputComp>(id).map(|c| c.0).unwrap_or_default();

        // In-place update (no remove/insert).
        world.with2_mut::<OrbitCameraMotor, CameraRigComp, _>(id, |motor_opt, rig_opt| {
            let (Some(motor), Some(rig)) = (motor_opt, rig_opt) else { return; };
            motor.controller.apply(&mut rig.0, input, dt);
        });
    }
}

/// Copies `CameraRigComp`/// Copies `CameraRigComp` to `Transform`.
pub fn sys_camera_rig_to_transform(world: &mut World, _frame: SimFrame) {
    let ids: Vec<EntityId> = world.query2_ids::<CameraRigComp, Transform>().collect();
    for id in ids {
        let Some(rig) = world.get::<CameraRigComp>(id).copied() else { continue; };
        if let Some(t) = world.get_mut::<Transform>(id) {
            t.position = rig.0.position;
            t.rotation = rig.0.rotation;
        }
    }
}

/// Integrates velocities into transforms.
pub fn sys_integrate_velocities(world: &mut World, frame: SimFrame) {
    let dt = frame.dt;
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }

    // Translation.
    let ids: Vec<EntityId> = world.query2_ids::<Transform, Velocity>().collect();
    for id in ids {
        let Some(v) = world.get::<Velocity>(id).copied() else { continue; };
        if let Some(t) = world.get_mut::<Transform>(id) {
            t.position += v.0 * dt;
        }
    }

    // Rotation.
    let ids: Vec<EntityId> = world.query2_ids::<Transform, AngularVelocity>().collect();
    for id in ids {
        let Some(w) = world.get::<AngularVelocity>(id).copied() else { continue; };
        if let Some(t) = world.get_mut::<Transform>(id) {
            let d = w.0 * dt;
            if d.is_finite() && d.length_squared() > 1e-12 {
                let dq = Quat::from_euler(EulerRot::YXZ, d.y, d.x, d.z);
                t.rotation = (t.rotation * dq).normalize();
            }
        }
    }
}

/// Updates derived scene data (world pose, bounds, cached scene bounds).
pub fn sys_scene_derived(world: &mut World, _frame: SimFrame) {
    update_scene_world(world);
}
