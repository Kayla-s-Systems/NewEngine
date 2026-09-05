#[derive(Clone, Copy, Debug)]
struct WeaponArmIkRig {
    chest: usize,
    right_clavicle: Option<usize>,
    right_shoulder: usize,
    right_elbow: usize,
    right_wrist: usize,
    right_palm: usize,
    right_prop_helper: Option<usize>,
    right_prop_attachment: Option<usize>,
    left_clavicle: Option<usize>,
    left_shoulder: usize,
    left_elbow: usize,
    left_wrist: usize,
    left_palm: usize,
    left_prop_helper: Option<usize>,
    left_prop_attachment: Option<usize>,
}

fn build_weapon_arm_ik_rig(
    skeleton: &ModelSkeletonMetadata,
    authored: &newengine_engine_runtime::gameplay::PlayerWeaponArmIkRigDefinition,
) -> Result<WeaponArmIkRig, String> {
    let resolve_required = |label: &str, name: &str| -> Result<usize, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(format!("authored weapon IK joint '{label}' is empty"));
        }
        skeleton
            .joints
            .iter()
            .position(|joint| joint.name == name)
            .ok_or_else(|| {
                format!("authored weapon IK joint '{label}' is absent from skeleton name='{name}'")
            })
    };
    let resolve_optional = |label: &str, name: Option<&String>| -> Result<Option<usize>, String> {
        let Some(name) = name
            .map(String::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            return Ok(None);
        };
        skeleton
            .joints
            .iter()
            .position(|joint| joint.name == name)
            .map(Some)
            .ok_or_else(|| {
                format!("authored weapon IK joint '{label}' is absent from skeleton name='{name}'")
            })
    };

    let right_shoulder = resolve_required("right_shoulder", &authored.right_shoulder)?;
    let left_shoulder = resolve_required("left_shoulder", &authored.left_shoulder)?;
    let right_prop_attachment = resolve_optional(
        "right_prop_attachment",
        authored.right_prop_attachment.as_ref(),
    )?;
    let left_prop_attachment = resolve_optional(
        "left_prop_attachment",
        authored.left_prop_attachment.as_ref(),
    )?;
    let parent_index = |joint: usize| {
        skeleton
            .joints
            .get(joint)
            .and_then(|joint| joint.parent_index)
            .map(|parent| parent as usize)
            .filter(|parent| *parent < skeleton.joints.len())
    };

    Ok(WeaponArmIkRig {
        chest: resolve_required("chest", &authored.chest)?,
        right_clavicle: parent_index(right_shoulder),
        right_shoulder,
        right_elbow: resolve_required("right_elbow", &authored.right_elbow)?,
        right_wrist: resolve_required("right_wrist", &authored.right_wrist)?,
        right_palm: resolve_required("right_palm", &authored.right_palm)?,
        right_prop_helper: right_prop_attachment.and_then(parent_index),
        right_prop_attachment,
        left_clavicle: parent_index(left_shoulder),
        left_shoulder,
        left_elbow: resolve_required("left_elbow", &authored.left_elbow)?,
        left_wrist: resolve_required("left_wrist", &authored.left_wrist)?,
        left_palm: resolve_required("left_palm", &authored.left_palm)?,
        left_prop_helper: left_prop_attachment.and_then(parent_index),
        left_prop_attachment,
    })
}

fn rebuild_model_joint_frames(
    animation_runtime: &AnimationSkeletonRuntime,
    pose: &[JointLocalPose],
    frames: &mut Vec<Mat4>,
) -> Result<(), String> {
    animation_runtime.build_model_joint_frames_from_local_pose(pose, frames)
}

