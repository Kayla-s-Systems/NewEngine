#[derive(Clone, Copy, Debug)]
struct WeaponArmIkRig {
    chest: usize,
    right_shoulder: usize,
    right_elbow: usize,
    right_wrist: usize,
    right_palm: usize,
    right_hand_prop: Option<usize>,
    right_hand_prop_attachment: Option<usize>,
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
        right_hand_prop: find("r_hand_prop"),
        right_hand_prop_attachment: find("r_hand_prop_attachment"),
        left_shoulder: find("l_shoulder")?,
        left_elbow: find("l_elbow")?,
        left_wrist: find("l_wrist")?,
        left_palm: find("l_palm")?,
    })
}

fn native_firing_handle_anchor(rig: &WeaponArmIkRig, frames: &[Mat4]) -> Option<Vec3> {
    const MAX_PROP_TO_PALM_DISTANCE_M: f32 = 0.12;
    let palm = frames.get(rig.right_palm)?.transform_point3(Vec3::ZERO);
    if !palm.is_finite() {
        return None;
    }
    for candidate in [rig.right_hand_prop_attachment, rig.right_hand_prop] {
        let Some(index) = candidate else { continue };
        let Some(frame) = frames.get(index) else { continue };
        let anchor = frame.transform_point3(Vec3::ZERO);
        let delta = anchor - palm;
        if anchor.is_finite()
            && delta.is_finite()
            && delta.length_squared() <= MAX_PROP_TO_PALM_DISTANCE_M.powi(2)
        {
            return Some(anchor);
        }
    }
    // The original long-gun graph is firing-hand owned. A missing/stale prop constraint therefore
    // falls back to the actual animated palm, never to a stock-owned weapon solve that would make
    // the arm chase the gun.
    Some(palm)
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

/// Native long-gun overlays own the firing arm. The weapon is pinned to that authored firing-hand
/// contact; stock/shoulder are soft constraints and only the support arm receives bounded IK.
/// Reload manipulation can temporarily release support-hand correction.
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
    recoil_yaw_radians: f32,
    obstruction_alpha: f32,
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
    let handle_anchor = native_firing_handle_anchor(rig, frames);
    let authored_left_palm = frames
        .get(rig.left_palm)
        .map(|frame| frame.transform_point3(Vec3::ZERO))
        .filter(|position| position.is_finite());
    let contract = crate::weapon_grip::weapon_ready_solve_contract_presented(
        presentation,
        chest,
        right_shoulder,
        left_shoulder,
        view_forward_model,
        aim_alpha,
        recoil_alpha,
        recoil_yaw_radians,
    )
    .and_then(|contract| {
        crate::weapon_grip::weapon_ready_contract_with_contacts(
            presentation,
            contract,
            handle_anchor,
            authored_left_palm,
            aim_alpha,
            obstruction_alpha,
        )
    })
    .ok_or("weapon ReadyHold could not resolve authored contact constraint")?;
    // `native_firing_handle_anchor` falls back to the physical palm, so a valid humanoid rifle rig
    // always has a firing-hand master. Keep the fallback branch only for malformed/incomplete rigs.
    let solve_right_hand = support_right_hand && handle_anchor.is_none();
    let right_target =
        crate::weapon_grip::weapon_ready_right_palm_position(presentation, contract.root);
    let left_target =
        crate::weapon_grip::weapon_ready_left_palm_position(presentation, contract.root);

    let limited_target = |current: Vec3, target: Vec3, max_delta: f32| {
        let delta = target - current;
        let distance = delta.length();
        if distance.is_finite() && distance > max_delta && distance > 1.0e-6 {
            current + delta * (max_delta / distance)
        } else {
            target
        }
    };
    let current_right_palm = frames[rig.right_palm].transform_point3(Vec3::ZERO);
    let current_left_palm = frames[rig.left_palm].transform_point3(Vec3::ZERO);
    let right_target = limited_target(current_right_palm, right_target, 0.07);
    let left_target = limited_target(current_left_palm, left_target, 0.10);

    if solve_right_hand {
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
    if solve_right_hand {
        orient_wrist_for_palm_basis(
            skeleton,
            pose,
            frames,
            rig.right_wrist,
            rig.right_palm,
            crate::weapon_grip::weapon_ready_right_palm_rotation(presentation, contract.root),
            24.0_f32.to_radians(),
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
            30.0_f32.to_radians(),
        )?;
    }

    rebuild_model_joint_frames(skeleton, source_to_model, pose, frames)?;
    let right_error = if let Some(anchor) = handle_anchor {
        (crate::weapon_grip::weapon_handle_position(presentation, contract.root) - anchor).length()
    } else if solve_right_hand {
        (frames[rig.right_palm].transform_point3(Vec3::ZERO) - right_target).length()
    } else {
        0.0
    };
    let left_error = if support_left_hand {
        (frames[rig.left_palm].transform_point3(Vec3::ZERO) - left_target).length()
    } else {
        0.0
    };
    // Stock/shoulder is intentionally a soft angular constraint. It must not be promoted to a
    // hard IK failure because different authored body proportions legitimately leave a few cm of
    // stock compression/clearance. Hand contact residual is the hard invariant.
    let error = right_error.max(left_error);
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
