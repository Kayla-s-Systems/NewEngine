#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    active_equipped_weapon_binding, apply_player_stance_geometry, emit_gameplay_event,
    is_player_controller_enabled, update_player_stance_camera, CharacterBody,
    CharacterExertionState, CharacterMotionTuning, PhysicsSurface, PlayerCommandFrame,
    PlayerController, PlayerGroundState, PlayerLocomotionState, PlayerMovementSpeeds,
    PlayerStanceKind, PlayerStanceState,
};
use newengine_gameplay_fps_api::{FpsActionFrame, FpsGameplayPolicySnapshot};
use newengine_sim::{MotorInput, Velocity};
use newengine_transform::Transform;

#[inline]
fn fps_ground_speed_multiplier(
    movement: PlayerMovementSpeeds,
    crouched: bool,
    sprinting: bool,
    aiming: bool,
) -> f32 {
    let movement = movement.sanitized();
    if aiming {
        // ADS owns deliberate combat locomotion. Use the character-authored walk/run ratio rather
        // than a product hard-code, and never allow sprint to override it while the weapon is up.
        (movement.walk / movement.run).clamp(0.05, 1.0)
    } else if crouched {
        movement.crouch_multiplier()
    } else if sprinting {
        movement.sprint_multiplier()
    } else {
        1.0
    }
}

fn publish_jump_surface_event(
    world: &mut World,
    player: EntityId,
    ground_key: Option<u64>,
    source_frame: u64,
    jump_speed: f32,
) {
    let Some(ground_key) = ground_key else {
        return;
    };
    let surface = world
        .query::<PhysicsSurface>()
        .find(|(entity, _)| entity.stable_u64() == ground_key)
        .map(|(entity, surface)| (entity, surface.clone()));
    let Some((surface_entity, surface)) = surface else {
        return;
    };
    let Some(event_id) = surface.event_for("jump").map(str::to_owned) else {
        return;
    };
    let position = world
        .get::<Transform>(player)
        .map(|transform| {
            [
                transform.position.x,
                transform.position.y,
                transform.position.z,
            ]
        })
        .unwrap_or([0.0; 3]);
    if let Err(error) = emit_gameplay_event(
        world,
        event_id.clone(),
        Some(player),
        serde_json::json!({
            "source_kind": "character_control",
            "phase": "lift",
            "mode": "jump",
            "surface": surface.id,
            "surface_entity": surface_entity.stable_u64(),
            "position": position,
            "sequence": source_frame,
            "jump_speed": jump_speed,
        }),
    ) {
        newengine_ulog_api::ulog::warn!(
            "jump surface event publish rejected event='{}' player={} err='{}'",
            event_id,
            player.stable_u64(),
            error,
        );
    }
}

