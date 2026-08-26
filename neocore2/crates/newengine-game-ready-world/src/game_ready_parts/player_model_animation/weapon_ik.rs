#[derive(Clone, Copy, Debug)]
struct WeaponArmIkRig {
    chest: usize,
    right_shoulder: usize,
    right_elbow: usize,
    right_wrist: usize,
    right_palm: usize,
    left_shoulder: usize,
    left_elbow: usize,
    left_wrist: usize,
    left_palm: usize,
}

fn build_weapon_arm_ik_rig(skeleton: &ModelSkeletonMetadata) -> Option<WeaponArmIkRig> {
    let find = |name: &str| skeleton.joints.iter().position(|joint| joint.name == name);
    Some(WeaponArmIkRig {
        chest: find("spined")?,
        right_shoulder: find("r_shoulder")?,
        right_elbow: find("r_elbow")?,
        right_wrist: find("r_wrist")?,
        right_palm: find("r_palm")?,
        left_shoulder: find("l_shoulder")?,
        left_elbow: find("l_elbow")?,
        left_wrist: find("l_wrist")?,
        left_palm: find("l_palm")?,
    })
}

fn rebuild_model_joint_frames(
    skeleton: &ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    pose: &[JointLocalPose],
    frames: &mut Vec<Mat4>,
) -> Result<(), String> {
    frames.clear();
    build_model_joint_frames_from_local_pose(skeleton, source_to_model, pose, frames)
}

fn rotate_pose_joint_toward(
    skeleton: &ModelSkeletonMetadata,
    pose: &mut [JointLocalPose],
    frames: &[Mat4],
    joint_index: usize,
    end_effector_index: usize,
    target: Vec3,
    correction_weight: f32,
) -> Result<(), String> {
    let joint_frame = *frames
        .get(joint_index)
        .ok_or_else(|| format!("rifle IK joint frame missing index={joint_index}"))?;
    let end_frame = *frames
        .get(end_effector_index)
        .ok_or_else(|| format!("rifle IK end frame missing index={end_effector_index}"))?;
    let joint_position = joint_frame.transform_point3(Vec3::ZERO);
    let end_position = end_frame.transform_point3(Vec3::ZERO);
    let to_end = end_position - joint_position;
    let to_target = target - joint_position;
    if !to_end.is_finite()
        || !to_target.is_finite()
        || to_end.length_squared() <= 1.0e-10
        || to_target.length_squared() <= 1.0e-10
    {
        return Ok(());
    }

    let full_delta =
        Quat::from_rotation_arc(to_end.normalize(), to_target.normalize()).normalize_or_identity();
    let correction_weight = if correction_weight.is_finite() {
        correction_weight.clamp(0.0, 1.0)
    } else {
        0.5
    };
    let delta = Quat::IDENTITY
        .slerp(full_delta, correction_weight)
        .normalize_or_identity();
    let (_, joint_global_rotation, _) = joint_frame.to_scale_rotation_translation();
    let parent_global_rotation = skeleton.joints[joint_index]
        .parent_index
        .and_then(|parent| frames.get(parent as usize).copied())
        .map(|frame| frame.to_scale_rotation_translation().1)
        .unwrap_or(Quat::IDENTITY);
    let desired_global = (delta * joint_global_rotation).normalize_or_identity();
    let local_rotation =
        (parent_global_rotation.inverse() * desired_global).normalize_or_identity();
    let local = pose
        .get_mut(joint_index)
        .ok_or_else(|| format!("rifle IK local pose missing index={joint_index}"))?;
    local.rotation = [
        local_rotation.x,
        local_rotation.y,
        local_rotation.z,
        local_rotation.w,
    ];
    Ok(())
}

