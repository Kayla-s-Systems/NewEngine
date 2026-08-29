#[derive(Clone, Copy, Debug)]
struct WeaponArmIkRig {
    chest: usize,
    right_shoulder: usize,
    right_elbow: usize,
    right_wrist: usize,
    right_palm: usize,
    right_prop_attachment: Option<usize>,
    left_shoulder: usize,
    left_elbow: usize,
    left_wrist: usize,
    left_palm: usize,
    left_prop_attachment: Option<usize>,
}

fn build_weapon_arm_ik_rig(skeleton: &ModelSkeletonMetadata) -> Option<WeaponArmIkRig> {
    let find = |name: &str| skeleton.joints.iter().position(|joint| joint.name == name);
    Some(WeaponArmIkRig {
        chest: find("spined")?,
        right_shoulder: find("r_shoulder")?,
        right_elbow: find("r_elbow")?,
        right_wrist: find("r_wrist")?,
        right_palm: find("r_palm")?,
        right_prop_attachment: find("r_hand_prop_attachment"),
        left_shoulder: find("l_shoulder")?,
        left_elbow: find("l_elbow")?,
        left_wrist: find("l_wrist")?,
        left_palm: find("l_palm")?,
        left_prop_attachment: find("l_hand_prop_attachment"),
    })
}

fn rebuild_model_joint_frames(
    animation_runtime: &AnimationSkeletonRuntime,
    pose: &[JointLocalPose],
    frames: &mut Vec<Mat4>,
) -> Result<(), String> {
    animation_runtime.build_model_joint_frames_from_local_pose(pose, frames)
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
    animation_runtime: &AnimationSkeletonRuntime,
    pose: &mut [JointLocalPose],
    frames: &mut Vec<Mat4>,
    shoulder: usize,
    elbow: usize,
    palm: usize,
    target: Vec3,
    pole: Vec3,
) -> Result<(), String> {
    rebuild_model_joint_frames(animation_runtime, pose, frames)?;
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
    rebuild_model_joint_frames(animation_runtime, pose, frames)?;
    rotate_pose_joint_toward(skeleton, pose, frames, elbow, palm, reachable_target, 1.0)?;
    rebuild_model_joint_frames(animation_runtime, pose, frames)?;
    Ok(())
}

