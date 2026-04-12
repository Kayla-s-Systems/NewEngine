#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;
use newengine_math::{EulerRot, Quat, Vec2, Vec3};

use crate::{CharacterMotor, ControllerCtx, Intent, IntentSink, MotorInput, Velocity};

#[derive(Clone, Copy, Debug)]
pub struct CharacterMotorStep {
    pub motor: CharacterMotor,
    pub rotation: Quat,
    pub velocity_ws: Vec3,
}

#[inline]
fn sanitize_dt(dt: f32) -> Option<f32> {
    if !dt.is_finite() || dt <= 0.0 {
        return None;
    }
    // Prevent numerical blowups on stalls/breakpoints.
    Some(dt.min(0.1))
}

#[inline]
fn sanitize_speed_mul(m: f32) -> f32 {
    if m.is_finite() && m > 0.0 {
        m
    } else {
        1.0
    }
}

#[inline]
fn sanitize_vec2(v: Vec2) -> Vec2 {
    Vec2::new(
        if v.x.is_finite() { v.x } else { 0.0 },
        if v.y.is_finite() { v.y } else { 0.0 },
    )
}

#[inline]
fn sanitize_vec3(v: Vec3) -> Vec3 {
    Vec3::new(
        if v.x.is_finite() { v.x } else { 0.0 },
        if v.y.is_finite() { v.y } else { 0.0 },
        if v.z.is_finite() { v.z } else { 0.0 },
    )
}

/// Deterministic character motor step (pure math, no ECS).
///
/// Inputs:
/// - `motor`: current motor state.
/// - `input`: sampled input for this tick.
/// - `current_rot`: current entity rotation (used to convert local move axis to world).
/// - `dt`: delta time.
///
/// Output:
/// - updated motor
/// - desired new rotation (yaw/pitch)
/// - desired world-space velocity
#[inline]
pub fn step_character_motor(
    motor: CharacterMotor,
    input: MotorInput,
    current_rot: Quat,
    dt: f32,
) -> Option<CharacterMotorStep> {
    let dt = sanitize_dt(dt)?;
    let mut motor = motor;

    // Look.
    if input.look_active {
        let d = sanitize_vec2(input.look_delta);
        // dt is intentionally NOT applied to mouse delta (it's already "per-frame").
        motor.yaw = motor.yaw + d.x * motor.look_sens;
        motor.pitch = motor.pitch + d.y * motor.look_sens;
    }

    let pitch_limit = if motor.pitch_limit.is_finite() && motor.pitch_limit > 0.0 {
        motor.pitch_limit
    } else {
        1.54
    };
    motor.pitch = motor.pitch.clamp(-pitch_limit, pitch_limit);

    // Orientation from yaw/pitch.
    let rotation = Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0);

    // Move axis -> world velocity.
    let speed_mul = sanitize_speed_mul(input.speed_mul);
    let move_axis = sanitize_vec3(input.move_axis);

    // Convention: controller space uses x=right, y=up, z=forward.
    // Engine/world forward can be configured per motor via `forward_sign_z`.
    let forward_sign = if motor.forward_sign_z.is_finite() && motor.forward_sign_z != 0.0 {
        motor.forward_sign_z.signum()
    } else {
        -1.0
    };

    let local = Vec3::new(move_axis.x, move_axis.y, move_axis.z * forward_sign);

    let len = local.length();
    let velocity_ws = if len > 1e-6 {
        let dir = local / len;
        let move_speed = if motor.move_speed.is_finite() && motor.move_speed >= 0.0 {
            motor.move_speed
        } else {
            0.0
        };
        (current_rot * dir) * (move_speed * speed_mul)
    } else {
        Vec3::ZERO
    };

    // dt currently only used for clamping / future accel integration; keep to avoid API churn.
    let _ = dt;

    Some(CharacterMotorStep {
        motor,
        rotation,
        velocity_ws,
    })
}

/// ECS-agnostic controller runner producing semantic intents.
#[inline]
pub fn run_character_motor_controller(
    entity: EntityId,
    ctx: &ControllerCtx<'_>,
    motor: CharacterMotor,
    input: MotorInput,
    out: &mut impl IntentSink,
) {
    let Some(step) = step_character_motor(motor, input, ctx.local_rotation_or_identity(entity), ctx.dt()) else {
        return;
    };

    if ctx.has_transform(entity) {
        out.emit(Intent::TransformSetLocalRotation {
            entity,
            rotation: step.rotation,
        });
    }

    let mut velocity_ws = step.velocity_ws;
    if let Some(current_velocity) = ctx.world().get::<Velocity>(entity).copied() {
        velocity_ws.y = current_velocity.0.y;
    }

    out.emit(Intent::SetVelocity {
        entity,
        value: Velocity(velocity_ws),
    });
    out.emit(Intent::SetCharacterMotor {
        entity,
        value: step.motor,
    });
}
