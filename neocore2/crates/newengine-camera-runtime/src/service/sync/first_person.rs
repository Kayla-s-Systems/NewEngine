#[inline]
fn first_person_horizontal_forward(rotation: Quat) -> Vec3 {
    let forward = (rotation.normalize_or_identity() * -Vec3::Z).normalize_or_zero();
    Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero()
}

#[inline]
fn first_person_position_contract(
    body_rotation: Quat,
    _camera_rotation: Quat,
    forward_clearance: f32,
) -> (Vec3, Vec3) {
    let mut body_forward = first_person_horizontal_forward(body_rotation);
    if body_forward.length_squared() <= 1.0e-8 {
        body_forward = Vec3::new(0.0, 0.0, -1.0);
    }

    // The gameplay eye is a rigid body-relative anchor. Mouse yaw/pitch must never translate it.
    // Previous centimetre-scale parallax was visually harmless in isolation, but in full-body FPP
    // it changed the direction fed into the self/body constraints and turned tiny look changes into
    // visible positional pops. Any head/weapon parallax belongs to presentation, not camera position.
    (body_forward * forward_clearance, Vec3::ZERO)
}

#[inline]
fn first_person_ads_position_contract(
    hip_camera_position: Vec3,
    ads_camera_position: Option<Vec3>,
    aim_alpha: f32,
) -> Vec3 {
    let aim_alpha = if aim_alpha.is_finite() {
        aim_alpha.clamp(0.0, 1.0)
    } else {
        0.0
    };
    ads_camera_position
        .filter(|position| position.is_finite())
        .map(|ads| hip_camera_position.lerp(ads, aim_alpha))
        .unwrap_or(hip_camera_position)
}

#[inline]
fn constrain_first_person_camera_position(
    player: EntityId,
    eye_center: Vec3,
    desired_camera_position: Vec3,
    spring_arm: CameraSpringArmConfig,
    collision_world: Option<&CameraSpringArmCollisionWorld>,
) -> Vec3 {
    let desired_offset_ws = desired_camera_position - eye_center;
    if !desired_offset_ws.is_finite() || desired_offset_ws.length_squared() <= 1.0e-10 {
        return eye_center;
    }
    // FPP uses the same collision scene as the third-person spring arm but with a head-sized
    // probe and no minimum arm length. The PlayerActor collider itself is ignored by the shared
    // constraint, so this prevents wall/ceiling penetration without treating the player's own
    // capsule as an obstacle.
    let constrained_offset_ws = constrain_spring_arm_offset_ls(
        player,
        eye_center,
        Quat::IDENTITY,
        desired_offset_ws,
        spring_arm,
        collision_world,
    );
    let constrained = eye_center + constrained_offset_ws;
    if constrained.is_finite() {
        constrained
    } else {
        eye_center
    }
}

fn stabilize_first_person_eye_anchor(
    current: Vec3,
    target: Vec3,
    grounded: bool,
    dt: f32,
    deadband_m: f32,
    time_constant_seconds: f32,
) -> Vec3 {
    if !target.is_finite() {
        return current;
    }
    if !current.is_finite() {
        return target;
    }

    // X/Z already come from render-cadence player presentation and must remain spatially exact.
    // Only grounded Y receives hysteresis because physics contact/grounding can oscillate by a few
    // millimetres even while the character is visually standing still.
    let mut next = Vec3::new(target.x, current.y, target.z);
    if !grounded {
        next.y = target.y;
        return next;
    }

    let deadband_m = if deadband_m.is_finite() {
        deadband_m.clamp(0.0, 0.25)
    } else {
        0.010
    };
    let time_constant_seconds = if time_constant_seconds.is_finite() {
        time_constant_seconds.clamp(0.001, 5.0)
    } else {
        0.060
    };
    let delta = target.y - current.y;
    if delta.abs() <= deadband_m {
        return next;
    }
    if !(dt.is_finite() && dt > 0.0) {
        next.y = target.y;
        return next;
    }
    let outside_deadband = delta - delta.signum() * deadband_m;
    let alpha = (1.0 - (-dt.min(0.05) / time_constant_seconds).exp()).clamp(0.0, 1.0);
    next.y = current.y + outside_deadband * alpha;
    next
}

#[derive(Clone, Copy, Debug, Default)]
struct FirstPersonAdditivePose {
    position_ls: Vec3,
    rotation_ls: Quat,
}

