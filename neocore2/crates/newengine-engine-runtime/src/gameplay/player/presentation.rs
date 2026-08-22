use super::*;
use newengine_math::Mat4;
use newengine_transform::read_entity_world_pose_local_chain;

#[inline]
fn exp_follow_scalar(current: f32, target: f32, dt: f32, time_constant: f32) -> f32 {
    if !current.is_finite() || !target.is_finite() {
        return target;
    }
    if !(dt.is_finite() && dt > 0.0) || !(time_constant.is_finite() && time_constant > 0.0) {
        return target;
    }
    let alpha = (1.0 - (-dt / time_constant).exp()).clamp(0.0, 1.0);
    current + (target - current) * alpha
}

#[inline]
fn stabilize_player_render_position(
    world: &World,
    player: EntityId,
    raw_position: Vec3,
    previous: Option<PlayerRenderPose>,
    render_dt: f32,
    teleported: bool,
) -> Vec3 {
    if teleported || !raw_position.is_finite() || !(render_dt.is_finite() && render_dt > 0.0) {
        return raw_position;
    }
    let Some(previous) = previous.filter(|pose| pose.position.is_finite()) else {
        return raw_position;
    };

    // Contact solvers can make a grounded capsule alternate by a few millimetres around the
    // support plane. Feeding those corrections directly into both the avatar and possessed
    // camera makes the whole world appear to tremble even though fixed-step interpolation is
    // otherwise correct. Stabilize the shared presentation pose once, before either consumer.
    let grounded = world
        .get::<PlayerGroundState>(player)
        .map(|ground| ground.grounded && ground.walkable)
        .unwrap_or(false);
    if !grounded {
        return raw_position;
    }

    let mut stabilized = raw_position;
    // Vertical support-plane corrections are the dominant source of camera bob. A short
    // render-space time constant removes 60 Hz solver chatter while remaining fast enough for
    // ramps, stairs and stance changes. Because this is the shared PlayerRenderPose, the model
    // and camera stay locked to the same presentation trajectory.
    stabilized.y = exp_follow_scalar(previous.position.y, raw_position.y, render_dt, 0.020);

    // When essentially stationary, also reject sub-centimetre lateral contact corrections.
    // Never filter actual locomotion: once horizontal velocity or displacement is meaningful,
    // X/Z remain the exact fixed-step interpolation result.
    let horizontal_speed = world
        .get::<newengine_sim::Velocity>(player)
        .map(|velocity| Vec2::new(velocity.0.x, velocity.0.z).length())
        .filter(|speed| speed.is_finite())
        .unwrap_or(0.0);
    let lateral_delta = Vec2::new(
        raw_position.x - previous.position.x,
        raw_position.z - previous.position.z,
    );
    if horizontal_speed <= 0.20 && lateral_delta.length() <= 0.012 {
        stabilized.x = exp_follow_scalar(previous.position.x, raw_position.x, render_dt, 0.016);
        stabilized.z = exp_follow_scalar(previous.position.z, raw_position.z, render_dt, 0.016);
    }
    stabilized
}

/// Captures the authoritative player pose after one fixed gameplay/physics tick.
/// The pair is intentionally kept one tick deep for standard accumulator interpolation.
pub fn capture_player_fixed_poses(world: &mut World, fixed_tick: u64) {
    let players = world
        .query::<PlayerActor>()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    for player in players {
        let Some((position, rotation)) = read_entity_world_pose_local_chain(world, player) else {
            continue;
        };
        let rotation = rotation.normalize_or_identity();
        let mut history = world
            .get::<PlayerFixedPoseHistory>(player)
            .copied()
            .unwrap_or_default();
        if !history.initialized {
            history.previous_position = position;
            history.previous_rotation = rotation;
            history.current_position = position;
            history.current_rotation = rotation;
            history.current_fixed_tick = fixed_tick;
            history.initialized = true;
        } else if history.current_fixed_tick != fixed_tick {
            history.previous_position = history.current_position;
            history.previous_rotation = history.current_rotation;
            history.current_position = position;
            history.current_rotation = rotation;
            history.current_fixed_tick = fixed_tick;
        } else {
            // Multiple writers within the same fixed tick update only the current endpoint.
            history.current_position = position;
            history.current_rotation = rotation;
        }
        let _ = world.insert(player, history);
    }
}