fn solve_arm_to_palm_contact(
    skeleton: &ModelSkeletonMetadata,
    animation_runtime: &AnimationSkeletonRuntime,
    pose: &mut [JointLocalPose],
    frames: &mut Vec<Mat4>,
    shoulder: usize,
    elbow: usize,
    wrist: usize,
    palm: usize,
    palm_target: Vec3,
    pole: Vec3,
    desired_palm_global: Quat,
    label: &str,
) -> Result<(), String> {
    rebuild_model_joint_frames(animation_runtime, pose, frames)?;
    let wrist_frame = *frames
        .get(wrist)
        .ok_or("rifle wrist frame missing")?;
    let palm_frame = *frames
        .get(palm)
        .ok_or("rifle palm frame missing")?;
    let wrist_position = wrist_frame.transform_point3(Vec3::ZERO);
    let palm_position = palm_frame.transform_point3(Vec3::ZERO);
    let wrist_rotation = wrist_frame
        .to_scale_rotation_translation()
        .1
        .normalize_or_identity();
    let palm_local = pose
        .get(palm)
        .ok_or("rifle palm local pose missing")?
        .rotation;
    let palm_local_rotation = Quat::from_xyzw(
        palm_local[0],
        palm_local[1],
        palm_local[2],
        palm_local[3],
    )
    .normalize_or_identity();
    let desired_wrist_rotation =
        (desired_palm_global * palm_local_rotation.inverse()).normalize_or_identity();
    let wrist_to_palm_local =
        wrist_rotation.inverse() * (palm_position - wrist_position);
    let wrist_target = palm_target - desired_wrist_rotation * wrist_to_palm_local;
    if crate::env_config::var_os("NORTHSTAR_DEBUG_WEAPON_IK").is_some() {
        static DEBUG_SAMPLES: std::sync::atomic::AtomicUsize =
            std::sync::atomic::AtomicUsize::new(0);
        let sample = DEBUG_SAMPLES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if sample < 2 {
            let shoulder_position = frames[shoulder].transform_point3(Vec3::ZERO);
            let elbow_position = frames[elbow].transform_point3(Vec3::ZERO);
            let upper_len = (elbow_position - shoulder_position).length();
            let lower_len = (wrist_position - elbow_position).length();
            let target_dist = (wrist_target - shoulder_position).length();
            let reach_excess = (target_dist - (upper_len + lower_len)).max(0.0);
            newengine_ulog_api::ulog::info!(
                "WEAPON_IK_REACH arm={} upper_m={:.5} lower_m={:.5} max_reach_m={:.5} target_dist_m={:.5} reach_excess_m={:.5} shoulder={:?} wrist_target={:?} palm_target={:?}",
                label,
                upper_len,
                lower_len,
                upper_len + lower_len,
                target_dist,
                reach_excess,
                shoulder_position,
                wrist_target,
                palm_target,
            );
        }
    }
    solve_two_bone_arm_with_pole(
        skeleton,
        animation_runtime,
        pose,
        frames,
        shoulder,
        elbow,
        wrist,
        wrist_target,
        pole,
    )?;
    rebuild_model_joint_frames(animation_runtime, pose, frames)?;
    set_pose_joint_global_rotation(
        skeleton,
        pose,
        frames,
        wrist,
        desired_wrist_rotation,
    )?;
    rebuild_model_joint_frames(animation_runtime, pose, frames)?;
    Ok(())
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

#[derive(Clone, Copy, Debug)]
struct WeaponIkSolveResult {
    error_m: f32,
    base_root: crate::weapon_grip::WeaponRootTransform,
}

fn arm_reach_fit_correction(
    pose: &[JointLocalPose],
    frames: &[Mat4],
    shoulder: usize,
    elbow: usize,
    wrist: usize,
    palm: usize,
    palm_target: Vec3,
    desired_palm_global: Quat,
) -> Vec3 {
    const REACH_MARGIN_M: f32 = 0.006;
    const MAX_SINGLE_CORRECTION_M: f32 = 0.040;

    let Some(shoulder_frame) = frames.get(shoulder).copied() else {
        return Vec3::ZERO;
    };
    let Some(elbow_frame) = frames.get(elbow).copied() else {
        return Vec3::ZERO;
    };
    let Some(wrist_frame) = frames.get(wrist).copied() else {
        return Vec3::ZERO;
    };
    let Some(palm_frame) = frames.get(palm).copied() else {
        return Vec3::ZERO;
    };
    let Some(palm_local) = pose.get(palm).copied() else {
        return Vec3::ZERO;
    };

    let shoulder_position = shoulder_frame.transform_point3(Vec3::ZERO);
    let elbow_position = elbow_frame.transform_point3(Vec3::ZERO);
    let wrist_position = wrist_frame.transform_point3(Vec3::ZERO);
    let palm_position = palm_frame.transform_point3(Vec3::ZERO);
    let wrist_rotation = wrist_frame
        .to_scale_rotation_translation()
        .1
        .normalize_or_identity();
    let palm_local_rotation = Quat::from_xyzw(
        palm_local.rotation[0],
        palm_local.rotation[1],
        palm_local.rotation[2],
        palm_local.rotation[3],
    )
    .normalize_or_identity();
    let desired_wrist_rotation =
        (desired_palm_global * palm_local_rotation.inverse()).normalize_or_identity();
    let wrist_to_palm_local =
        wrist_rotation.inverse() * (palm_position - wrist_position);
    let wrist_target = palm_target - desired_wrist_rotation * wrist_to_palm_local;

    let upper_len = (elbow_position - shoulder_position).length();
    let lower_len = (wrist_position - elbow_position).length();
    let to_target = wrist_target - shoulder_position;
    let target_distance = to_target.length();
    if !upper_len.is_finite()
        || !lower_len.is_finite()
        || !target_distance.is_finite()
        || upper_len <= 1.0e-5
        || lower_len <= 1.0e-5
        || target_distance <= 1.0e-5
    {
        return Vec3::ZERO;
    }

    let preferred_reach = (upper_len + lower_len - REACH_MARGIN_M).max(1.0e-4);
    let required = target_distance - preferred_reach;
    if required <= 1.0e-5 || required > MAX_SINGLE_CORRECTION_M {
        return Vec3::ZERO;
    }
    to_target * (-required / target_distance)
}

fn fit_weapon_contract_to_supported_arm_reach(
    presentation: &newengine_engine_runtime::gameplay::WeaponPresentationDefinition,
    pose: &[JointLocalPose],
    frames: &[Mat4],
    rig: &WeaponArmIkRig,
    mut contract: crate::weapon_grip::WeaponReadySolveContract,
    support_right_hand: bool,
    support_left_hand: bool,
) -> crate::weapon_grip::WeaponReadySolveContract {
    const MAX_TOTAL_CORRECTION_M: f32 = 0.040;
    let mut accumulated = Vec3::ZERO;

    for _ in 0..2 {
        let mut correction = Vec3::ZERO;
        if support_right_hand {
            let candidate = arm_reach_fit_correction(
                pose,
                frames,
                rig.right_shoulder,
                rig.right_elbow,
                rig.right_wrist,
                rig.right_palm,
                crate::weapon_grip::weapon_ready_right_palm_position(presentation, contract.root),
                crate::weapon_grip::weapon_ready_right_palm_rotation(presentation, contract.root),
            );
            if candidate.length_squared() > correction.length_squared() {
                correction = candidate;
            }
        }
        if support_left_hand {
            let candidate = arm_reach_fit_correction(
                pose,
                frames,
                rig.left_shoulder,
                rig.left_elbow,
                rig.left_wrist,
                rig.left_palm,
                crate::weapon_grip::weapon_ready_left_palm_position(presentation, contract.root),
                crate::weapon_grip::weapon_ready_left_palm_rotation(presentation, contract.root),
            );
            if candidate.length_squared() > correction.length_squared() {
                correction = candidate;
            }
        }

        let correction_len = correction.length();
        if !correction_len.is_finite() || correction_len <= 1.0e-5 {
            break;
        }
        let remaining = (MAX_TOTAL_CORRECTION_M - accumulated.length()).max(0.0);
        if remaining <= 1.0e-5 {
            break;
        }
        if correction_len > remaining {
            correction = correction * (remaining / correction_len);
        }
        contract.root.position += correction;
        contract.stock_contact += correction;
        accumulated += correction;
    }
    contract
}

/// The anatomical ReadyHold contract owns the long-gun root. Character animation contributes torso
/// style, but both hands are solved to weapon contacts after locomotion blending. This prevents a
/// relaxed/reference pose from becoming a false firing-hand authority. Reload may release constraints.
fn apply_equipped_weapon_support_ik(
    presentation: &newengine_engine_runtime::gameplay::WeaponPresentationDefinition,
    rig: Option<&WeaponArmIkRig>,
    skeleton: &ModelSkeletonMetadata,
    animation_runtime: &AnimationSkeletonRuntime,
    pose: &mut [JointLocalPose],
    frames: &mut Vec<Mat4>,
    view_forward_model: Option<Vec3>,
    aim_alpha: f32,
    recoil_alpha: f32,
    recoil_yaw_radians: f32,
    obstruction_alpha: f32,
    secondary_rotation_offset_local: Vec3,
    authored_hand_contacts: bool,
    support_right_hand: bool,
    support_left_hand: bool,
) -> Result<Option<WeaponIkSolveResult>, String> {
    let Some(rig) = rig else {
        return Ok(None);
    };
    rebuild_model_joint_frames(animation_runtime, pose, frames)?;
    let chest = *frames
        .get(rig.chest)
        .ok_or("weapon ReadyHold chest frame is unavailable")?;
    let right_shoulder = *frames
        .get(rig.right_shoulder)
        .ok_or("weapon ReadyHold right shoulder frame is unavailable")?;
    let left_shoulder = *frames
        .get(rig.left_shoulder)
        .ok_or("weapon ReadyHold left shoulder frame is unavailable")?;
    // A complete authored long-gun pose owns the firing-grip translation: preserve the resolved
    // orientation from the torso/aim contract, but place the actual weapon handle at the authored
    // right-hand contact. This is deliberately gated so a missing/partial pose can never let a
    // relaxed hand drag the rifle across the body. The left hand is a bounded rotational support.
    let handle_anchor = (authored_hand_contacts && support_right_hand)
        .then(|| frames[rig.right_palm])
        .and_then(|frame| crate::weapon_grip::weapon_handle_anchor_from_right_palm(presentation, frame));
    let support_anchor = (authored_hand_contacts && support_left_hand)
        .then(|| frames[rig.left_palm])
        .and_then(|frame| crate::weapon_grip::weapon_left_grip_anchor_from_left_palm(presentation, frame));
    let base_contract = crate::weapon_grip::weapon_ready_solve_contract_presented(
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
            support_anchor,
            aim_alpha,
            obstruction_alpha,
        )
    })
    .ok_or("weapon ReadyHold could not resolve torso/contact constraint")?;
    // Reach fitting is a fallback for torso-owned placement only. Once an authored firing hand
    // supplies the handle anchor, translating the root again would break the exact hand/weapon
    // contact we just established.
    let base_contract = if handle_anchor.is_some() {
        base_contract
    } else {
        fit_weapon_contract_to_supported_arm_reach(
            presentation,
            pose,
            frames,
            rig,
            base_contract,
            support_right_hand,
            support_left_hand,
        )
    };
    let base_root = base_contract.root;
    let contract = crate::weapon_grip::weapon_ready_contract_with_secondary_rotation(
        presentation,
        base_contract,
        secondary_rotation_offset_local,
    )
    .ok_or("weapon ReadyHold could not resolve secondary constraint")?;
    // always has a firing-hand master. Keep the fallback branch only for malformed/incomplete rigs.
    let solve_right_hand = support_right_hand;
    let right_target =
        crate::weapon_grip::weapon_ready_right_palm_position(presentation, contract.root);
    let left_target =
        crate::weapon_grip::weapon_ready_left_palm_position(presentation, contract.root);

    if solve_right_hand {
        solve_arm_to_palm_contact(
            skeleton,
            animation_runtime,
            pose,
            frames,
            rig.right_shoulder,
            rig.right_elbow,
            rig.right_wrist,
            rig.right_palm,
            right_target,
            contract.right_elbow_pole,
            crate::weapon_grip::weapon_ready_right_palm_rotation(presentation, contract.root),
            "right",
        )?;
    }
    if support_left_hand {
        solve_arm_to_palm_contact(
            skeleton,
            animation_runtime,
            pose,
            frames,
            rig.left_shoulder,
            rig.left_elbow,
            rig.left_wrist,
            rig.left_palm,
            left_target,
            contract.left_elbow_pole,
            crate::weapon_grip::weapon_ready_left_palm_rotation(presentation, contract.root),
            "left",
        )?;
    }

    rebuild_model_joint_frames(animation_runtime, pose, frames)?;
    if crate::env_config::var_os("NORTHSTAR_DEBUG_WEAPON_CONTACT_FRAMES").is_some() {
        static CONTACT_FRAME_SAMPLES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let sample = CONTACT_FRAME_SAMPLES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if sample < 2 {
            let handle = crate::weapon_grip::weapon_handle_position(presentation, contract.root);
            let left_grip = crate::weapon_grip::weapon_ready_left_grip_position(presentation, contract.root);
            let right_palm_frame = frames[rig.right_palm];
            let left_palm_frame = frames[rig.left_palm];
            let right_palm = right_palm_frame.transform_point3(Vec3::ZERO);
            let left_palm = left_palm_frame.transform_point3(Vec3::ZERO);
            let right_contact = crate::weapon_grip::weapon_handle_anchor_from_right_palm(
                presentation,
                right_palm_frame,
            );
            let left_contact = crate::weapon_grip::weapon_left_grip_anchor_from_left_palm(
                presentation,
                left_palm_frame,
            );
            let right_prop = rig.right_prop_attachment.and_then(|index| frames.get(index).copied()).map(|frame| frame.transform_point3(Vec3::ZERO));
            let left_prop = rig.left_prop_attachment.and_then(|index| frames.get(index).copied()).map(|frame| frame.transform_point3(Vec3::ZERO));
            newengine_ulog_api::ulog::info!(
                "WEAPON_CONTACT_FRAMES right_palm={:?} right_palm_to_handle_m={:.5} right_contact={:?} right_contact_error_m={:?} right_prop={:?} right_prop_reference_to_handle_m={:?} handle={:?} left_palm={:?} left_palm_to_grip_m={:.5} left_contact={:?} left_contact_error_m={:?} left_prop={:?} left_prop_reference_to_handle_m={:?} l_grip={:?}",
                right_palm, (right_palm-handle).length(), right_contact, right_contact.map(|value| (value-handle).length()), right_prop, right_prop.map(|value| (value-handle).length()), handle, left_palm, (left_palm-left_grip).length(), left_contact, left_contact.map(|value| (value-left_grip).length()), left_prop, left_prop.map(|value| (value-handle).length()), left_grip,
            );
        }
    }
    let right_error = if solve_right_hand {
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
    Ok(Some(WeaponIkSolveResult { error_m: error, base_root }))
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
