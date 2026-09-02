/// Current input-owned gameplay view rotation in world space. Weapon presentation and camera ADS
/// anchoring must share this exact orientation or the rendered sights and gameplay ray diverge.
pub(crate) fn player_rifle_view_rotation_world(
    world: &newengine_ecs::World,
    player: EntityId,
) -> Option<Quat> {
    let active_camera = world
        .resource::<newengine_scene::SceneState>()
        .and_then(|state| state.active_camera.or(state.root));
    let camera_rot_offset = active_camera
        .and_then(|camera| world.get::<newengine_sim::FollowTargetCameraController>(camera))
        .filter(|controller| controller.target == player)
        .map(|controller| controller.rot_offset)
        .unwrap_or(Quat::IDENTITY)
        .normalize_or_identity();
    let view_rotation_ws = world
        .get::<newengine_sim::CharacterMotor>(player)
        .map(|motor| {
            (Quat::from_euler(EulerRot::YXZ, motor.yaw, motor.pitch, 0.0) * camera_rot_offset)
                .normalize_or_identity()
        })
        .or_else(|| {
            active_camera
                .and_then(|camera| world.get::<newengine_sim::CameraRigComp>(camera))
                .map(|rig| rig.0.rotation.normalize_or_identity())
        })?;
    view_rotation_ws.is_finite().then_some(view_rotation_ws)
}

/// Current gameplay view rotation converted into avatar/model-local space. Full-body FPP uses
/// this complete frame rather than only a forward vector so authored camera-space weapon offsets
/// preserve lateral/up placement while yaw/pitch remain gameplay-owned.
pub(crate) fn player_rifle_view_rotation_model(
    world: &newengine_ecs::World,
    player: EntityId,
) -> Option<Quat> {
    let visual_root = world
        .get::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)?
        .visual_root
        .filter(|entity| world.exists(*entity))?;
    let (_, visual_rotation) =
        newengine_transform::read_entity_world_pose_local_chain(world, visual_root)?;
    let view_rotation_ws = player_rifle_view_rotation_world(world, player)?;
    let view_rotation_model = (visual_rotation.normalize_or_identity().inverse()
        * view_rotation_ws)
        .normalize_or_identity();
    view_rotation_model
        .is_finite()
        .then_some(view_rotation_model)
}

/// Current gameplay view direction converted into avatar/model-local space.
pub(crate) fn player_rifle_view_forward_model(
    world: &newengine_ecs::World,
    player: EntityId,
) -> Option<Vec3> {
    let view_rotation = player_rifle_view_rotation_model(world, player)?;
    let forward_model = (view_rotation * -Vec3::Z).normalize_or_zero();
    (forward_model.is_finite() && forward_model.length_squared() > 1.0e-8).then_some(forward_model)
}

fn player_prop_frame(
    world: &newengine_ecs::World,
    player: EntityId,
    candidates: &[&str],
) -> Option<Mat4> {
    let binding = world.get::<PlayerAnimationRuntimeBinding>(player)?;
    for candidate in candidates {
        let Some(index) = binding
            .skeleton
            .joints
            .iter()
            .position(|joint| joint.name == *candidate)
        else {
            continue;
        };
        if let Some(frame) = binding.joint_frames_scratch.get(index).copied() {
            return Some(frame);
        }
        if let Some(frame) = binding.bind_joint_frames.get(index).copied() {
            return Some(frame);
        }
    }
    None
}

const MAX_PROP_SOCKET_TO_HAND_DISTANCE: f32 = 0.12;

fn stable_hand_grip_frame(
    world: &newengine_ecs::World,
    player: EntityId,
    prop_candidates: &[&str],
    physical_candidates: &[&str],
) -> Option<Mat4> {
    let physical = player_prop_frame(world, player, physical_candidates)?;
    let Some(prop) = player_prop_frame(world, player, prop_candidates) else {
        return Some(physical);
    };
    let prop_position = prop.transform_point3(Vec3::ZERO);
    let physical_position = physical.transform_point3(Vec3::ZERO);
    let delta = prop_position - physical_position;
    if delta.is_finite() && delta.length_squared() <= MAX_PROP_SOCKET_TO_HAND_DISTANCE.powi(2) {
        Some(prop)
    } else {
        // source prop-attachment joints can be animation/constraint targets rather than
        // literal palm centers. A stale target may move far away from the hand; never drag an
        // equipped weapon there. Fall back to the animated palm/wrist frame.
        Some(physical)
    }
}

/// Physical left-hand frame used for support diagnostics. Weapon transform never depends on it.
pub(crate) fn player_left_hand_weapon_frame(
    world: &newengine_ecs::World,
    player: EntityId,
) -> Option<Mat4> {
    player_prop_frame(
        world,
        player,
        &["l_palm", "l_wrist", "DEF-hand.L", "hand.L"],
    )
}