/// Publishes a non-authoritative render pose from the fixed-step accumulator remainder.
/// Teleports snap rather than interpolating through the level.
pub fn publish_player_render_poses(world: &mut World, fixed_alpha: f32, render_dt: f32) {
    let alpha = if fixed_alpha.is_finite() {
        fixed_alpha.clamp(0.0, 0.999_999)
    } else {
        0.0
    };
    let players = world
        .query::<PlayerActor>()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    for player in players {
        let Some((simulation_position, simulation_rotation)) =
            read_entity_world_pose_local_chain(world, player)
        else {
            continue;
        };
        let simulation_rotation = simulation_rotation.normalize_or_identity();
        let mut history = world
            .get::<PlayerFixedPoseHistory>(player)
            .copied()
            .unwrap_or_default();
        if !history.initialized {
            history.previous_position = simulation_position;
            history.previous_rotation = simulation_rotation;
            history.current_position = simulation_position;
            history.current_rotation = simulation_rotation;
            history.initialized = true;
            let _ = world.insert(player, history);
        }

        let delta = history.current_position - history.previous_position;
        let teleported = !delta.is_finite() || delta.length_squared() > 24.0 * 24.0;
        let (raw_position, rotation) = if teleported {
            (simulation_position, simulation_rotation)
        } else {
            (
                history
                    .previous_position
                    .lerp(history.current_position, alpha),
                history
                    .previous_rotation
                    .normalize_or_identity()
                    .slerp(history.current_rotation.normalize_or_identity(), alpha)
                    .normalize_or_identity(),
            )
        };
        let previous_render_pose = world.get::<PlayerRenderPose>(player).copied();
        let position = stabilize_player_render_position(
            world,
            player,
            raw_position,
            previous_render_pose,
            render_dt,
            teleported,
        );
        let _ = world.insert(
            player,
            PlayerRenderPose {
                position,
                rotation,
                simulation_position,
                simulation_rotation,
                fixed_alpha: alpha,
                source_fixed_tick: history.current_fixed_tick,
            },
        );
    }
}

