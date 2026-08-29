#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::EntityId;
use newengine_math::{EulerRot, Quat, Vec2, Vec3};

use crate::{CharacterMotor, ControllerCtx, Intent, IntentSink, MotorInput, Velocity};

#[derive(Clone, Copy, Debug)]
pub struct CharacterMotorStep {
    pub motor: CharacterMotor,
    /// Visible/physical character facing. This is deliberately yaw-only and is
    /// independent from camera pitch/free-look yaw.
    pub body_rotation: Quat,
    /// Camera/view orientation derived from motor yaw + pitch.
    pub view_rotation: Quat,
    pub velocity_ws: Vec3,
}

#[inline]
fn sanitize_dt(dt: f32) -> Option<f32> {
    if !dt.is_finite() || dt <= 0.0 {
        return None;
    }
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

#[inline]
fn wrap_pi(mut angle: f32) -> f32 {
    const TAU: f32 = core::f32::consts::TAU;
    const PI: f32 = core::f32::consts::PI;
    if !angle.is_finite() {
        return 0.0;
    }
    angle = (angle + PI).rem_euclid(TAU) - PI;
    angle
}

#[inline]
fn move_toward_angle(current: f32, target: f32, max_delta: f32) -> f32 {
    let delta = wrap_pi(target - current);
    if delta.abs() <= max_delta {
        wrap_pi(target)
    } else {
        wrap_pi(current + delta.signum() * max_delta)
    }
}

#[inline]
fn yaw_from_forward(direction: Vec3) -> f32 {
    let dir = Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero();
    if dir.length_squared() <= 1.0e-12 {
        0.0
    } else {
        // Engine forward is -Z. This produces yaw such that
        // Quat::from_rotation_y(yaw) * -Vec3::Z == direction.
        (-dir.x).atan2(-dir.z)
    }
}

/// Deterministic character motor step (pure math, no ECS).
///
/// The motor owns **view yaw/pitch**. The entity transform owns **body yaw**.
/// Free look therefore never spins or pitches the player mesh. Locomotion turns
/// the body toward travel direction, while aim/lock-on can request `face_view`.
#[inline]
pub fn step_character_motor(
    motor: CharacterMotor,
    input: MotorInput,
    current_rot: Quat,
    dt: f32,
) -> Option<CharacterMotorStep> {
    let dt = sanitize_dt(dt)?;
    let mut motor = motor;

    // View look. Mouse delta is already per-frame and is intentionally not scaled by dt.
    if input.look_active {
        let d = sanitize_vec2(input.look_delta);
        motor.yaw = wrap_pi(motor.yaw + d.x * motor.look_sens);
        motor.pitch += d.y * motor.look_sens;
    }

    let pitch_limit = if motor.pitch_limit.is_finite() && motor.pitch_limit > 0.0 {
        motor.pitch_limit
    } else {
        1.54
    };
    motor.pitch = motor.pitch.clamp(-pitch_limit, pitch_limit);
    let view_rotation = Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0);

    // Move axis -> world velocity relative to view yaw only. Pitch never tilts locomotion.
    let speed_mul = sanitize_speed_mul(input.speed_mul);
    let move_axis = sanitize_vec3(input.move_axis);
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
        let locomotion_rotation = Quat::from_rotation_y(motor.yaw);
        (locomotion_rotation * dir) * (move_speed * speed_mul)
    } else {
        Vec3::ZERO
    };

    // Body facing is independent from view. While moving, face travel direction.
    // In explicit aim/lock-on mode, face the view yaw even while stationary.
    let (current_body_yaw, _, _) = current_rot.normalize_or_identity().to_euler(EulerRot::YXZ);
    let horizontal_velocity = Vec3::new(velocity_ws.x, 0.0, velocity_ws.z);
    let target_body_yaw = if input.face_view {
        motor.yaw
    } else if horizontal_velocity.length_squared() > 1.0e-8 {
        yaw_from_forward(horizontal_velocity)
    } else {
        current_body_yaw
    };
    let turn_speed = if motor.body_turn_speed.is_finite() && motor.body_turn_speed > 0.0 {
        motor.body_turn_speed
    } else {
        CharacterMotor::default().body_turn_speed
    };
    let body_yaw = move_toward_angle(current_body_yaw, target_body_yaw, turn_speed * dt);
    let body_rotation = Quat::from_rotation_y(body_yaw);

    Some(CharacterMotorStep {
        motor,
        body_rotation,
        view_rotation,
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
    let Some(step) = step_character_motor(
        motor,
        input,
        ctx.local_rotation_or_identity(entity),
        ctx.dt(),
    ) else {
        return;
    };

    if ctx.has_transform(entity) {
        out.emit(Intent::TransformSetLocalRotation {
            entity,
            rotation: step.body_rotation,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_locomotion_stays_horizontal_at_extreme_view_pitch() {
        let motor = CharacterMotor {
            pitch: 1.4,
            move_speed: 6.0,
            ..CharacterMotor::default()
        };
        let input = MotorInput {
            move_axis: Vec3::new(0.0, 0.0, 1.0),
            speed_mul: 1.0,
            ..MotorInput::default()
        };
        let pitched_rotation = Quat::from_euler(EulerRot::YXZ, 0.0, motor.pitch, 0.0);

        let step = step_character_motor(motor, input, pitched_rotation, 1.0 / 60.0)
            .expect("valid fixed-step motor update");

        assert!(step.velocity_ws.y.abs() <= 1.0e-6);
        assert!((step.velocity_ws.length() - 6.0).abs() <= 1.0e-5);
        let body_forward = step.body_rotation * -Vec3::Z;
        assert!(body_forward.y.abs() <= 1.0e-6);
    }

    #[test]
    fn diagonal_locomotion_is_normalized() {
        let motor = CharacterMotor {
            move_speed: 7.5,
            ..CharacterMotor::default()
        };
        let input = MotorInput {
            move_axis: Vec3::new(1.0, 0.0, 1.0),
            speed_mul: 1.0,
            ..MotorInput::default()
        };

        let step = step_character_motor(motor, input, Quat::IDENTITY, 1.0 / 60.0)
            .expect("valid fixed-step motor update");

        assert!((step.velocity_ws.length() - 7.5).abs() <= 1.0e-5);
        assert!(step.velocity_ws.y.abs() <= 1.0e-6);
    }

    #[test]
    fn free_look_changes_view_without_spinning_stationary_body() {
        let motor = CharacterMotor::default();
        let current_body = Quat::from_rotation_y(0.35);
        let input = MotorInput {
            look_delta: Vec2::new(120.0, -40.0),
            look_active: true,
            ..MotorInput::default()
        };

        let step =
            step_character_motor(motor, input, current_body, 1.0 / 60.0).expect("motor step");
        let (body_yaw, body_pitch, _) = step.body_rotation.to_euler(EulerRot::YXZ);
        assert!((body_yaw - 0.35).abs() <= 1.0e-5);
        assert!(body_pitch.abs() <= 1.0e-6);
        assert!(step.motor.yaw.abs() > 0.1);
        assert!(step.motor.pitch.abs() > 0.01);
    }

    #[test]
    fn moving_body_turns_toward_world_travel_direction() {
        let motor = CharacterMotor {
            body_turn_speed: 1000.0,
            ..CharacterMotor::default()
        };
        let input = MotorInput {
            move_axis: Vec3::new(1.0, 0.0, 0.0),
            speed_mul: 1.0,
            ..MotorInput::default()
        };
        let step =
            step_character_motor(motor, input, Quat::IDENTITY, 1.0 / 60.0).expect("motor step");
        let body_forward = (step.body_rotation * -Vec3::Z).normalize_or_zero();
        let travel = Vec3::new(step.velocity_ws.x, 0.0, step.velocity_ws.z).normalize_or_zero();
        assert!(body_forward.dot(travel) > 0.999);
    }

    #[test]
    fn aim_mode_turns_body_toward_view_with_bounded_rate() {
        let motor = CharacterMotor {
            yaw: 1.0,
            body_turn_speed: 2.0,
            ..CharacterMotor::default()
        };
        let input = MotorInput {
            face_view: true,
            ..MotorInput::default()
        };
        let step = step_character_motor(motor, input, Quat::IDENTITY, 0.1).expect("motor step");
        let (body_yaw, body_pitch, _) = step.body_rotation.to_euler(EulerRot::YXZ);
        assert!((body_yaw - 0.2).abs() <= 1.0e-4);
        assert!(body_pitch.abs() <= 1.0e-6);
    }
}
