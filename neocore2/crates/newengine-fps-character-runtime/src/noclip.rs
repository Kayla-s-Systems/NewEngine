#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::{EntityId, World};
use newengine_engine_runtime::gameplay::{
    ensure_physics_body, remove_physics_body, PhysicsBodyDesc, PlayerFallState, PlayerGroundState,
};
use newengine_gameplay_fps_api::{FpsCharacterTraversalMode, FpsCharacterTraversalState};
use newengine_math::{Quat, Vec3};
use newengine_sim::{CharacterMotor, MotorInput, Velocity};
use newengine_transform::{
    read_entity_world_pose_local_chain, write_entity_local_from_world_pose_local_chain,
    TransformDirty,
};

#[derive(Clone, Copy, Debug, Default)]
struct FpsNoClipState {
    enabled: bool,
    saved_body: Option<PhysicsBodyDesc>,
    last_toggle_source_frame: Option<u64>,
}

#[inline]
pub fn fps_noclip_enabled(world: &World, player: EntityId) -> bool {
    world
        .get::<FpsNoClipState>(player)
        .is_some_and(|state| state.enabled)
}

fn reset_player_fall_state(world: &mut World, player: EntityId) {
    let height = read_entity_world_pose_local_chain(world, player)
        .map(|(position, _)| position.y)
        .filter(|height| height.is_finite())
        .unwrap_or(0.0);
    let revision = world
        .get::<PlayerFallState>(player)
        .map(|state| state.revision.saturating_add(1).max(1))
        .unwrap_or(1);
    let _ = world.insert(
        player,
        PlayerFallState {
            start_height: height,
            peak_height: height,
            current_height: height,
            revision,
            ..PlayerFallState::default()
        },
    );
}

pub fn set_fps_noclip(world: &mut World, player: EntityId, enabled: bool) -> bool {
    if !world.exists(player) {
        return false;
    }
    let current = world
        .get::<FpsNoClipState>(player)
        .copied()
        .unwrap_or_default();
    if current.enabled == enabled {
        return false;
    }

    if enabled {
        if world
            .get::<newengine_engine_runtime::gameplay::PlayerAuthoredAnimationCapabilities>(player)
            .is_some_and(|capabilities| !capabilities.noclip)
        {
            newengine_ulog_api::ulog::warn!(
                "fps noclip rejected player={} reason=missing_authored_noclip_animation policy=no_locomotion_or_bind_pose_fallback",
                player.stable_u64()
            );
            return false;
        }
        let saved_body = world
            .get::<PhysicsBodyDesc>(player)
            .copied()
            .or(current.saved_body);
        let _ = world.insert(
            player,
            FpsNoClipState {
                enabled: true,
                saved_body,
                last_toggle_source_frame: current.last_toggle_source_frame,
            },
        );
        let traversal = world
            .resource::<FpsCharacterTraversalState>()
            .copied()
            .unwrap_or_default()
            .with_mode(FpsCharacterTraversalMode::NoClip);
        world.insert_resource(traversal);
        remove_physics_body(world, player);
        if let Some(velocity) = world.get_mut::<Velocity>(player) {
            velocity.0 = Vec3::ZERO;
        }
        if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
            *ground = PlayerGroundState::default();
        }
        reset_player_fall_state(world, player);
        newengine_ulog_api::ulog::info!(
            "fps noclip enabled player={} collision=off gravity=off movement='view-yaw + vertical-axis'",
            player.stable_u64()
        );
    } else {
        if let Some(body) = current.saved_body {
            ensure_physics_body(world, player, body);
        }
        let traversal = world
            .resource::<FpsCharacterTraversalState>()
            .copied()
            .unwrap_or_default()
            .with_mode(FpsCharacterTraversalMode::Standard);
        world.insert_resource(traversal);
        let _ = world.insert(
            player,
            FpsNoClipState {
                enabled: false,
                saved_body: None,
                last_toggle_source_frame: current.last_toggle_source_frame,
            },
        );
        if let Some(velocity) = world.get_mut::<Velocity>(player) {
            velocity.0.y = 0.0;
        }
        if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
            *ground = PlayerGroundState::default();
        }
        reset_player_fall_state(world, player);
        newengine_ulog_api::ulog::info!(
            "fps noclip disabled player={} collision=restored gravity=restored",
            player.stable_u64()
        );
    }
    let traversal_event = if enabled {
        "character.traversal.noclip.enabled"
    } else {
        "character.traversal.noclip.disabled"
    };
    if let Err(error) = newengine_engine_runtime::gameplay::emit_animation_state(
        world,
        player,
        "character.traversal",
        traversal_event,
        serde_json::json!({"enabled": enabled}),
    ) {
        newengine_ulog_api::ulog::warn!(
            "fps traversal animation semantic publish failed player={} enabled={} err='{}'",
            player.stable_u64(),
            enabled,
            error
        );
    }
    true
}