/// Returns a presentation-space model matrix for any visual entity owned by a player.
/// The ECS `GlobalTransform` remains simulation-authoritative; only draw packets are adjusted.
pub fn player_render_model_matrix(world: &World, entity: EntityId, simulation_model: Mat4) -> Mat4 {
    let owner = world
        .get::<PlayerVisualPart>(entity)
        .map(|part| part.owner)
        .or_else(|| {
            world
                .get::<super::super::PlayerSkinBinding>(entity)
                .map(|skin| skin.owner)
        });
    let Some(owner) = owner else {
        return simulation_model;
    };
    let Some(render_pose) = world.get::<PlayerRenderPose>(owner).copied() else {
        return simulation_model;
    };
    if !render_pose.position.is_finite()
        || !render_pose.simulation_position.is_finite()
        || !render_pose.rotation.is_finite()
        || !render_pose.simulation_rotation.is_finite()
    {
        return simulation_model;
    }
    let render_root = Mat4::from_scale_rotation_translation(
        Vec3::ONE,
        render_pose.rotation.normalize_or_identity(),
        render_pose.position,
    );
    let simulation_root = Mat4::from_scale_rotation_translation(
        Vec3::ONE,
        render_pose.simulation_rotation.normalize_or_identity(),
        render_pose.simulation_position,
    );
    render_root * simulation_root.inverse() * simulation_model
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_transform::Transform;

    #[test]
    fn fixed_pose_interpolation_is_monotonic_and_does_not_mutate_simulation_transform() {
        let mut world = World::new();
        let player = world.spawn();
        let visual = world.spawn();
        let _ = world.insert(player, PlayerActor);
        let _ = world.insert(player, Transform::default());
        let _ = world.insert(
            visual,
            PlayerVisualPart {
                owner: player,
                part_index: 0,
                kind: PlayerVisualKind::FallbackCapsule,
                material_slot: "test".to_owned(),
            },
        );
        capture_player_fixed_poses(&mut world, 1);
        world.get_mut::<Transform>(player).unwrap().position = Vec3::new(2.0, 0.0, 0.0);
        capture_player_fixed_poses(&mut world, 2);
        publish_player_render_poses(&mut world, 0.25, 1.0 / 144.0);
        let pose = world.get::<PlayerRenderPose>(player).copied().unwrap();
        assert!((pose.position.x - 0.5).abs() < 1.0e-6);
        assert!((world.get::<Transform>(player).unwrap().position.x - 2.0).abs() < 1.0e-6);

        let raw = Mat4::from_translation(Vec3::new(2.0, 1.0, 0.0));
        let rendered = player_render_model_matrix(&world, visual, raw);
        let rendered_origin = rendered.transform_point3(Vec3::ZERO);
        assert!((rendered_origin - Vec3::new(0.5, 1.0, 0.0)).length() < 1.0e-5);
    }

    #[test]
    fn grounded_presentation_damps_micro_solver_chatter_for_camera_and_avatar() {
        let mut world = World::new();
        let player = world.spawn();
        let _ = world.insert(player, PlayerActor);
        let _ = world.insert(player, Transform::default());
        let _ = world.insert(
            player,
            PlayerGroundState {
                grounded: true,
                walkable: true,
                ..PlayerGroundState::default()
            },
        );
        let _ = world.insert(player, newengine_sim::Velocity(Vec3::ZERO));
        capture_player_fixed_poses(&mut world, 1);
        publish_player_render_poses(&mut world, 0.5, 1.0 / 144.0);
        let baseline = world.get::<PlayerRenderPose>(player).copied().unwrap();

        world.get_mut::<Transform>(player).unwrap().position = Vec3::new(0.004, 0.006, -0.003);
        capture_player_fixed_poses(&mut world, 2);
        publish_player_render_poses(&mut world, 0.999, 1.0 / 144.0);
        let stabilized = world.get::<PlayerRenderPose>(player).copied().unwrap();
        let raw = Vec3::new(0.004, 0.006, -0.003) * 0.999;

        assert!((stabilized.position.y - baseline.position.y).abs() < raw.y.abs());
        assert!((stabilized.position.x - baseline.position.x).abs() < raw.x.abs());
        assert!((stabilized.position.z - baseline.position.z).abs() < raw.z.abs());
    }

    #[test]
    fn grounded_presentation_does_not_lag_real_horizontal_locomotion() {
        let mut world = World::new();
        let player = world.spawn();
        let _ = world.insert(player, PlayerActor);
        let _ = world.insert(player, Transform::default());
        let _ = world.insert(
            player,
            PlayerGroundState {
                grounded: true,
                walkable: true,
                ..PlayerGroundState::default()
            },
        );
        let _ = world.insert(player, newengine_sim::Velocity(Vec3::new(3.0, 0.0, 0.0)));
        capture_player_fixed_poses(&mut world, 1);
        publish_player_render_poses(&mut world, 0.0, 1.0 / 144.0);
        world.get_mut::<Transform>(player).unwrap().position = Vec3::new(0.05, 0.0, 0.0);
        capture_player_fixed_poses(&mut world, 2);
        publish_player_render_poses(&mut world, 0.5, 1.0 / 144.0);
        let pose = world.get::<PlayerRenderPose>(player).copied().unwrap();
        assert!((pose.position.x - 0.025).abs() < 1.0e-6);
    }

    #[test]
    fn airborne_presentation_bypasses_ground_stabilizer() {
        let mut world = World::new();
        let player = world.spawn();
        let _ = world.insert(player, PlayerActor);
        let _ = world.insert(player, Transform::default());
        capture_player_fixed_poses(&mut world, 1);
        publish_player_render_poses(&mut world, 0.0, 1.0 / 144.0);
        world.get_mut::<Transform>(player).unwrap().position = Vec3::new(0.003, 0.004, 0.002);
        capture_player_fixed_poses(&mut world, 2);
        publish_player_render_poses(&mut world, 0.5, 1.0 / 144.0);
        let pose = world.get::<PlayerRenderPose>(player).copied().unwrap();
        assert!((pose.position - Vec3::new(0.0015, 0.002, 0.001)).length() < 1.0e-6);
    }
}