#[inline]
fn signed_sequence_noise(sequence: u64, salt: u64) -> f32 {
    let bits = (newengine_math::avalanche_u64(sequence ^ salt) >> 40) as u32 & 0x00ff_ffff;
    (bits as f32 / 0x00ff_ffffu32 as f32) * 2.0 - 1.0
}

fn step_first_person_additive_motion(
    state: &mut GameplayFirstPersonCameraState,
    input: FirstPersonPresentationInput,
    dt: f32,
    aim_response_hz: f32,
    camera_recoil_share: f32,
) -> FirstPersonAdditivePose {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.05)
    } else {
        0.0
    };
    let aim_target = if input.aiming { 1.0 } else { 0.0 };
    if dt > 0.0 {
        let aim_response_hz = if aim_response_hz.is_finite() {
            aim_response_hz.clamp(0.01, 120.0)
        } else {
            18.0
        };
        let alpha = 1.0 - (-aim_response_hz * dt).exp();
        state.aim_alpha =
            (state.aim_alpha + (aim_target - state.aim_alpha) * alpha).clamp(0.0, 1.0);
    } else {
        state.aim_alpha = aim_target;
    }

    if input.shot_sequence != state.last_shot_sequence {
        const PITCH_SALT: u64 = 0x243f_6a88_85a3_08d3;
        const YAW_SALT: u64 = 0x1319_8a2e_0370_7344;
        let pitch_base = input.recoil_pitch_radians.max(0.0);
        let pitch_random = input.recoil_pitch_random_radians.max(0.0);
        let yaw_random = input.recoil_yaw_radians.max(0.0);
        let yaw_bias = if input.recoil_yaw_bias_radians.is_finite() {
            input.recoil_yaw_bias_radians
        } else {
            0.0
        };
        let ads_multiplier = if input.ads_recoil_multiplier.is_finite() {
            input.ads_recoil_multiplier.clamp(0.0, 4.0)
        } else {
            1.0
        };
        let recoil_scale = 1.0 + (ads_multiplier - 1.0) * state.aim_alpha;
        // Camera recoil is intentionally separate from weapon-side recoil. The project camera
        // definition owns the visual share while the runtime owns impulse execution/recovery.
        let camera_recoil_share = if camera_recoil_share.is_finite() {
            camera_recoil_share.clamp(0.0, 2.0)
        } else {
            0.42
        };
        state.recoil_pitch_radians += (pitch_base
            + signed_sequence_noise(input.shot_sequence, PITCH_SALT) * pitch_random)
            .max(0.0)
            * recoil_scale
            * camera_recoil_share;
        state.recoil_yaw_radians += (yaw_bias
            + signed_sequence_noise(input.shot_sequence, YAW_SALT) * yaw_random)
            * recoil_scale
            * camera_recoil_share;
        state.last_shot_sequence = input.shot_sequence;
    }

    if dt > 0.0 {
        let recovery_hz = if input.recoil_recovery_hz.is_finite() {
            input.recoil_recovery_hz.clamp(0.05, 120.0)
        } else {
            7.5
        };
        let decay = (-recovery_hz * dt).exp();
        state.recoil_pitch_radians *= decay;
        state.recoil_yaw_radians *= decay;
    }
    state.recoil_pitch_radians = state.recoil_pitch_radians.clamp(0.0, 0.20);
    state.recoil_yaw_radians = state.recoil_yaw_radians.clamp(-0.12, 0.12);

    // Locomotion never modifies the gameplay camera transform. Full-body motion is already visible
    // on the animated body and weapon; duplicating gait as camera pitch/roll is perceived as
    // camera bounce and makes the hidden-head boundary easier to expose. Only authored recoil is
    // allowed to affect the first-person view rotation.

    FirstPersonAdditivePose {
        // Full-body FPP keeps the eye position locked to the stable render-cadence anchor.
        // Locomotion belongs to the body/weapon presentation; translating the camera itself makes
        // the body barrier amplify millimetre-scale bob into visible lateral/vertical jumps.
        position_ls: Vec3::ZERO,
        rotation_ls: (Quat::from_rotation_y(state.recoil_yaw_radians)
            * Quat::from_rotation_x(state.recoil_pitch_radians))
        .normalize_or_identity(),
    }
}
