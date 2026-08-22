use super::*;

#[inline]
fn sanitized_dt(dt: f32) -> f32 {
    if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    }
}

#[inline]
fn select_ground_locomotion_animation(
    crouched: bool,
    horizontal_speed: f32,
    normalized_speed: f32,
    sprinting: bool,
) -> PlayerLocomotionAnimation {
    if crouched {
        return if horizontal_speed > 0.08 {
            PlayerLocomotionAnimation::CrouchWalk
        } else {
            PlayerLocomotionAnimation::CrouchIdle
        };
    }
    if horizontal_speed <= 0.08 {
        PlayerLocomotionAnimation::Idle
    } else if sprinting {
        PlayerLocomotionAnimation::Sprint
    } else if normalized_speed >= 0.55 {
        PlayerLocomotionAnimation::Run
    } else {
        PlayerLocomotionAnimation::Walk
    }
}

#[inline]
fn select_locomotion_animation(
    grounded: bool,
    crouched: bool,
    horizontal_speed: f32,
    normalized_speed: f32,
    vertical_speed: f32,
    sprinting: bool,
    airborne_time: f32,
    jump_started: bool,
) -> PlayerLocomotionAnimation {
    let ground_locomotion =
        select_ground_locomotion_animation(crouched, horizontal_speed, normalized_speed, sprinting);
    if grounded {
        return ground_locomotion;
    }

    // The character controller can report one or more ground-probe misses while its
    // vertical correction velocity oscillates around zero. Treating every miss as an
    // airborne state caused idle/walk/run to thrash into jump/fall many times per second.
    // Airborne animation therefore requires sustained separation plus meaningful Y speed.
    let airborne_time = if airborne_time.is_finite() {
        airborne_time.max(0.0)
    } else {
        0.0
    };
    if jump_started {
        if !vertical_speed.is_finite() || vertical_speed > -0.45 || airborne_time < 0.08 {
            return PlayerLocomotionAnimation::Jump;
        }
        return PlayerLocomotionAnimation::Fall;
    }
    // A rigid character capsule can receive short positive/negative Y impulses from
    // uneven terrain. They are physics correction, not jump/fall intent. Walking off
    // a ledge therefore requires sustained, meaningful downward motion before Fall.
    if vertical_speed.is_finite() && vertical_speed < -2.5 && airborne_time >= 0.35 {
        return PlayerLocomotionAnimation::Fall;
    }

    // Ground-contact uncertainty is presentation-only. Physics remains authoritative;
    // locomotion animation holds the appropriate grounded pose until a true jump/fall
    // has enough temporal/velocity evidence to be visually stable.
    ground_locomotion
}

#[inline]
fn cycle_rate_hz(state: PlayerLocomotionAnimation, normalized_speed: f32) -> f32 {
    match state {
        PlayerLocomotionAnimation::Idle => 0.22,
        PlayerLocomotionAnimation::Walk => 1.55 * normalized_speed.clamp(0.45, 1.25),
        PlayerLocomotionAnimation::Run => 2.15 * normalized_speed.clamp(0.7, 1.45),
        PlayerLocomotionAnimation::Sprint => 2.65 * normalized_speed.clamp(1.0, 1.8),
        PlayerLocomotionAnimation::CrouchIdle => 0.16,
        PlayerLocomotionAnimation::CrouchWalk => 1.05 * normalized_speed.clamp(0.4, 1.0),
        PlayerLocomotionAnimation::Jump => 0.75,
        PlayerLocomotionAnimation::Fall => 0.35,
    }
}

