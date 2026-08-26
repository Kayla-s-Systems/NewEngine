/// Current gameplay view direction converted into avatar/model-local space. Full-body first
/// person and explicit third-person aim use this for both rendered rifle and arm IK, so the weapon
/// and visible hands cannot diverge from the gameplay view axis.
pub(crate) fn player_rifle_view_forward_model(
    world: &newengine_ecs::World,
    player: EntityId,
) -> Option<Vec3> {
    let visual_root = world
        .get::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)?
        .visual_root
        .filter(|entity| world.exists(*entity))?;
    let (_, visual_rotation) =
        newengine_transform::read_entity_world_pose_local_chain(world, visual_root)?;

    let active_camera = world
        .resource::<newengine_scene::SceneState>()
        .and_then(|state| state.active_camera.or(state.root));
    let camera_rot_offset = active_camera
        .and_then(|camera| world.get::<newengine_sim::FollowTargetCameraController>(camera))
        .filter(|controller| controller.target == player)
        .map(|controller| controller.rot_offset)
        .unwrap_or(Quat::IDENTITY)
        .normalize_or_identity();
    let view_rotation = world
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
    let forward_ws = (view_rotation * -Vec3::Z).normalize_or_zero();
    let forward_model = visual_rotation.normalize_or_identity().inverse() * forward_ws;
    (forward_model.is_finite() && forward_model.length_squared() > 1.0e-8)
        .then_some(forward_model.normalize())
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
        // Naughty Dog prop-attachment joints can be animation/constraint targets rather than
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
/// both shoulders: Naughty Dog `spined` axes are not body-forward/body-up, so a stable body frame
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

/// Stable right-hand weapon grip in player-model local space.
pub(crate) fn player_right_hand_prop_frame(
    world: &newengine_ecs::World,
    player: EntityId,
) -> Option<Mat4> {
    let authored_equipment_contact = world
        .get::<PlayerAnimationRuntimeBinding>(player)
        .is_some_and(|binding| {
            binding.equipment_ready_pose.is_some()
                || binding.equipment_aim_pose.is_some()
                || binding.equipment_reload_pose.is_some()
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
    const EYE_FORWARD_CLEARANCE_M: f32 = 0.055;
    let players = world
        .query::<PlayerAnimationRuntimeBinding>()
        .map(|(player, _)| player)
        .collect::<Vec<_>>();

    for player in players {
        let eye_center_model = {
            let Some(binding) = world.get::<PlayerAnimationRuntimeBinding>(player) else {
                continue;
            };
            if let Some(eyes) = binding.eye_contract.as_ref() {
                let frame_at = |index: usize| {
                    binding
                        .joint_frames_scratch
                        .get(index)
                        .copied()
                        .or_else(|| binding.bind_joint_frames.get(index).copied())
                };
                match (frame_at(eyes.left), frame_at(eyes.right)) {
                    (Some(left), Some(right)) => {
                        let left = left.transform_point3(Vec3::ZERO);
                        let right = right.transform_point3(Vec3::ZERO);
                        ((left + right) * 0.5)
                            .is_finite()
                            .then_some((left + right) * 0.5)
                    }
                    _ => None,
                }
            } else {
                let anchor = binding.skeleton.anchors.eye.as_str();
                let frame = binding
                    .skeleton
                    .joints
                    .iter()
                    .position(|joint| joint.name == anchor)
                    .and_then(|index| {
                        binding
                            .joint_frames_scratch
                            .get(index)
                            .copied()
                            .or_else(|| binding.bind_joint_frames.get(index).copied())
                    });
                frame
                    .map(|frame| frame.transform_point3(Vec3::ZERO))
                    .filter(|position| position.is_finite())
            }
        };
        let Some(eye_center_model) = eye_center_model else {
            continue;
        };
        let Some(visual_root) = world
            .get::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)
            .and_then(|binding| binding.visual_root)
            .filter(|entity| world.exists(*entity))
        else {
            continue;
        };
        let Some((visual_position, visual_rotation)) =
            newengine_transform::read_entity_world_pose_local_chain(world, visual_root)
        else {
            continue;
        };
        let eye_center_ws =
            visual_position + visual_rotation.normalize_or_identity() * eye_center_model;
        if !eye_center_ws.is_finite() {
            continue;
        }
        let _ = world.insert(
            player,
            newengine_engine_runtime::gameplay::PlayerFirstPersonCameraAnchor {
                eye_center_ws,
                forward_clearance: EYE_FORWARD_CLEARANCE_M,
            },
        );
    }
}