/// FPS-owned interpretation of generic semantic command transport.
/// The engine owns stance geometry/motion components; this package owns what jump/crouch mean.
pub fn apply_fps_character_commands(world: &mut World, dt: f32, fixed_tick: u64) {
    let player_policy = world
        .resource::<FpsGameplayPolicySnapshot>()
        .map(|policy| policy.player)
        .unwrap_or_default();
    let players = world
        .query2_ids::<PlayerController, PlayerCommandFrame>()
        .collect::<Vec<_>>();

    for player in players {
        if !is_player_controller_enabled(world, player) {
            continue;
        }
        let (source_frame, actions) = world
            .get::<PlayerCommandFrame>(player)
            .map(|commands| {
                (
                    commands.source_frame,
                    FpsActionFrame::from_commands(&commands.actions),
                )
            })
            .unwrap_or_default();
        let body = world
            .get::<CharacterBody>(player)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let motion = world
            .get::<CharacterMotionTuning>(player)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let stance = world
            .get::<PlayerStanceState>(player)
            .copied()
            .unwrap_or_else(|| PlayerStanceState::standing(body.standing_eye_height));

        if actions.noclip_toggle_pressed {
            let _ =
                crate::noclip::toggle_fps_noclip_once_for_source_frame(world, player, source_frame);
        }

        if crate::noclip::fps_noclip_enabled(world, player) {
            // Noclip owns collision/gravity and 3-axis travel. Do not run stance or jump
            // semantics while the body is intentionally detached from the physics world.
            continue;
        }

        let crouched = stance.current == PlayerStanceKind::Crouched;
        let aiming_with_weapon = actions.aim_held
            && active_equipped_weapon_binding(world, player)
                .is_some_and(|binding| binding.capabilities().aim);
        let movement = world
            .get::<PlayerMovementSpeeds>(player)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let requested_sprint = world
            .get::<MotorInput>(player)
            .is_some_and(|input| input.speed_mul > 1.05);
        let sprinting = player_policy.allow_sprint && requested_sprint && !aiming_with_weapon;
        if let Some(input) = world.get_mut::<MotorInput>(player) {
            input.speed_mul =
                fps_ground_speed_multiplier(movement, crouched, sprinting, aiming_with_weapon);
        }
        if aiming_with_weapon {
            if let Some(exertion) = world.get_mut::<CharacterExertionState>(player) {
                exertion.sprinting = false;
            }
        }

        if player_policy.allow_crouch && actions.crouch_held {
            if stance.current != PlayerStanceKind::Crouched {
                let _ = apply_player_stance_geometry(
                    world,
                    player,
                    PlayerStanceKind::Crouched,
                    fixed_tick,
                );
            }
            if let Some(state) = world.get_mut::<PlayerStanceState>(player) {
                state.stand_requested = false;
                state.stand_blocked = false;
                state.target_eye_height = body.crouched_eye_height;
            }
        } else if stance.current == PlayerStanceKind::Crouched {
            if let Some(state) = world.get_mut::<PlayerStanceState>(player) {
                state.stand_requested = true;
                state.target_eye_height = body.crouched_eye_height;
            }
        }

        let ground_state = world
            .get::<PlayerGroundState>(player)
            .copied()
            .unwrap_or_default();
        let grounded = ground_state.grounded;

        // CharacterMotor is intentionally input-driven and can produce zero lateral velocity
        // for a zero movement sample. During an explicit jump that must not erase takeoff
        // momentum: preserve X/Z until the player supplies new horizontal air input.
        let locomotion_state = world
            .get::<PlayerLocomotionState>(player)
            .copied()
            .unwrap_or_default();
        let air_input_active = world
            .get::<MotorInput>(player)
            .map(|input| {
                let horizontal = newengine_math::Vec2::new(input.move_axis.x, input.move_axis.z);
                horizontal.length_squared() > 1.0e-6
            })
            .unwrap_or(false);
        if !grounded && locomotion_state.jump_started && !air_input_active {
            if let Some(velocity) = world.get_mut::<Velocity>(player) {
                velocity.0.x = locomotion_state.jump_takeoff_horizontal_velocity.x;
                velocity.0.z = locomotion_state.jump_takeoff_horizontal_velocity.z;
            }
        }

        let jump_pressed = if actions.jump_pressed {
            let already_consumed = world
                .get::<PlayerLocomotionState>(player)
                .and_then(|state| state.last_jump_command_source_frame)
                == Some(source_frame);
            if already_consumed {
                false
            } else {
                if let Some(state) = world.get_mut::<PlayerLocomotionState>(player) {
                    state.last_jump_command_source_frame = Some(source_frame);
                }
                true
            }
        } else {
            false
        };
        if player_policy.allow_jump && jump_pressed && grounded && motion.jump_speed > 0.0 {
            publish_jump_surface_event(
                world,
                player,
                ground_state.ground_entity,
                source_frame,
                motion.jump_speed,
            );
            let mut velocity = world.get::<Velocity>(player).copied().unwrap_or_default();
            let takeoff_horizontal = newengine_math::Vec3::new(velocity.0.x, 0.0, velocity.0.z);
            velocity.0.y = motion.jump_speed;
            let _ = world.insert(player, velocity);
            if let Some(state) = world.get_mut::<PlayerGroundState>(player) {
                state.grounded = false;
                state.walkable = false;
                state.ground_entity = None;
                state.distance = f32::INFINITY;
            }
            if let Some(state) = world.get_mut::<PlayerLocomotionState>(player) {
                state.jump_started = true;
                state.jump_takeoff_horizontal_velocity = takeoff_horizontal;
                state.airborne_time = 0.0;
                state.max_downward_speed = 0.0;
            }
        }
    }

    update_player_stance_camera(world, dt);
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_engine_runtime::gameplay::{
        drain_gameplay_events, spawn_default_player, CharacterBody, PhysicsSurface,
        PlayerGroundState, PlayerStanceKind, PlayerStanceState,
    };
    use newengine_gameplay_fps_api::action;
    use newengine_input_actions_api::ActionCommandFrame;
    use newengine_math::Vec3;

    #[test]
    fn ads_ground_speed_uses_authored_walk_ratio_and_suppresses_sprint() {
        let movement = PlayerMovementSpeeds {
            walk: 1.5,
            run: 3.0,
            sprint: 4.6,
            crouch: 1.0,
        };
        assert!((fps_ground_speed_multiplier(movement, false, false, true) - 0.5).abs() <= 1.0e-6);
        assert!((fps_ground_speed_multiplier(movement, false, true, true) - 0.5).abs() <= 1.0e-6);
        assert!(
            (fps_ground_speed_multiplier(movement, false, true, false) - (4.6 / 3.0)).abs()
                <= 1.0e-6
        );
    }

    #[test]
    fn grounded_fps_jump_sets_vertical_velocity_and_clears_ground_state() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "fps-jump", Vec3::ZERO);
        let jump_speed = world
            .get::<CharacterMotionTuning>(player)
            .copied()
            .unwrap_or_default()
            .jump_speed;
        if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
            ground.grounded = true;
            ground.walkable = true;
            ground.ground_entity = Some(99);
        }
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.actions = ActionCommandFrame {
                pressed: vec![action::PLAYER_JUMP.into()],
                ..ActionCommandFrame::default()
            };
        }

        apply_fps_character_commands(&mut world, 1.0 / 60.0, 7);

        assert_eq!(
            world.get::<Velocity>(player).map(|velocity| velocity.0.y),
            Some(jump_speed)
        );
        assert!(
            !world
                .get::<PlayerGroundState>(player)
                .expect("ground state")
                .grounded
        );
        assert!(
            world
                .get::<PlayerLocomotionState>(player)
                .expect("locomotion state")
                .jump_started,
            "gameplay jump must publish explicit airborne origin for animation semantics"
        );
    }

    #[test]
    fn explicit_jump_preserves_takeoff_momentum_across_zero_air_input_frame() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "fps-jump-momentum", Vec3::ZERO);
        if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
            ground.grounded = true;
            ground.walkable = true;
        }
        let _ = world.insert(player, Velocity(Vec3::new(4.0, 0.0, 1.5)));
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = 42;
            commands.actions = ActionCommandFrame {
                pressed: vec![action::PLAYER_JUMP.into()],
                ..ActionCommandFrame::default()
            };
        }

        apply_fps_character_commands(&mut world, 1.0 / 60.0, 7);
        let takeoff = world
            .get::<Velocity>(player)
            .copied()
            .expect("takeoff velocity")
            .0;
        assert!((takeoff.x - 4.0).abs() <= 1.0e-6);
        assert!((takeoff.z - 1.5).abs() <= 1.0e-6);
        assert!(takeoff.y > 0.0);

        // Simulate one zero movement packet on the next airborne fixed tick.
        let _ = world.insert(player, Velocity(Vec3::new(0.0, takeoff.y - 0.2, 0.0)));
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = 43;
            commands.actions = ActionCommandFrame::default();
        }
        if let Some(input) = world.get_mut::<MotorInput>(player) {
            input.move_axis = Vec3::ZERO;
        }
        apply_fps_character_commands(&mut world, 1.0 / 60.0, 8);
        let airborne = world
            .get::<Velocity>(player)
            .copied()
            .expect("airborne velocity")
            .0;
        assert!((airborne.x - 4.0).abs() <= 1.0e-6);
        assert!((airborne.z - 1.5).abs() <= 1.0e-6);
        assert!((airborne.y - (takeoff.y - 0.2)).abs() <= 1.0e-6);
    }

    #[test]
    fn accepted_jump_publishes_authored_surface_signal_before_ground_clear() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "fps-jump-audio", Vec3::ZERO);
        let ground = world.spawn();
        let _ = world.insert(
            ground,
            PhysicsSurface::default().with_event("jump", "room.audio.footstep.jump"),
        );
        if let Some(ground_state) = world.get_mut::<PlayerGroundState>(player) {
            ground_state.grounded = true;
            ground_state.walkable = true;
            ground_state.ground_entity = Some(ground.stable_u64());
        }
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = 77;
            commands.actions = ActionCommandFrame {
                pressed: vec![action::PLAYER_JUMP.into()],
                ..ActionCommandFrame::default()
            };
        }

        apply_fps_character_commands(&mut world, 1.0 / 60.0, 7);

        let events = drain_gameplay_events(&mut world);
        let jump = events
            .iter()
            .find(|event| event.id == "room.audio.footstep.jump")
            .expect("accepted jump must publish authored surface signal");
        assert_eq!(jump.source, Some(player.stable_u64()));
        assert_eq!(jump.payload["sequence"], 77);
        assert_eq!(jump.payload["phase"], "lift");
    }

    #[test]
    fn jump_edge_is_consumed_once_per_input_source_frame() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "fps-jump-once", Vec3::ZERO);
        if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
            ground.grounded = true;
            ground.walkable = true;
            ground.ground_entity = Some(99);
        }
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = 77;
            commands.actions = ActionCommandFrame {
                pressed: vec![action::PLAYER_JUMP.into()],
                ..ActionCommandFrame::default()
            };
        }
        apply_fps_character_commands(&mut world, 1.0 / 60.0, 7);
        assert_eq!(
            world
                .get::<PlayerLocomotionState>(player)
                .and_then(|state| state.last_jump_command_source_frame),
            Some(77)
        );

        // Simulate a later fixed tick seeing the exact same sampled input frame after a
        // physics/contact update restored grounding. It must not jump a second time.
        if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
            ground.grounded = true;
            ground.walkable = true;
        }
        if let Some(velocity) = world.get_mut::<Velocity>(player) {
            velocity.0.y = 0.0;
        }
        apply_fps_character_commands(&mut world, 1.0 / 60.0, 8);
        assert_eq!(
            world.get::<Velocity>(player).map(|velocity| velocity.0.y),
            Some(0.0)
        );

        // A genuinely new input sample is allowed to initiate the next jump.
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = 78;
        }
        apply_fps_character_commands(&mut world, 1.0 / 60.0, 9);
        assert!(world
            .get::<Velocity>(player)
            .is_some_and(|velocity| velocity.0.y > 0.0));
    }

    #[test]
    fn f7_noclip_edge_toggles_once_per_input_source_frame() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "fps-noclip-f7", Vec3::ZERO);
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = 700;
            commands.actions = ActionCommandFrame {
                pressed: vec![action::NOCLIP_TOGGLE.into()],
                ..ActionCommandFrame::default()
            };
        }
        apply_fps_character_commands(&mut world, 1.0 / 60.0, 1);
        assert!(crate::noclip::fps_noclip_enabled(&world, player));
        assert!(world
            .get::<newengine_engine_runtime::gameplay::PhysicsBodyDesc>(player)
            .is_none());

        // Same sampled input edge on catch-up fixed tick must not toggle back off.
        apply_fps_character_commands(&mut world, 1.0 / 60.0, 2);
        assert!(crate::noclip::fps_noclip_enabled(&world, player));

        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = 701;
        }
        apply_fps_character_commands(&mut world, 1.0 / 60.0, 3);
        assert!(!crate::noclip::fps_noclip_enabled(&world, player));
        assert!(world
            .get::<newengine_engine_runtime::gameplay::PhysicsBodyDesc>(player)
            .is_some());
    }

    #[test]
    fn held_crouch_does_not_request_stand_until_release() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "fps-crouch-held", Vec3::ZERO);

        for (fixed_tick, source_frame) in [(21_u64, 100_u64), (22, 101), (23, 102), (24, 103)] {
            if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
                commands.source_frame = source_frame;
                commands.actions = ActionCommandFrame {
                    held: vec![action::PLAYER_CROUCH.into()],
                    ..ActionCommandFrame::default()
                };
            }
            apply_fps_character_commands(&mut world, 1.0 / 60.0, fixed_tick);
            let stance = world.get::<PlayerStanceState>(player).expect("stance");
            assert_eq!(stance.current, PlayerStanceKind::Crouched);
            assert!(
                !stance.stand_requested,
                "held crouch requested stand on source_frame={source_frame}"
            );
        }

        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.source_frame = 104;
            commands.actions = ActionCommandFrame::default();
        }
        apply_fps_character_commands(&mut world, 1.0 / 60.0, 25);
        let stance = world.get::<PlayerStanceState>(player).expect("stance");
        assert_eq!(stance.current, PlayerStanceKind::Crouched);
        assert!(
            stance.stand_requested,
            "release must request a clearance-tested stand"
        );
    }

    #[test]
    fn fps_crouch_policy_drives_generic_stance_geometry() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "fps-crouch", Vec3::ZERO);
        let body = world
            .get::<CharacterBody>(player)
            .copied()
            .unwrap_or_default()
            .sanitized();
        if let Some(commands) = world.get_mut::<PlayerCommandFrame>(player) {
            commands.actions = ActionCommandFrame {
                held: vec![action::PLAYER_CROUCH.into()],
                ..ActionCommandFrame::default()
            };
        }

        apply_fps_character_commands(&mut world, 1.0 / 60.0, 11);

        let stance = world
            .get::<PlayerStanceState>(player)
            .copied()
            .expect("stance state");
        assert_eq!(stance.current, PlayerStanceKind::Crouched);
        assert_eq!(stance.target_eye_height, body.crouched_eye_height);
    }
}