fn solve_two_bone_arm_with_pole(
    skeleton: &ModelSkeletonMetadata,
    pose: &mut [JointLocalPose],
    frames: &mut Vec<Mat4>,
    source_to_model: [f32; 16],
    shoulder: usize,
    elbow: usize,
    palm: usize,
    target: Vec3,
    pole: Vec3,
) -> Result<(), String> {
    rebuild_model_joint_frames(skeleton, source_to_model, pose, frames)?;
    let shoulder_position = frames
        .get(shoulder)
        .copied()
        .ok_or("rifle IK shoulder frame missing")?
        .transform_point3(Vec3::ZERO);
    let elbow_position = frames
        .get(elbow)
        .copied()
        .ok_or("rifle IK elbow frame missing")?
        .transform_point3(Vec3::ZERO);
    let palm_position = frames
        .get(palm)
        .copied()
        .ok_or("rifle IK palm frame missing")?
        .transform_point3(Vec3::ZERO);
    let upper_len = (elbow_position - shoulder_position).length();
    let lower_len = (palm_position - elbow_position).length();
    let raw_to_target = target - shoulder_position;
    let raw_distance = raw_to_target.length();
    if !upper_len.is_finite()
        || !lower_len.is_finite()
        || !raw_distance.is_finite()
        || upper_len <= 1.0e-5
        || lower_len <= 1.0e-5
        || raw_distance <= 1.0e-5
    {
        return Ok(());
    }

    let direction = raw_to_target / raw_distance;
    let min_reach = (upper_len - lower_len).abs() + 1.0e-4;
    let max_reach = (upper_len + lower_len - 1.0e-4).max(min_reach);
    let distance = raw_distance.clamp(min_reach, max_reach);
    let reachable_target = shoulder_position + direction * distance;

    let pole_vector = pole - shoulder_position;
    let mut bend_direction = pole_vector - direction * pole_vector.dot(direction);
    if bend_direction.length_squared() <= 1.0e-8 {
        let current_bend = elbow_position - shoulder_position;
        bend_direction = current_bend - direction * current_bend.dot(direction);
    }
    bend_direction = bend_direction.normalize_or_zero();
    if bend_direction.length_squared() <= 1.0e-8 {
        return Ok(());
    }

    let along = ((upper_len * upper_len - lower_len * lower_len + distance * distance)
        / (2.0 * distance))
        .clamp(0.0, upper_len);
    let height = (upper_len * upper_len - along * along).max(0.0).sqrt();
    let desired_elbow = shoulder_position + direction * along + bend_direction * height;

    // First orient the upper arm into the preferred elbow plane, then close the forearm onto the
    // palm target. No free CCD iterations remain, so the elbow cannot flip to another plane.
    rotate_pose_joint_toward(skeleton, pose, frames, shoulder, elbow, desired_elbow, 1.0)?;
    rebuild_model_joint_frames(skeleton, source_to_model, pose, frames)?;
    rotate_pose_joint_toward(skeleton, pose, frames, elbow, palm, reachable_target, 1.0)?;
    rebuild_model_joint_frames(skeleton, source_to_model, pose, frames)?;
    Ok(())
}