/// Anatomical frames used by third-person rifle ReadyHold. The solve contract deliberately needs
/// both shoulders: authored `spined` axes are not body-forward/body-up, so a stable body frame
/// is reconstructed from the shoulder line instead of trusting the spine joint basis.
pub(crate) fn player_rifle_ready_body_frames(
    world: &newengine_ecs::World,
    player: EntityId,
) -> Option<(Mat4, Mat4, Mat4)> {
    let chest = player_prop_frame(
        world,
        player,
        &["spined", "DEF-spine.003", "spine_fk.003", "DEF-spine.004"],
    )?;
    let right_shoulder = player_prop_frame(
        world,
        player,
        &["r_shoulder", "DEF-upper_arm.R", "upper_arm.R"],
    )?;
    let left_shoulder = player_prop_frame(
        world,
        player,
        &["l_shoulder", "DEF-upper_arm.L", "upper_arm.L"],
    )?;
    Some((chest, right_shoulder, left_shoulder))
}

pub(crate) fn player_resolved_weapon_ready_root(
    world: &newengine_ecs::World,
    player: EntityId,
) -> Option<crate::weapon_grip::WeaponRootTransform> {
    world
        .get::<PlayerAnimationRuntimeBinding>(player)
        .and_then(|binding| binding.equipment_resolved_weapon_root)
}

/// Stable right-hand weapon grip in player-model local space.
pub(crate) fn player_right_hand_prop_frame(
    world: &newengine_ecs::World,
    player: EntityId,
) -> Option<Mat4> {
    let equipment_pose_family = world
        .get::<newengine_engine_runtime::gameplay::EquippedWeaponBinding>(player)
        .and_then(|equipped| {
            world
                .resource::<newengine_engine_runtime::gameplay::ItemCatalog>()?
                .get(equipped.item)?
                .weapon_class
                .clone()
        });
    let authored_equipment_contact = world
        .get::<PlayerAnimationRuntimeBinding>(player)
        .is_some_and(|binding| {
            binding.has_equipment_pose_for_family(equipment_pose_family.as_deref())
        });
    if authored_equipment_contact {
        stable_hand_grip_frame(
            world,
            player,
            &["r_hand_prop_attachment", "r_hand_prop"],
            &["r_palm", "r_wrist", "DEF-hand.R", "hand.R"],
        )
    } else {
        // Before the project character policy has installed its authored equipment clip, prop
        // channels are ordinary locomotion/constraint targets and are not a valid weapon socket.
        player_prop_frame(
            world,
            player,
            &["r_palm", "r_wrist", "DEF-hand.R", "hand.R"],
        )
    }
}

pub(crate) fn publish_player_first_person_camera_anchors(world: &mut newengine_ecs::World) {
    // FPP camera position is gameplay-owned, not a child of animated eye/head joints. Locomotion
    // may move the visible skull substantially; sampling that motion directly makes the camera
    // cross the face/torso shell during walk cycles. Use the render-interpolated actor position and
    // stance eye height. Camera runtime adds only a small body-owned forward/parallax offset.
    let players = world
        .query::<PlayerAnimationRuntimeBinding>()
        .filter_map(|(player, _)| {
            world
                .get::<newengine_engine_runtime::gameplay::PlayerActor>(player)
                .is_some()
                .then_some(player)
        })
        .collect::<Vec<_>>();

    for player in players {
        let actor_position = world
            .get::<newengine_engine_runtime::gameplay::PlayerRenderPose>(player)
            .copied()
            .filter(|pose| pose.position.is_finite())
            .map(|pose| pose.position)
            .or_else(|| {
                newengine_transform::read_entity_world_pose_local_chain(world, player)
                    .map(|pose| pose.0)
            });
        let Some(actor_position) = actor_position else {
            continue;
        };
        let body = world
            .get::<newengine_engine_runtime::gameplay::CharacterBody>(player)
            .copied()
            .unwrap_or_default()
            .sanitized();
        let stance_eye_height = world
            .get::<newengine_engine_runtime::gameplay::PlayerStanceState>(player)
            .map(|state| state.current_eye_height)
            .filter(|height| height.is_finite() && *height > 0.01)
            .unwrap_or(body.standing_eye_height);
        // Eye height is authored by project character/camera data. Do not re-fit the camera from
        // current model proportions: changing a model must not silently move the gameplay camera.
        let eye_height = stance_eye_height;
        // Forward eye clearance is project-authored camera data. The avatar provider publishes
        // only the resolved render-cadence anchor; it does not invent a game-specific camera offset.
        let face_forward_clearance = world
            .get::<newengine_engine_runtime::gameplay::PlayerCameraProfile>(player)
            .copied()
            .unwrap_or_default()
            .sanitized()
            .first_person_forward_clearance;
        let eye_center_ws = actor_position + Vec3::Y * eye_height;
        if !eye_center_ws.is_finite() {
            continue;
        }
        let _ = world.insert(
            player,
            newengine_engine_runtime::gameplay::PlayerFirstPersonCameraAnchor {
                eye_center_ws,
                ads_camera_position_ws: None,
                forward_clearance: face_forward_clearance,
            },
        );
    }
}
