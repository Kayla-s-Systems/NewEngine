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
fn select_locomotion_animation(
    grounded: bool,
    crouched: bool,
    horizontal_speed: f32,
    normalized_speed: f32,
    vertical_speed: f32,
    sprinting: bool,
) -> PlayerLocomotionAnimation {
    if !grounded {
        return if vertical_speed > 0.15 {
            PlayerLocomotionAnimation::Jump
        } else {
            PlayerLocomotionAnimation::Fall
        };
    }
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
            select_locomotion_animation(true, false, 0.0, 0.0, 0.0, false),
            PlayerLocomotionAnimation::Idle
        );
        assert_eq!(
            select_locomotion_animation(true, false, 2.0, 0.33, 0.0, false),
            PlayerLocomotionAnimation::Walk
        );
        assert_eq!(
            select_locomotion_animation(true, false, 4.0, 0.7, 0.0, false),
            PlayerLocomotionAnimation::Run
        );
        assert_eq!(
            select_locomotion_animation(true, false, 6.0, 1.0, 0.0, true),
            PlayerLocomotionAnimation::Sprint
        );
        assert_eq!(
            select_locomotion_animation(true, true, 0.0, 0.0, 0.0, false),
            PlayerLocomotionAnimation::CrouchIdle
        );
        assert_eq!(
            select_locomotion_animation(true, true, 1.0, 0.2, 0.0, false),
            PlayerLocomotionAnimation::CrouchWalk
        );
        assert_eq!(
            select_locomotion_animation(false, false, 1.0, 0.2, 1.0, false),
            PlayerLocomotionAnimation::Jump
        );
        assert_eq!(
            select_locomotion_animation(false, false, 1.0, 0.2, -1.0, false),
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