fn orient_wrist_for_palm_basis(
    skeleton: &ModelSkeletonMetadata,
    pose: &mut [JointLocalPose],
    frames: &[Mat4],
    wrist: usize,
    palm: usize,
    desired_palm_global: Quat,
    max_correction_radians: f32,
) -> Result<(), String> {
    let current_wrist = frames
        .get(wrist)
        .copied()
        .ok_or("rifle wrist frame missing")?
        .to_scale_rotation_translation()
        .1
        .normalize_or_identity();
    let palm_local = pose
        .get(palm)
        .ok_or("rifle palm local pose missing")?
        .rotation;
    let palm_local = Quat::from_xyzw(palm_local[0], palm_local[1], palm_local[2], palm_local[3])
        .normalize_or_identity();
    let desired_wrist = (desired_palm_global * palm_local.inverse()).normalize_or_identity();
    let dot = current_wrist.dot(desired_wrist).abs().clamp(0.0, 1.0);
    let angle = 2.0 * dot.acos();
    let weight = if angle.is_finite() && angle > max_correction_radians.max(1.0e-4) {
        (max_correction_radians / angle).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let limited = current_wrist
        .slerp(desired_wrist, weight)
        .normalize_or_identity();
    set_pose_joint_global_rotation(skeleton, pose, frames, wrist, limited)
}

fn set_pose_joint_global_rotation(
    skeleton: &ModelSkeletonMetadata,
    pose: &mut [JointLocalPose],
    frames: &[Mat4],
    joint_index: usize,
    desired_global: Quat,
) -> Result<(), String> {
    let parent_global = skeleton.joints[joint_index]
        .parent_index
        .and_then(|parent| frames.get(parent as usize).copied())
        .map(|frame| frame.to_scale_rotation_translation().1)
        .unwrap_or(Quat::IDENTITY)
        .normalize_or_identity();
    let local_rotation = (parent_global.inverse() * desired_global).normalize_or_identity();
    let local = pose
        .get_mut(joint_index)
        .ok_or_else(|| format!("rifle ready local pose missing index={joint_index}"))?;
    local.rotation = [
        local_rotation.x,
        local_rotation.y,
        local_rotation.z,
        local_rotation.w,
    ];
    Ok(())
}

/// ReadyHold keeps one stock/shoulder-owned weapon transform while native standing rifle overlays
/// provide authored upper-body style. Outside the authored reload manipulation window, bilateral
/// IK enforces the physical firing-hand and support-hand contacts without feeding either hand back
/// into weapon placement.
fn apply_equipped_weapon_support_ik(
    presentation: &newengine_engine_runtime::gameplay::WeaponPresentationDefinition,
    rig: Option<&WeaponArmIkRig>,
    skeleton: &ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    pose: &mut Vec<JointLocalPose>,
    frames: &mut Vec<Mat4>,
    view_forward_model: Option<Vec3>,
    aim_alpha: f32,
    recoil_alpha: f32,
    support_right_hand: bool,
    support_left_hand: bool,
) -> Result<Option<f32>, String> {
    let Some(rig) = rig else {
        return Ok(None);
    };
    rebuild_model_joint_frames(skeleton, source_to_model, pose, frames)?;
    let chest = *frames
        .get(rig.chest)
        .ok_or("weapon ReadyHold chest frame is unavailable")?;
    let right_shoulder = *frames
        .get(rig.right_shoulder)
        .ok_or("weapon ReadyHold right shoulder frame is unavailable")?;
    let left_shoulder = *frames
        .get(rig.left_shoulder)
        .ok_or("weapon ReadyHold left shoulder frame is unavailable")?;
    let contract = crate::weapon_grip::weapon_ready_solve_contract_presented(
        presentation,
        chest,
        right_shoulder,
        left_shoulder,
        view_forward_model,
        aim_alpha,
        recoil_alpha,
    )
    .ok_or("weapon ReadyHold could not resolve anatomical solve contract")?;
    let right_target =
        crate::weapon_grip::weapon_ready_right_palm_position(presentation, contract.root);
    let left_target =
        crate::weapon_grip::weapon_ready_left_palm_position(presentation, contract.root);

    if support_right_hand {
        solve_two_bone_arm_with_pole(
            skeleton,
            pose,
            frames,
            source_to_model,
            rig.right_shoulder,
            rig.right_elbow,
            rig.right_palm,
            right_target,
            contract.right_elbow_pole,
        )?;
    }
    if support_left_hand {
        solve_two_bone_arm_with_pole(
            skeleton,
            pose,
            frames,
            source_to_model,
            rig.left_shoulder,
            rig.left_elbow,
            rig.left_palm,
            left_target,
            contract.left_elbow_pole,
        )?;
    }

    // Wrist orientation is a separate constrained pass. The palm contact calibration supplies the
    // desired grip basis, while a maximum correction prevents the wrist from absorbing an entire
    // arm-plane mismatch as twist.
    rebuild_model_joint_frames(skeleton, source_to_model, pose, frames)?;
    if support_right_hand {
        orient_wrist_for_palm_basis(
            skeleton,
            pose,
            frames,
            rig.right_wrist,
            rig.right_palm,
            crate::weapon_grip::weapon_ready_right_palm_rotation(presentation, contract.root),
            35.0_f32.to_radians(),
        )?;
    }
    if support_left_hand {
        rebuild_model_joint_frames(skeleton, source_to_model, pose, frames)?;
        orient_wrist_for_palm_basis(
            skeleton,
            pose,
            frames,
            rig.left_wrist,
            rig.left_palm,
            crate::weapon_grip::weapon_ready_left_palm_rotation(presentation, contract.root),
            40.0_f32.to_radians(),
        )?;
    }

    rebuild_model_joint_frames(skeleton, source_to_model, pose, frames)?;
    let right_error = if support_right_hand {
        (frames[rig.right_palm].transform_point3(Vec3::ZERO) - right_target).length()
    } else {
        0.0
    };
    let left_error = if support_left_hand {
        (frames[rig.left_palm].transform_point3(Vec3::ZERO) - left_target).length()
    } else {
        0.0
    };
    let stock_error = (contract.stock_contact - contract.shoulder_pocket).length();
    let error = right_error.max(left_error).max(stock_error);
    if !error.is_finite() {
        return Err("weapon ReadyHold IK produced non-finite contact error".to_owned());
    }
    Ok(Some(error))
}

fn build_helper_mirror_pairs(skeleton: &ModelSkeletonMetadata) -> Vec<(usize, usize)> {
    use std::collections::HashMap;

    let by_name = skeleton
        .joints
        .iter()
        .enumerate()
        .map(|(index, joint)| (joint.name.as_str(), index))
        .collect::<HashMap<_, _>>();
    skeleton
        .joints
        .iter()
        .enumerate()
        .filter_map(|(helper_index, joint)| {
            let primary_name = joint.name.strip_suffix("_helper")?;
            let primary_index = *by_name.get(primary_name)?;
            (primary_index != helper_index).then_some((helper_index, primary_index))
        })
        .collect()
}

#[inline]
fn synchronize_helper_pose(pairs: &[(usize, usize)], pose: &mut [JointLocalPose]) {
    for &(helper_index, primary_index) in pairs {
        if helper_index < pose.len() && primary_index < pose.len() {
            pose[helper_index] = pose[primary_index];
        }
    }
}
