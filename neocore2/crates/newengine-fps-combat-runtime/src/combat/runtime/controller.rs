use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
struct WeaponRecoilRuntime {
    weapon_instance_id: ItemInstanceId,
    applied_pitch_radians: f32,
    applied_yaw_radians: f32,
    pitch_speed_radians_per_second: f32,
    yaw_speed_radians_per_second: f32,
    recovery_hz: f32,
    hold_remaining_seconds: f32,
}

#[inline]
fn critically_damped_recoil_step(angle: f32, speed: f32, recovery_hz: f32, dt: f32) -> (f32, f32) {
    // NorthStar exposes both a recoil angle and a recoil-tracker speed. Model that contract as a
    // stable critically damped second-order tracker rather than subtracting a fixed exponential
    // fraction of the angle every frame. The analytic form is stable even on a long frame.
    let omega = recovery_hz.max(0.05) * 2.2;
    let c = speed + omega * angle;
    let decay = (-omega * dt).exp();
    let next_angle = (angle + c * dt) * decay;
    let next_speed = (speed - omega * c * dt) * decay;
    if next_angle.is_finite() && next_speed.is_finite() {
        (next_angle, next_speed)
    } else {
        (0.0, 0.0)
    }
}

pub(super) fn recover_weapon_accuracy(world: &mut World, player: EntityId, dt: f32) {
    let Some(binding) = active_equipped_weapon_binding(world, player) else {
        let _ = world.remove::<WeaponAccuracyState>(player);
        return;
    };
    let Some(firearm) = binding.weapon.firearm else {
        let _ = world.remove::<WeaponAccuracyState>(player);
        return;
    };
    let profiles = firearm.profiles.sanitized();
    let spread = profiles.spread;
    let resolved_stats = resolved_weapon_stats(world, player);
    let max_bloom = spread
        .hip
        .maximum_radians
        .into_iter()
        .chain(spread.ads.maximum_radians)
        .fold(0.0_f32, f32::max)
        * resolved_stats.spread_multiplier;
    let mut state = world
        .get::<WeaponAccuracyState>(player)
        .copied()
        .filter(|state| state.weapon_instance_id == binding.instance_id)
        .unwrap_or_else(|| WeaponAccuracyState::new(binding.instance_id));
    state.time_since_shot = (state.time_since_shot + dt.max(0.0)).min(60.0);
    if state.time_since_shot >= spread.recovery_delay_seconds && state.bloom_radians > 0.0 {
        let omega = spread.recovery_hz.max(0.05) * 2.0;
        let c = state.recovery_velocity + omega * state.bloom_radians;
        let decay = (-omega * dt.max(0.0)).exp();
        state.bloom_radians =
            ((state.bloom_radians + c * dt.max(0.0)) * decay).clamp(0.0, max_bloom);
        state.recovery_velocity = (state.recovery_velocity - omega * c * dt.max(0.0)) * decay;
        if state.bloom_radians < 1.0e-5 && state.recovery_velocity.abs() < 1.0e-4 {
            state.bloom_radians = 0.0;
            state.recovery_velocity = 0.0;
            state.shot_count = 0;
        }
    }
    let _ = world.insert(player, state);
}

#[cfg(test)]
pub(super) fn kick_weapon_accuracy(
    world: &mut World,
    player: EntityId,
    weapon_instance_id: ItemInstanceId,
    tuning: HitscanWeaponTuning,
) {
    kick_weapon_accuracy_with_profile(
        world,
        player,
        weapon_instance_id,
        WeaponRuntimeProfiles::from_legacy_tuning(tuning).spread,
        false,
    );
}

pub(super) fn kick_weapon_accuracy_with_profile(
    world: &mut World,
    player: EntityId,
    weapon_instance_id: ItemInstanceId,
    spread: WeaponSpreadProfile,
    aiming: bool,
) {
    let spread = spread.sanitized();
    let state_profile = if aiming { spread.ads } else { spread.hip };
    let resolved_stats = resolved_weapon_stats(world, player);
    let per_shot = state_profile.change_per_shot_radians[0]
        .max(state_profile.change_per_shot_radians[1])
        * resolved_stats.spread_multiplier;
    let maximum = state_profile.maximum_radians[0].max(state_profile.maximum_radians[1])
        * resolved_stats.spread_multiplier;
    let mut state = world
        .get::<WeaponAccuracyState>(player)
        .copied()
        .filter(|state| state.weapon_instance_id == weapon_instance_id)
        .unwrap_or_else(|| WeaponAccuracyState::new(weapon_instance_id));
    state.bloom_radians = (state.bloom_radians + per_shot).clamp(0.0, maximum);
    // Positive velocity delays the initial recovery and gives automatic fire a genuine accuracy
    // state instead of coupling dispersion to camera recoil.
    state.recovery_velocity += per_shot * spread.recovery_hz * 0.35;
    state.shot_count = state.shot_count.saturating_add(1);
    state.time_since_shot = 0.0;
    let _ = world.insert(player, state);
}