/// Updates semantic locomotion animation state after physics.
///
/// This deliberately does not deform the mesh itself. It is the stable player-side
/// input to the skeletal animation backend: YCD/blend-tree/motion-matching providers
/// can consume `locomotion`, `normalized_speed` and `cycle_phase` without coupling
/// player physics or camera code to a particular clip format.
pub fn update_player_animation_states(world: &mut World, dt: f32) {
    let dt = sanitized_dt(dt);
    let players = world
        .query::<PlayerActor>()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();

    for player in players {
        let velocity = world.get::<Velocity>(player).copied().unwrap_or_default().0;
        let horizontal_speed = Vec3::new(velocity.x, 0.0, velocity.z).length();
        let ground = world
            .get::<PlayerGroundState>(player)
            .copied()
            .unwrap_or_default();
        // Before the first physics result arrives, keep the authored spawn pose in idle
        // rather than reporting a one-frame fall transition.
        let grounded = ground.grounded || ground.last_fixed_tick == 0;
        let crouched = world
            .get::<PlayerStanceState>(player)
            .is_some_and(|state| matches!(state.current, PlayerStanceKind::Crouched));
        let input = world.get::<MotorInput>(player).copied().unwrap_or_default();
        let locomotion_state = world
            .get::<PlayerLocomotionState>(player)
            .copied()
            .unwrap_or_default();
        let sprinting = input.speed_mul > 1.05 && horizontal_speed > 0.08;
        let base_speed = world
            .get::<CharacterMotor>(player)
            .map(|motor| motor.move_speed.max(0.01))
            .unwrap_or(6.0);
        let normalized_speed = (horizontal_speed / base_speed).clamp(0.0, 2.0);
        let desired = select_locomotion_animation(
            grounded,
            crouched,
            horizontal_speed,
            normalized_speed,
            velocity.y,
            sprinting,
            locomotion_state.airborne_time,
            locomotion_state.jump_started,
        );

        let mut changed = false;
        if let Some(state) = world.get_mut::<PlayerAnimationState>(player) {
            if state.locomotion != desired {
                state.locomotion = desired;
                state.revision = state.revision.saturating_add(1).max(1);
                state.transition_alpha = 0.0;
                // Airborne transitions are one-shot-ish; locomotion loops start at a stable phase.
                if matches!(
                    desired,
                    PlayerLocomotionAnimation::Jump | PlayerLocomotionAnimation::Fall
                ) {
                    state.cycle_phase = 0.0;
                }
                changed = true;
            }
            let speed_blend = if dt > 0.0 {
                1.0 - (-dt / 0.08).exp()
            } else {
                1.0
            };
            state.normalized_speed += (normalized_speed - state.normalized_speed) * speed_blend;
            state.transition_alpha = (state.transition_alpha + dt / 0.12).clamp(0.0, 1.0);
            let rate = cycle_rate_hz(state.locomotion, state.normalized_speed);
            state.cycle_phase = (state.cycle_phase + dt * rate).fract();
        } else {
            let mut state = PlayerAnimationState::default();
            state.locomotion = desired;
            state.normalized_speed = normalized_speed;
            let _ = world.insert(player, state);
            changed = true;
        }

        if changed {
            let state = world
                .get::<PlayerAnimationState>(player)
                .copied()
                .unwrap_or_default();
            emit_player_event(
                world,
                player,
                PlayerEventKind::AnimationStateChanged,
                format!(
                    "locomotion='{}' normalized_speed={:.3} revision={}",
                    state.locomotion.clip_hint(),
                    state.normalized_speed,
                    state.revision
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locomotion_semantics_cover_ground_crouch_and_air() {
        assert_eq!(
            select_locomotion_animation(true, false, 0.0, 0.0, 0.0, false, 0.0, false),
            PlayerLocomotionAnimation::Idle
        );
        assert_eq!(
            select_locomotion_animation(true, false, 2.0, 0.33, 0.0, false, 0.0, false),
            PlayerLocomotionAnimation::Walk
        );
        assert_eq!(
            select_locomotion_animation(true, false, 4.0, 0.7, 0.0, false, 0.0, false),
            PlayerLocomotionAnimation::Run
        );
        assert_eq!(
            select_locomotion_animation(true, false, 6.0, 1.0, 0.0, true, 0.0, false),
            PlayerLocomotionAnimation::Sprint
        );
        assert_eq!(
            select_locomotion_animation(true, true, 0.0, 0.0, 0.0, false, 0.0, false),
            PlayerLocomotionAnimation::CrouchIdle
        );
        assert_eq!(
            select_locomotion_animation(true, true, 1.0, 0.2, 0.0, false, 0.0, false),
            PlayerLocomotionAnimation::CrouchWalk
        );
        assert_eq!(
            select_locomotion_animation(false, false, 1.0, 0.2, 5.0, false, 0.05, true),
            PlayerLocomotionAnimation::Jump
        );
        assert_eq!(
            select_locomotion_animation(false, false, 1.0, 0.2, -3.0, false, 0.20, true),
            PlayerLocomotionAnimation::Fall
        );
    }

    #[test]
    fn ground_probe_glitches_do_not_force_fall_animation() {
        assert_eq!(
            select_locomotion_animation(false, false, 0.0, 0.0, -0.12, false, 0.016, false),
            PlayerLocomotionAnimation::Idle
        );
        assert_eq!(
            select_locomotion_animation(false, false, 3.0, 0.50, 0.28, false, 0.032, false),
            PlayerLocomotionAnimation::Walk
        );
    }

    #[test]
    fn terrain_contact_upward_impulse_does_not_synthesize_jump() {
        assert_eq!(
            select_locomotion_animation(false, false, 7.3, 1.0, 4.2, false, 1.0, false),
            PlayerLocomotionAnimation::Run
        );
    }

    #[test]
    fn sustained_airborne_motion_enters_jump_and_fall() {
        assert_eq!(
            select_locomotion_animation(false, false, 1.0, 0.2, 5.0, false, 0.016, true),
            PlayerLocomotionAnimation::Jump
        );
        assert_eq!(
            select_locomotion_animation(false, false, 1.0, 0.2, -3.0, false, 0.20, true),
            PlayerLocomotionAnimation::Fall
        );
    }

    #[test]
    fn player_animation_state_transitions_from_idle_to_walk() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "animated-player", Vec3::ZERO);
        if let Some(velocity) = world.get_mut::<Velocity>(player) {
            velocity.0 = Vec3::new(0.0, 0.0, -2.0);
        }
        if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
            ground.grounded = true;
            ground.last_fixed_tick = 1;
        }

        update_player_animation_states(&mut world, 1.0 / 60.0);

        let state = world
            .get::<PlayerAnimationState>(player)
            .copied()
            .expect("animation state");
        assert_eq!(state.locomotion, PlayerLocomotionAnimation::Walk);
        assert!(state.normalized_speed > 0.0);
        assert!(state.revision >= 2);
    }
}