#[inline]
fn refresh_model_joint_frames_subtree(
    animation_runtime: &AnimationSkeletonRuntime,
    pose: &[JointLocalPose],
    frames: &mut Vec<Mat4>,
    root_joint: usize,
) -> Result<(), String> {
    animation_runtime.refresh_model_joint_frames_subtree_from_local_pose(
        pose,
        frames.as_mut_slice(),
        root_joint,
    )
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

#[allow(clippy::too_many_arguments)]
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
) -> Result<bool, String> {
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
        return Ok(false);
    }

    let direction = raw_to_target / raw_distance;
    let min_reach = (upper_len - lower_len).abs() + 1.0e-4;
    // Never manufacture contact by locking a real arm at mathematical full extension. Skinning
    // around a nearly collinear shoulder/elbow/wrist chain is exactly what reads as a rubber arm.
    // Small presentation mismatch is preferable: the authored pose remains authoritative until the
    // weapon/contact contract is physically reachable with an anatomical elbow bend.
    const SAFE_EXTENSION_MARGIN_M: f32 = 0.012;
    let safe_max_reach = (upper_len + lower_len - SAFE_EXTENSION_MARGIN_M).max(min_reach);
    if raw_distance > safe_max_reach {
        return Ok(false);
    }
    let distance = raw_distance.clamp(min_reach, safe_max_reach);
    let reachable_target = shoulder_position + direction * distance;

    let pole_vector = pole - shoulder_position;
    let mut bend_direction = pole_vector - direction * pole_vector.dot(direction);
    if bend_direction.length_squared() <= 1.0e-8 {
        let current_bend = elbow_position - shoulder_position;
        bend_direction = current_bend - direction * current_bend.dot(direction);
    }
    bend_direction = bend_direction.normalize_or_zero();
    if bend_direction.length_squared() <= 1.0e-8 {
        return Ok(false);
    }

    let along = ((upper_len * upper_len - lower_len * lower_len + distance * distance)
        / (2.0 * distance))
        .clamp(0.0, upper_len);
    let height = (upper_len * upper_len - along * along).max(0.0).sqrt();
    let desired_elbow = shoulder_position + direction * along + bend_direction * height;

    // First orient the upper arm into the preferred elbow plane, then close the forearm onto the
    // palm target. No free CCD iterations remain, so the elbow cannot flip to another plane.
    rotate_pose_joint_toward(skeleton, pose, frames, shoulder, elbow, desired_elbow, 1.0)?;
    refresh_model_joint_frames_subtree(animation_runtime, pose, frames, shoulder)?;
    rotate_pose_joint_toward(skeleton, pose, frames, elbow, palm, reachable_target, 1.0)?;
    refresh_model_joint_frames_subtree(animation_runtime, pose, frames, elbow)?;
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
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
) -> Result<bool, String> {
    let wrist_frame = *frames.get(wrist).ok_or("rifle wrist frame missing")?;
    let palm_frame = *frames.get(palm).ok_or("rifle palm frame missing")?;
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
    let palm_local_rotation =
        Quat::from_xyzw(palm_local[0], palm_local[1], palm_local[2], palm_local[3])
            .normalize_or_identity();
    let desired_wrist_rotation =
        (desired_palm_global * palm_local_rotation.inverse()).normalize_or_identity();
    let wrist_to_palm_local = wrist_rotation.inverse() * (palm_position - wrist_position);
    let wrist_target = palm_target - desired_wrist_rotation * wrist_to_palm_local;
    if newengine_runtime_env::var_os("NORTHSTAR_DEBUG_WEAPON_IK").is_some() {
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
    let solved = solve_two_bone_arm_with_pole(
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
    if !solved {
        return Ok(false);
    }
    set_pose_joint_global_rotation(skeleton, pose, frames, wrist, desired_wrist_rotation)?;
    refresh_model_joint_frames_subtree(animation_runtime, pose, frames, wrist)?;
    Ok(true)
}

fn set_pose_joint_global_transform(
    skeleton: &ModelSkeletonMetadata,
    pose: &mut [JointLocalPose],
    frames: &[Mat4],
    joint_index: usize,
    desired_global: Mat4,
) -> Result<(), String> {
    let parent_global = skeleton.joints[joint_index]
        .parent_index
        .and_then(|parent| frames.get(parent as usize).copied())
        .unwrap_or(Mat4::IDENTITY);
    let desired_local = parent_global.inverse() * desired_global;
    let (scale, rotation, translation) = desired_local.to_scale_rotation_translation();
    if !scale.is_finite()
        || scale.x <= 0.0
        || scale.y <= 0.0
        || scale.z <= 0.0
        || !rotation.is_finite()
        || !translation.is_finite()
    {
        return Err(format!(
            "rifle global transform projection produced invalid local frame index={joint_index}"
        ));
    }
    let local = pose
        .get_mut(joint_index)
        .ok_or_else(|| format!("rifle projected local pose missing index={joint_index}"))?;
    local.translation = [translation.x, translation.y, translation.z];
    let rotation = rotation.normalize_or_identity();
    local.rotation = [rotation.x, rotation.y, rotation.z, rotation.w];
    // A camera-space presentation delta is rigid. Preserve the authored local scale verbatim rather
    // than feeding decomposition noise back into the deformation rig.
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
    right_error_m: f32,
    left_error_m: f32,
    socket_position_error_m: f32,
    socket_angular_error_deg: f32,
    /// Exact final root consumed by terminal hand contacts and rendered weapon attachment.
    resolved_root: crate::weapon_grip::WeaponRootTransform,
}

#[allow(clippy::too_many_arguments)]
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
    let wrist_to_palm_local = wrist_rotation.inverse() * (palm_position - wrist_position);
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
            correction *= remaining / correction_len;
        }
        contract.root.position += correction;
        contract.stock_contact += correction;
        accumulated += correction;
    }
    contract
}
