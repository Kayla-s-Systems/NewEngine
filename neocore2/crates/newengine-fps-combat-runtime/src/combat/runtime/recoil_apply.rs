/// Backward-compatible public entry point retained for existing callers/tests. The implementation
/// now processes both player command frames and controller-neutral AI combat actuation frames.
#[inline]
pub fn step_player_combat(world: &mut World, dt: f32, fixed_tick: u64) {
    step_actor_combat(world, dt, fixed_tick);
}

#[cfg(test)]
pub(super) fn apply_recoil(
    world: &mut World,
    player: EntityId,
    weapon_instance_id: ItemInstanceId,
    tuning: HitscanWeaponTuning,
    aiming: bool,
    shot_sequence: u64,
) {
    apply_recoil_with_profile(
        world,
        player,
        weapon_instance_id,
        WeaponRuntimeProfiles::from_legacy_tuning(tuning).recoil,
        aiming,
        shot_sequence,
    );
}

pub(super) fn apply_recoil_with_profile(
    world: &mut World,
    player: EntityId,
    weapon_instance_id: ItemInstanceId,
    profile: WeaponRecoilProfile,
    aiming: bool,
    shot_sequence: u64,
) {
    let profile = profile.sanitized();
    let recoil_state = profile.state(aiming);
    let component_recoil = resolved_weapon_stats(world, player).recoil_multiplier;
    let pitch_noise = signed_unit(shot_sequence ^ 0x243f_6a88_85a3_08d3);
    let yaw_noise = signed_unit(shot_sequence ^ 0x1319_8a2e_0370_7344);
    let pitch_kick = (recoil_state.pitch_radians + pitch_noise * recoil_state.pitch_random_radians)
        .max(0.0)
        * component_recoil;
    let yaw_kick =
        (recoil_state.yaw_bias_radians + yaw_noise * recoil_state.yaw_radians) * component_recoil;

    let previous = world.get::<WeaponRecoilRuntime>(player).copied();
    if let Some(previous) = previous.filter(|state| state.weapon_instance_id != weapon_instance_id)
    {
        if let Some(motor) = world.get_mut::<CharacterMotor>(player) {
            motor.pitch = (motor.pitch - previous.applied_pitch_radians)
                .clamp(-motor.pitch_limit, motor.pitch_limit);
            motor.yaw -= previous.applied_yaw_radians;
        }
        let _ = world.remove::<WeaponRecoilRuntime>(player);
    }

    let Some(motor) = world.get_mut::<CharacterMotor>(player) else {
        return;
    };
    // Positive pitch rotates the canonical -Z forward vector upward. Keep the immediate impulse
    // responsive, then let the recoil tracker carry a short follow-through before settling.
    let prior_pitch = motor.pitch;
    motor.pitch = (motor.pitch + pitch_kick).clamp(-motor.pitch_limit, motor.pitch_limit);
    let applied_pitch_kick = motor.pitch - prior_pitch;
    motor.yaw += yaw_kick;

    let mut recoil =
        world
            .get::<WeaponRecoilRuntime>(player)
            .copied()
            .unwrap_or(WeaponRecoilRuntime {
                weapon_instance_id,
                applied_pitch_radians: 0.0,
                applied_yaw_radians: 0.0,
                pitch_speed_radians_per_second: 0.0,
                yaw_speed_radians_per_second: 0.0,
                recovery_hz: recoil_state.recovery_hz,
                hold_remaining_seconds: recoil_state.hold_seconds,
            });
    recoil.weapon_instance_id = weapon_instance_id;
    recoil.applied_pitch_radians += applied_pitch_kick;
    recoil.applied_yaw_radians += yaw_kick;
    // Angle impulse and tracker velocity are independent authored quantities. This preserves a
    // crisp trigger response while allowing each weapon to own how strongly recoil continues for
    // the first few frames before the critically damped recovery takes over.
    recoil.pitch_speed_radians_per_second +=
        applied_pitch_kick * recoil_state.recovery_hz * recoil_state.pitch_tracker_speed_scale;
    recoil.yaw_speed_radians_per_second +=
        yaw_kick * recoil_state.recovery_hz * recoil_state.yaw_tracker_speed_scale;
    recoil.recovery_hz = recoil_state.recovery_hz;
    recoil.hold_remaining_seconds = recoil.hold_remaining_seconds.max(recoil_state.hold_seconds);
    let _ = world.insert(player, recoil);
}