pub(super) fn recover_weapon_recoil(world: &mut World, player: EntityId, dt: f32) {
    let Some(mut recoil) = world.get::<WeaponRecoilRuntime>(player).copied() else {
        return;
    };
    if dt <= 0.0 {
        return;
    }
    if recoil.hold_remaining_seconds > 0.0 {
        recoil.hold_remaining_seconds = (recoil.hold_remaining_seconds - dt).max(0.0);
        let _ = world.insert(player, recoil);
        return;
    }
    let (next_pitch, next_pitch_speed) = critically_damped_recoil_step(
        recoil.applied_pitch_radians,
        recoil.pitch_speed_radians_per_second,
        recoil.recovery_hz,
        dt,
    );
    let (next_yaw, next_yaw_speed) = critically_damped_recoil_step(
        recoil.applied_yaw_radians,
        recoil.yaw_speed_radians_per_second,
        recoil.recovery_hz,
        dt,
    );
    if let Some(motor) = world.get_mut::<CharacterMotor>(player) {
        motor.pitch = (motor.pitch + next_pitch - recoil.applied_pitch_radians)
            .clamp(-motor.pitch_limit, motor.pitch_limit);
        motor.yaw += next_yaw - recoil.applied_yaw_radians;
    }
    recoil.applied_pitch_radians = next_pitch;
    recoil.applied_yaw_radians = next_yaw;
    recoil.pitch_speed_radians_per_second = next_pitch_speed;
    recoil.yaw_speed_radians_per_second = next_yaw_speed;
    if next_pitch.abs() < 1.0e-5
        && next_yaw.abs() < 1.0e-5
        && next_pitch_speed.abs() < 1.0e-4
        && next_yaw_speed.abs() < 1.0e-4
    {
        let _ = world.remove::<WeaponRecoilRuntime>(player);
    } else {
        let _ = world.insert(player, recoil);
    }
}

pub(super) fn fire_controller_wants_shot(
    world: &mut World,
    player: EntityId,
    weapon_instance_id: ItemInstanceId,
    pattern: FiringPatternDefinition,
    actions: FpsActionFrame,
    dt: f32,
) -> bool {
    let pattern = pattern.sanitized();
    let mut state = world
        .get::<WeaponFireControllerState>(player)
        .copied()
        .filter(|state| state.weapon_instance_id == weapon_instance_id)
        .unwrap_or_else(|| WeaponFireControllerState::new(weapon_instance_id));
    state.pattern_cooldown_seconds = (state.pattern_cooldown_seconds - dt.max(0.0)).max(0.0);
    let release_edge = state.trigger_was_held && !actions.fire_primary_held;
    if actions.fire_primary_held {
        state.activation_seconds = (state.activation_seconds + dt.max(0.0)).min(60.0);
    } else if !matches!(pattern.kind, FiringPatternKind::Charge) {
        state.activation_seconds = 0.0;
    }

    if matches!(
        pattern.kind,
        FiringPatternKind::Burst | FiringPatternKind::ScriptedSequence
    ) && actions.fire_primary_pressed
        && state.burst_shots_remaining == 0
        && state.pattern_cooldown_seconds <= 0.0
    {
        state.bursts_remaining = pattern.bursts_min;
        state.burst_shots_remaining = pattern.shots_per_burst_min;
    }

    let wants = match pattern.kind {
        FiringPatternKind::Semi | FiringPatternKind::Pump | FiringPatternKind::BoltAction => {
            actions.fire_primary_pressed && state.pattern_cooldown_seconds <= 0.0
        }
        FiringPatternKind::Automatic => {
            actions.fire_primary_held
                && state.activation_seconds >= pattern.delay_before_firing
                && state.pattern_cooldown_seconds <= 0.0
        }
        FiringPatternKind::Burst | FiringPatternKind::ScriptedSequence => {
            state.burst_shots_remaining > 0 && state.pattern_cooldown_seconds <= 0.0
        }
        FiringPatternKind::Charge => {
            release_edge && state.activation_seconds >= pattern.delay_before_firing
        }
        FiringPatternKind::SpinUp => {
            actions.fire_primary_held
                && state.activation_seconds >= pattern.delay_before_firing
                && state.pattern_cooldown_seconds <= 0.0
        }
        FiringPatternKind::Binary => {
            (actions.fire_primary_pressed || release_edge) && state.pattern_cooldown_seconds <= 0.0
        }
    };
    if release_edge && matches!(pattern.kind, FiringPatternKind::Charge) {
        state.activation_seconds = 0.0;
    }
    state.trigger_was_held = actions.fire_primary_held;
    let _ = world.insert(player, state);
    wants
}

pub(super) fn fire_controller_commit_shot(
    world: &mut World,
    player: EntityId,
    weapon_instance_id: ItemInstanceId,
    pattern: FiringPatternDefinition,
) {
    let pattern = pattern.sanitized();
    let mut state = world
        .get::<WeaponFireControllerState>(player)
        .copied()
        .filter(|state| state.weapon_instance_id == weapon_instance_id)
        .unwrap_or_else(|| WeaponFireControllerState::new(weapon_instance_id));
    match pattern.kind {
        FiringPatternKind::Burst | FiringPatternKind::ScriptedSequence => {
            state.burst_shots_remaining = state.burst_shots_remaining.saturating_sub(1);
            if state.burst_shots_remaining == 0 {
                state.bursts_remaining = state.bursts_remaining.saturating_sub(1);
                if state.bursts_remaining > 0 {
                    state.burst_shots_remaining = pattern.shots_per_burst_min;
                    state.pattern_cooldown_seconds = pattern.time_between_bursts;
                } else {
                    state.pattern_cooldown_seconds = pattern.burst_cooldown;
                }
            }
        }
        FiringPatternKind::Pump | FiringPatternKind::BoltAction => {
            state.pattern_cooldown_seconds = pattern
                .burst_cooldown
                .max(pattern.time_between_bursts)
                .max(pattern.time_between_shots);
        }
        FiringPatternKind::Binary => {
            state.pattern_cooldown_seconds = pattern.time_between_shots;
        }
        FiringPatternKind::Charge => {
            state.activation_seconds = 0.0;
            state.pattern_cooldown_seconds = pattern.burst_cooldown;
        }
        FiringPatternKind::Semi | FiringPatternKind::Automatic | FiringPatternKind::SpinUp => {}
    }
    let _ = world.insert(player, state);
}