#[inline]
pub fn toggle_fps_noclip(world: &mut World, player: EntityId) -> bool {
    let enabled = !fps_noclip_enabled(world, player);
    set_fps_noclip(world, player, enabled)
}

/// Consume a semantic toggle edge at most once for one sampled input source frame.
/// Fixed-step catch-up may execute several gameplay ticks against the same command frame.
pub fn toggle_fps_noclip_once_for_source_frame(
    world: &mut World,
    player: EntityId,
    source_frame: u64,
) -> bool {
    if world
        .get::<FpsNoClipState>(player)
        .and_then(|state| state.last_toggle_source_frame)
        == Some(source_frame)
    {
        return false;
    }
    let changed = toggle_fps_noclip(world, player);
    if let Some(state) = world.get_mut::<FpsNoClipState>(player) {
        state.last_toggle_source_frame = Some(source_frame);
    }
    changed
}

/// FPS-owned noclip locomotion. The player physics body is removed while enabled.
/// Service-backed physics deliberately skips the generic ECS velocity integrator, so
/// noclip advances the authoritative transform itself while still publishing Velocity.
/// Horizontal motion follows view yaw; Q/E (generic Y axis) provide vertical travel.
pub fn step_fps_noclip_motion(world: &mut World, dt: f32) {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let players = world
        .query2_ids::<CharacterMotor, MotorInput>()
        .filter(|player| fps_noclip_enabled(world, *player))
        .collect::<Vec<_>>();

    for player in players {
        let motor = world
            .get::<CharacterMotor>(player)
            .copied()
            .unwrap_or_default();
        let input = world.get::<MotorInput>(player).copied().unwrap_or_default();
        let move_axis = Vec3::new(
            if input.move_axis.x.is_finite() {
                input.move_axis.x
            } else {
                0.0
            },
            if input.move_axis.y.is_finite() {
                input.move_axis.y
            } else {
                0.0
            },
            if input.move_axis.z.is_finite() {
                input.move_axis.z
            } else {
                0.0
            },
        );
        let forward_sign = if motor.forward_sign_z.is_finite() && motor.forward_sign_z != 0.0 {
            motor.forward_sign_z.signum()
        } else {
            -1.0
        };
        let local = Vec3::new(move_axis.x, move_axis.y, move_axis.z * forward_sign);
        let direction = local.normalize_or_zero();
        let speed_mul = if input.speed_mul.is_finite() && input.speed_mul > 0.0 {
            input.speed_mul
        } else {
            1.0
        };
        let move_speed = if motor.move_speed.is_finite() && motor.move_speed >= 0.0 {
            motor.move_speed
        } else {
            0.0
        };
        let flight_velocity = if direction.length_squared() > 1.0e-8 {
            Quat::from_rotation_y(motor.yaw) * direction * (move_speed * speed_mul)
        } else {
            Vec3::ZERO
        };

        // NoClip owns transform integration directly. Do not publish flight speed through the
        // ordinary gameplay `Velocity` component: semantic locomotion consumes that component
        // to select idle/walk/run/fall presentation. A flying seated character must therefore
        // remain locomotion-neutral while the private flight vector advances its transform.
        let _ = world.insert(player, Velocity(Vec3::ZERO));

        // In ServiceBackend mode PhysicsBodyDesc entities are integrated by the provider and
        // the generic SimStage::Physics velocity integrator is intentionally skipped. Noclip
        // removes PhysicsBodyDesc, therefore this mode must own world-space translation here.
        if dt > 0.0 && flight_velocity.is_finite() && flight_velocity.length_squared() > 1.0e-12 {
            if let Some((world_pos, world_rot)) = read_entity_world_pose_local_chain(world, player)
            {
                write_entity_local_from_world_pose_local_chain(
                    world,
                    player,
                    world_pos + flight_velocity * dt,
                    world_rot,
                );
                let _ = world.insert(player, TransformDirty);
            }
        }

        if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
            *ground = PlayerGroundState::default();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_engine_runtime::gameplay::{
        spawn_default_player, update_player_animation_states, PhysicsBodyDesc,
        PlayerAnimationState, PlayerLocomotionAnimation,
    };

    #[test]
    fn noclip_is_rejected_when_bound_character_has_no_authored_noclip_pose() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "noclip-unsupported", Vec3::ZERO);
        let original = world.get::<PhysicsBodyDesc>(player).copied().expect("body");
        let _ = world.insert(
            player,
            newengine_engine_runtime::gameplay::PlayerAuthoredAnimationCapabilities {
                noclip: false,
                ..Default::default()
            },
        );

        assert!(!set_fps_noclip(&mut world, player, true));
        assert!(!fps_noclip_enabled(&world, player));
        assert_eq!(
            world.get::<PhysicsBodyDesc>(player).copied(),
            Some(original)
        );
    }

    #[test]
    fn noclip_removes_and_restores_player_physics_body() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "noclip-player", Vec3::ZERO);
        let original = world.get::<PhysicsBodyDesc>(player).copied().expect("body");

        assert!(set_fps_noclip(&mut world, player, true));
        assert!(fps_noclip_enabled(&world, player));
        assert!(world.get::<PhysicsBodyDesc>(player).is_none());

        assert!(set_fps_noclip(&mut world, player, false));
        assert!(!fps_noclip_enabled(&world, player));
        assert_eq!(
            world.get::<PhysicsBodyDesc>(player).copied(),
            Some(original)
        );
    }

    #[test]
    fn noclip_flight_does_not_select_walk_or_run_animation() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "noclip-seated-flight", Vec3::ZERO);
        assert!(set_fps_noclip(&mut world, player, true));
        if let Some(input) = world.get_mut::<MotorInput>(player) {
            input.move_axis = Vec3::new(1.0, 0.0, 1.0);
            input.speed_mul = 1.0;
        }

        let before = read_entity_world_pose_local_chain(&world, player)
            .expect("player world pose before noclip flight")
            .0;
        step_fps_noclip_motion(&mut world, 1.0 / 60.0);
        update_player_animation_states(&mut world, 1.0 / 60.0);
        let after = read_entity_world_pose_local_chain(&world, player)
            .expect("player world pose after noclip flight")
            .0;
        assert_ne!(before, after, "noclip must still translate the character");
        assert_eq!(
            world.get::<Velocity>(player).copied().unwrap_or_default().0,
            Vec3::ZERO
        );
        let animation = world
            .get::<PlayerAnimationState>(player)
            .copied()
            .expect("player animation state");
        assert!(
            !matches!(
                animation.locomotion,
                PlayerLocomotionAnimation::Walk
                    | PlayerLocomotionAnimation::Run
                    | PlayerLocomotionAnimation::Sprint
            ),
            "noclip flight must never publish walking/running locomotion presentation: {:?}",
            animation.locomotion
        );
    }
    #[test]
    fn noclip_uses_vertical_motor_axis_without_gravity() {
        let mut world = World::new();
        let player = spawn_default_player(&mut world, None, "noclip-vertical", Vec3::ZERO);
        assert!(set_fps_noclip(&mut world, player, true));
        if let Some(input) = world.get_mut::<MotorInput>(player) {
            input.move_axis = Vec3::new(0.0, 1.0, 0.0);
            input.speed_mul = 1.0;
        }
        let before = read_entity_world_pose_local_chain(&world, player)
            .expect("player world pose before noclip step")
            .0;
        step_fps_noclip_motion(&mut world, 1.0 / 60.0);
        assert_eq!(
            world.get::<Velocity>(player).copied().unwrap_or_default().0,
            Vec3::ZERO,
            "noclip flight must not leak into locomotion velocity semantics"
        );
        let after = read_entity_world_pose_local_chain(&world, player)
            .expect("player world pose after noclip step")
            .0;
        assert!(
            after.y > before.y,
            "noclip must integrate the transform directly when service physics owns the frame"
        );
    }
}
