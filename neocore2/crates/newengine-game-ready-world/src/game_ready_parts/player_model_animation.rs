use super::*;

use newengine_animation_runtime::{
    build_model_joint_frames_from_local_pose, build_skin_palette_from_local_pose, decode_ycd_body,
    AnimationClip, JointLocalPose,
};
use newengine_assets::{AssetDecodeRequest, AssetServiceClient, ASSET_LIST_FILE_BODY_OUTPUT};
use newengine_math::{Mat4, Quat, Vec3};
use newengine_model_skeleton_api::ModelSkeletonMetadata;

#[derive(Clone, Debug)]
struct PlayerAnimationRuntimeClip {
    clip_ref: String,
    clip: AnimationClip,
}

#[derive(Clone, Debug)]
pub(super) struct PlayerAnimationRuntimeBinding {
    clips: [Option<PlayerAnimationRuntimeClip>; 8],
    active_state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    active_slot: usize,
    skeleton: ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    time_seconds: f32,
    /// Pose currently visible on the character. This is preserved when a new
    /// locomotion state interrupts an in-flight cross-fade.
    current_locals: Vec<JointLocalPose>,
    sampled_target_locals: Vec<JointLocalPose>,
    transition_from_locals: Vec<JointLocalPose>,
    palette_scratch: Vec<Mat4>,
    /// Absolute bind joint frames in baked model space. Current animated frames are derived as
    /// `skin_palette * bind_frame`, after all pose/follower corrections but before braid solve.
    bind_joint_frames: Vec<Mat4>,
    joint_frames_scratch: Vec<Mat4>,
    /// Mirrored North Star deform/helper branches must follow their primary joints.
    helper_mirror_pairs: Vec<(usize, usize)>,
    /// Imported Rigify control/face branches need the authored constraint order restored:
    /// deform body -> animated neck/head controls -> face/eyes deform branches.
    eye_contract: Option<EyeRuntimeContract>,
    head_follow: Option<DetachedHeadFollowRig>,
    equipment_ready_pose: Option<PlayerAnimationRuntimeClip>,
    equipment_aim_pose: Option<PlayerAnimationRuntimeClip>,
    equipment_reload_pose: Option<PlayerAnimationRuntimeClip>,
    equipment_ready_sample_phase: f32,
    equipment_time_seconds: f32,
    equipment_ready_rotation_weights: Vec<(String, f32)>,
    equipment_aim_rotation_weights: Vec<(String, f32)>,
    equipment_reload_rotation_weights: Vec<(String, f32)>,
    equipment_overlay_locals: Vec<JointLocalPose>,
    equipment_ik: Option<WeaponArmIkRig>,
}

#[inline]
const fn locomotion_slot(
    state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
) -> usize {
    use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation as L;
    match state {
        L::Idle => 0,
        L::Walk => 1,
        L::Run => 2,
        L::Sprint => 3,
        L::CrouchIdle => 4,
        L::CrouchWalk => 5,
        L::Jump => 6,
        L::Fall => 7,
    }
}

impl PlayerAnimationRuntimeBinding {
    pub(super) fn initial_palette(&self) -> Vec<Mat4> {
        self.palette_scratch.clone()
    }

    pub(super) fn skeleton_joint_count(&self) -> usize {
        self.skeleton.joints.len()
    }

    pub(super) fn supplemental_palette_joint_count(&self) -> usize {
        0
    }

    pub(super) fn expected_palette_joints(&self) -> usize {
        self.skeleton_joint_count()
    }

    pub(super) fn clip_refs_csv(&self) -> String {
        self.clips
            .iter()
            .filter_map(|clip| clip.as_ref().map(|clip| clip.clip_ref.as_str()))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn resolve_slot(
        &self,
        state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    ) -> usize {
        use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation as L;
        let candidates: &[usize] = match state {
            L::Idle => &[0],
            L::Walk => &[1, 0],
            L::Run => &[2, 1, 0],
            L::Sprint => &[3, 2, 1, 0],
            L::CrouchIdle => &[4, 0],
            L::CrouchWalk => &[5, 4, 1, 0],
            L::Jump => &[6, 2, 0],
            L::Fall => &[7, 6, 2, 0],
        };
        candidates
            .iter()
            .copied()
            .find(|slot| self.clips[*slot].is_some())
            .unwrap_or(0)
    }
}

#[inline]
fn blend_joint_rotation_only(dst: &mut JointLocalPose, src: &JointLocalPose, weight: f32) {
    let weight = if weight.is_finite() {
        weight.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let from = Quat::from_xyzw(
        dst.rotation[0],
        dst.rotation[1],
        dst.rotation[2],
        dst.rotation[3],
    )
    .normalize_or_identity();
    let mut to = Quat::from_xyzw(
        src.rotation[0],
        src.rotation[1],
        src.rotation[2],
        src.rotation[3],
    )
    .normalize_or_identity();
    if from.dot(to) < 0.0 {
        to = Quat::from_xyzw(-to.x, -to.y, -to.z, -to.w);
    }
    let rotation = from.slerp(to, weight).normalize_or_identity();
    dst.rotation = [rotation.x, rotation.y, rotation.z, rotation.w];
}

fn apply_equipment_rotation_overlay(
    clip: Option<&PlayerAnimationRuntimeClip>,
    skeleton: &ModelSkeletonMetadata,
    scratch: &mut Vec<JointLocalPose>,
    target: &mut [JointLocalPose],
    normalized_phase: f32,
    weights: &[(String, f32)],
    weight_scale: f32,
) -> Result<(), String> {
    let Some(clip) = clip else {
        return Ok(());
    };
    let phase = if normalized_phase.is_finite() {
        normalized_phase.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let sample_time =
        (clip.clip.duration_seconds * phase).clamp(0.0, clip.clip.duration_seconds.max(0.0));
    clip.clip
        .sample_local_pose_for_skeleton(sample_time, skeleton, scratch)?;
    for (name, weight) in weights {
        let Some(index) = skeleton
            .joints
            .iter()
            .position(|joint| joint.name == name.as_str())
        else {
            continue;
        };
        if let (Some(dst), Some(src)) = (target.get_mut(index), scratch.get(index)) {
            blend_joint_rotation_only(dst, src, (*weight * weight_scale).clamp(0.0, 1.0));
        }
    }
    Ok(())
}

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

#[derive(Clone, Copy, Debug)]
struct EyeRuntimeContract {
    left: usize,
    right: usize,
    parent: usize,
}

fn build_eye_runtime_contract(skeleton: &ModelSkeletonMetadata) -> Option<EyeRuntimeContract> {
    let left = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == "l_eyeball")?;
    let right = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == "r_eyeball")?;
    let parent = skeleton.joints.get(left)?.parent_index? as usize;
    if skeleton
        .joints
        .get(right)?
        .parent_index
        .map(|value| value as usize)
        != Some(parent)
        || skeleton.joints.get(parent)?.name != "headb"
    {
        return None;
    }
    Some(EyeRuntimeContract {
        left,
        right,
        parent,
    })
}

fn stabilize_eye_locals(
    contract: Option<&EyeRuntimeContract>,
    skeleton: &ModelSkeletonMetadata,
    pose: &mut [JointLocalPose],
) -> Result<(), String> {
    let Some(contract) = contract else {
        return Ok(());
    };
    for index in [contract.left, contract.right] {
        let joint = skeleton
            .joints
            .get(index)
            .ok_or_else(|| format!("eye joint outside skeleton index={index}"))?;
        let dst = pose
            .get_mut(index)
            .ok_or_else(|| format!("eye joint outside sampled pose index={index}"))?;
        *dst = JointLocalPose {
            translation: joint.position_ls,
            rotation: joint.rotation_ls,
            scale: Some(joint.scale_ls),
        };
    }
    Ok(())
}

#[inline]
fn matrix_max_abs_delta(a: Mat4, b: Mat4) -> f32 {
    a.to_cols_array()
        .into_iter()
        .zip(b.to_cols_array())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max)
}

fn validate_eye_palette(
    contract: Option<&EyeRuntimeContract>,
    palette: &[Mat4],
) -> Result<(), String> {
    let Some(contract) = contract else {
        return Ok(());
    };
    let parent = *palette
        .get(contract.parent)
        .ok_or_else(|| "eye parent outside skin palette".to_owned())?;
    for (side, index) in [("left", contract.left), ("right", contract.right)] {
        let eye = *palette
            .get(index)
            .ok_or_else(|| format!("{side} eye outside skin palette index={index}"))?;
        let drift = matrix_max_abs_delta(eye, parent);
        // With authored bind-local eyes, A_eye=A_parent*Lbind and B_eye=B_parent*Lbind,
        // therefore A_eye*inverse(B_eye) must reduce to the exact parent deformation.
        if !drift.is_finite() || drift > 5.0e-4 {
            return Err(format!(
                "{side} eye palette drift violates animated_global*inverse_bind contract index={index} parent={} max_abs_delta={drift:.8}",
                contract.parent
            ));
        }
    }
    Ok(())
}

fn debug_dump_eye_matrices(
    contract: Option<&EyeRuntimeContract>,
    bind_joint_frames: &[Mat4],
    current_locals: &[JointLocalPose],
    palette: &[Mat4],
    context: &str,
) {
    let Some(contract) = contract else {
        return;
    };
    if crate::env_config::var_os("NORTHSTAR_DEBUG_ABBY_EYES").is_none() {
        return;
    }
    let Some(parent_bind_global) = bind_joint_frames.get(contract.parent).copied() else {
        return;
    };
    let Some(parent_palette) = palette.get(contract.parent).copied() else {
        return;
    };
    let parent_global = parent_palette * parent_bind_global;
    for (side, index) in [("left", contract.left), ("right", contract.right)] {
        let (Some(bind_global), Some(local), Some(palette_matrix)) = (
            bind_joint_frames.get(index).copied(),
            current_locals.get(index),
            palette.get(index).copied(),
        ) else {
            continue;
        };
        let scale = local.scale.unwrap_or([1.0, 1.0, 1.0]);
        let animated_local = Mat4::from_scale_rotation_translation(
            Vec3::new(scale[0], scale[1], scale[2]),
            Quat::from_xyzw(
                local.rotation[0],
                local.rotation[1],
                local.rotation[2],
                local.rotation[3],
            )
            .normalize_or_identity(),
            Vec3::new(
                local.translation[0],
                local.translation[1],
                local.translation[2],
            ),
        );
        let animated_global = palette_matrix * bind_global;
        newengine_ulog_api::ulog::info!(
            "ABBY_EYE_MATRIX context='{}' side={} joint={} parent={} bind_global={:?} parent_global={:?} animated_local={:?} animated_global={:?} palette_matrix={:?} parent_palette={:?} palette_parent_drift={:.8}",
            context,
            side,
            index,
            contract.parent,
            bind_global,
            parent_global,
            animated_local,
            animated_global,
            palette_matrix,
            parent_palette,
            matrix_max_abs_delta(palette_matrix, parent_palette),
        );
    }
}

#[derive(Clone, Debug)]
struct DetachedHeadFollowRig {
    /// Canonical imported equivalent of North Star `headb`.
    ///
    /// Abby's scalp/hair skin is predominantly weighted to `DEF-spine.006`, and
    /// the original `abby-skel` parents `braid_offset` directly to `headb`.
    /// Detached Blender control/face branches must therefore inherit this same
    /// deformation delta instead of becoming a second animated head space.
    headb_driver: usize,
    control_followers: Vec<usize>,
    face_followers: Vec<usize>,
}

fn collect_joint_descendants(skeleton: &ModelSkeletonMetadata, roots: &[usize]) -> Vec<usize> {
    let mut followers = Vec::new();
    for index in 0..skeleton.joints.len() {
        let mut cursor = Some(index);
        let mut remaining = skeleton.joints.len();
        while let Some(current) = cursor {
            if roots.contains(&current) {
                followers.push(index);
                break;
            }
            if current >= skeleton.joints.len() || remaining == 0 {
                break;
            }
            remaining -= 1;
            cursor = skeleton.joints[current]
                .parent_index
                .map(|value| value as usize);
        }
    }
    followers.sort_unstable();
    followers.dedup();
    followers
}

fn build_detached_head_follow(skeleton: &ModelSkeletonMetadata) -> Option<DetachedHeadFollowRig> {
    // Binary authority: original `abby-skel.pak` hierarchy is
    // `... -> neck -> heada -> headb -> braid_offset`. Bind-space comparison maps
    // those joints to `DEF-spine.004/.005/.006` in the imported 709-joint rig.
    let headb_driver = skeleton
        .joints
        .iter()
        .position(|joint| joint.name == "DEF-spine.006")?;

    // The Blender control rig is detached from the deform chain. Keep it useful
    // for authored controls, but project the *same* headb rigid deformation onto
    // it. It is never the skinning authority for Abby's head/hair.
    let control_roots = skeleton
        .joints
        .iter()
        .enumerate()
        .filter_map(|(index, joint)| (joint.name == "MCH-ROT-neck").then_some(index))
        .collect::<Vec<_>>();
    let face_roots = skeleton
        .joints
        .iter()
        .enumerate()
        .filter_map(|(index, joint)| {
            matches!(joint.name.as_str(), "ORG-face" | "MCH-eyes_parent").then_some(index)
        })
        .collect::<Vec<_>>();
    if face_roots.is_empty() {
        return None;
    }

    let control_followers = collect_joint_descendants(skeleton, &control_roots);
    let mut face_followers = collect_joint_descendants(skeleton, &face_roots);
    face_followers.retain(|joint| *joint != headb_driver && !control_followers.contains(joint));

    Some(DetachedHeadFollowRig {
        headb_driver,
        control_followers,
        face_followers,
    })
}

fn apply_detached_head_follow_palette(
    rig: Option<&DetachedHeadFollowRig>,
    palette: &mut [Mat4],
) -> Result<(), String> {
    let Some(rig) = rig else {
        return Ok(());
    };
    let headb_deformation = *palette.get(rig.headb_driver).ok_or_else(|| {
        format!(
            "head-follow canonical headb driver outside palette joint={}",
            rig.headb_driver
        )
    })?;

    // Skin-palette entries are model-space deformation transforms, not local
    // joint transforms. Never rebuild a fake MCH hierarchy by multiplying them
    // parent-by-child. Apply one shared rigid headb delta to every detached
    // control/face branch. Scalp, facial skin, eyes and braid then live in the
    // exact same animated head space as `DEF-spine.006`.
    for &joint in rig
        .control_followers
        .iter()
        .chain(rig.face_followers.iter())
    {
        let detached_deformation = *palette
            .get(joint)
            .ok_or_else(|| format!("detached head follower outside palette joint={joint}"))?;
        palette[joint] = headb_deformation * detached_deformation;
    }
    Ok(())
}

fn blend_local_poses(
    from: &[JointLocalPose],
    to: &[JointLocalPose],
    alpha: f32,
    out: &mut Vec<JointLocalPose>,
) -> Result<(), String> {
    if from.len() != to.len() {
        return Err(format!(
            "animation transition pose count mismatch from={} to={}",
            from.len(),
            to.len()
        ));
    }
    let alpha = if alpha.is_finite() {
        alpha.clamp(0.0, 1.0)
    } else {
        1.0
    };
    out.clear();
    out.reserve(to.len());
    for (a, b) in from.iter().zip(to.iter()) {
        let translation = Vec3::new(a.translation[0], a.translation[1], a.translation[2]).lerp(
            Vec3::new(b.translation[0], b.translation[1], b.translation[2]),
            alpha,
        );
        let qa = Quat::from_xyzw(a.rotation[0], a.rotation[1], a.rotation[2], a.rotation[3])
            .normalize_or_identity();
        let mut qb = Quat::from_xyzw(b.rotation[0], b.rotation[1], b.rotation[2], b.rotation[3])
            .normalize_or_identity();
        if qa.dot(qb) < 0.0 {
            qb = Quat::from_xyzw(-qb.x, -qb.y, -qb.z, -qb.w);
        }
        let q = qa.slerp(qb, alpha).normalize_or_identity();
        let scale = match (a.scale, b.scale) {
            (Some(a), Some(b)) => {
                let scale = Vec3::new(a[0], a[1], a[2]).lerp(Vec3::new(b[0], b[1], b[2]), alpha);
                Some([scale.x, scale.y, scale.z])
            }
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        out.push(JointLocalPose {
            translation: [translation.x, translation.y, translation.z],
            rotation: [q.x, q.y, q.z, q.w],
            scale,
        });
    }
    Ok(())
}

fn split_animation_ref(reference: &str) -> Result<(String, Option<String>), String> {
    let normalized = reference.trim().replace('\\', "/");
    if normalized.is_empty() {
        return Err("empty animation reference".to_owned());
    }
    let (path, selector) = normalized
        .rsplit_once('@')
        .map(|(path, selector)| {
            let selector = selector.trim();
            (
                path.to_owned(),
                (!selector.is_empty()).then(|| selector.to_owned()),
            )
        })
        .unwrap_or_else(|| (normalized.clone(), None));
    if !path.to_ascii_lowercase().ends_with(".ycd") {
        return Err(format!(
            "player animation must reference .ycd asset: '{reference}'"
        ));
    }
    Ok((path, selector))
}

fn load_animation_clip(reference: &str) -> Result<AnimationClip, String> {
    let (path, selector) = split_animation_ref(reference)?;
    let assets = AssetServiceClient::new(newengine_plugin_host::default_host_api());
    let payload = assets
        .decode_v1(&AssetDecodeRequest {
            logical_path: path.clone(),
            output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
            selector: serde_json::Value::Null,
        })
        .map_err(|error| {
            format!(
                "player animation asset decode failed ref='{reference}' path='{path}' err='{error}'"
            )
        })?;
    decode_ycd_body(&payload, selector.as_deref()).map_err(|error| {
        format!("player animation YCD decode failed ref='{reference}' err='{error}'")
    })
}

fn validate_animation_clip(
    clip_ref: &str,
    clip: &AnimationClip,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    skeleton: &ModelSkeletonMetadata,
) -> Result<(), String> {
    if !clip.skeleton_ref.trim().is_empty()
        && !clip
            .skeleton_ref
            .eq_ignore_ascii_case(assignment.skeleton_source.as_deref().unwrap_or_default())
    {
        return Err(format!(
            "player animation skeleton ref mismatch clip='{}' assignment='{}'",
            clip.skeleton_ref,
            assignment.skeleton_source.as_deref().unwrap_or("<none>")
        ));
    }
    for (clip_index, &tag) in clip.joint_tags.iter().enumerate() {
        if clip.joint_tags[..clip_index].contains(&tag) {
            return Err(format!(
                "player animation contains duplicate skeleton tag ref='{}' tag={}",
                clip_ref, tag
            ));
        }
        let dense = tag as usize;
        let present = dense < skeleton.joints.len() && skeleton.joints[dense].tag == tag
            || skeleton.joints.iter().any(|joint| joint.tag == tag);
        if !present {
            return Err(format!(
                "player animation skeleton tag is absent ref='{}' clip_index={} tag={} skeleton_joints={}",
                clip_ref,
                clip_index,
                tag,
                skeleton.joints.len()
            ));
        }
    }
    Ok(())
}

fn load_runtime_animation_clip(
    reference: &str,
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    skeleton: &ModelSkeletonMetadata,
) -> Result<PlayerAnimationRuntimeClip, String> {
    let clip = load_animation_clip(reference)?;
    validate_animation_clip(reference, &clip, assignment, skeleton)?;
    Ok(PlayerAnimationRuntimeClip {
        clip_ref: reference.to_owned(),
        clip,
    })
}

pub(super) fn prepare_player_animation_binding(
    assignment: &newengine_engine_runtime::gameplay::PlayerModelAssignment,
    parts: &[PlayerRuntimeModelPart],
    skeleton: Option<&ModelSkeletonMetadata>,
) -> Result<Option<PlayerAnimationRuntimeBinding>, String> {
    use newengine_engine_runtime::gameplay::PlayerLocomotionAnimation as L;

    let skinned_parts = parts
        .iter()
        .filter_map(|part| part.skin.as_ref())
        .collect::<Vec<_>>();
    if skinned_parts.is_empty() {
        return Ok(None);
    }
    let skeleton = skeleton
        .ok_or_else(|| "skinned player model requires authored skeleton metadata".to_owned())?;
    let source_to_model = skinned_parts[0].source_to_model;
    for (part_index, skin) in skinned_parts.iter().enumerate() {
        if skin.source_to_model != source_to_model {
            return Err(format!(
                "skinned player model source-space transform mismatch part={part_index}"
            ));
        }
    }

    let Some(idle_ref) = assignment.idle_animation.as_deref() else {
        return Ok(None);
    };
    let mut clips: [Option<PlayerAnimationRuntimeClip>; 8] =
        [None, None, None, None, None, None, None, None];
    clips[locomotion_slot(L::Idle)] =
        Some(load_runtime_animation_clip(idle_ref, assignment, skeleton)?);

    for (state, reference) in [
        (L::Walk, assignment.walk_animation.as_deref()),
        (L::Run, assignment.run_animation.as_deref()),
        (L::Sprint, assignment.sprint_animation.as_deref()),
        (L::CrouchIdle, assignment.crouch_idle_animation.as_deref()),
        (L::CrouchWalk, assignment.crouch_walk_animation.as_deref()),
        (L::Jump, assignment.jump_animation.as_deref()),
        (L::Fall, assignment.fall_animation.as_deref()),
    ] {
        if let Some(reference) = reference {
            clips[locomotion_slot(state)] = Some(load_runtime_animation_clip(
                reference, assignment, skeleton,
            )?);
        }
    }

    let idle = clips[locomotion_slot(L::Idle)]
        .as_ref()
        .expect("idle clip was inserted above");
    let helper_mirror_pairs = build_helper_mirror_pairs(skeleton);
    // Compatibility reconstruction is explicit project-authored presentation metadata. Runtime
    // behavior is never inferred from a character name, model path, or source franchise.
    let head_follow = assignment
        .presentation
        .detached_head_follow
        .then(|| build_detached_head_follow(skeleton))
        .flatten();
    let eye_contract = assignment
        .presentation
        .eye_parent_follow
        .then(|| build_eye_runtime_contract(skeleton))
        .flatten();
    let bind_locals = skeleton
        .joints
        .iter()
        .map(|joint| JointLocalPose {
            translation: joint.position_ls,
            rotation: joint.rotation_ls,
            scale: Some(joint.scale_ls),
        })
        .collect::<Vec<_>>();
    let mut bind_joint_frames = Vec::with_capacity(skeleton.joints.len());
    build_model_joint_frames_from_local_pose(
        skeleton,
        source_to_model,
        &bind_locals,
        &mut bind_joint_frames,
    )?;
    let mut current_locals = Vec::with_capacity(skeleton.joints.len());
    idle.clip
        .sample_local_pose_for_skeleton(0.0, skeleton, &mut current_locals)?;
    synchronize_helper_pose(&helper_mirror_pairs, &mut current_locals);
    stabilize_eye_locals(eye_contract.as_ref(), skeleton, &mut current_locals)?;
    let mut palette_scratch = Vec::with_capacity(skeleton.joints.len());
    build_skin_palette_from_local_pose(
        skeleton,
        source_to_model,
        &current_locals,
        &mut palette_scratch,
    )?;
    apply_detached_head_follow_palette(head_follow.as_ref(), &mut palette_scratch)?;
    validate_eye_palette(eye_contract.as_ref(), &palette_scratch)?;
    debug_dump_eye_matrices(
        eye_contract.as_ref(),
        &bind_joint_frames,
        &current_locals,
        &palette_scratch,
        "initial",
    );
    let equipment_ready_pose = assignment
        .presentation
        .equipment_ready_animation
        .as_deref()
        .map(|reference| load_runtime_animation_clip(reference, assignment, skeleton))
        .transpose()?;
    let equipment_aim_pose = assignment
        .presentation
        .equipment_aim_animation
        .as_deref()
        .map(|reference| load_runtime_animation_clip(reference, assignment, skeleton))
        .transpose()?;
    let equipment_reload_pose = assignment
        .presentation
        .equipment_reload_animation
        .as_deref()
        .map(|reference| load_runtime_animation_clip(reference, assignment, skeleton))
        .transpose()?;
    let equipment_ready_sample_phase = assignment
        .presentation
        .equipment_ready_sample_phase
        .clamp(0.0, 1.0);
    let equipment_ready_rotation_weights = assignment
        .presentation
        .equipment_ready_rotation_weights
        .iter()
        .map(|item| (item.joint.clone(), item.weight.clamp(0.0, 1.0)))
        .collect::<Vec<_>>();
    let equipment_aim_rotation_weights = assignment
        .presentation
        .equipment_aim_rotation_weights
        .iter()
        .map(|item| (item.joint.clone(), item.weight.clamp(0.0, 1.0)))
        .collect::<Vec<_>>();
    let equipment_reload_rotation_weights = assignment
        .presentation
        .equipment_reload_rotation_weights
        .iter()
        .map(|item| (item.joint.clone(), item.weight.clamp(0.0, 1.0)))
        .collect::<Vec<_>>();
    let equipment_ik = assignment
        .presentation
        .equipment_arm_ik
        .then(|| build_weapon_arm_ik_rig(skeleton))
        .flatten();
    let joint_frames_scratch = Vec::with_capacity(skeleton.joints.len());
    let sampled_target_locals = current_locals.clone();
    let transition_from_locals = current_locals.clone();
    if !helper_mirror_pairs.is_empty() {
        newengine_ulog_api::ulog::info!(
            "game-ready: mirrored North Star helper rig channels={} policy='primary local pose -> *_helper deform branch before skin palette'",
            helper_mirror_pairs.len()
        );
    }
    if let Some(rig) = head_follow.as_ref() {
        newengine_ulog_api::ulog::info!(
            "game-ready: restored authored detached head-space headb_driver={} control_followers={} face_followers={} policy='primary deform hierarchy -> detached controls + face/eyes'",
            rig.headb_driver,
            rig.control_followers.len(),
            rig.face_followers.len(),
        );
    }
    if let Some(eyes) = eye_contract.as_ref() {
        newengine_ulog_api::ulog::info!(
            "game-ready: authored eye-parent contract left={} right={} parent={} policy='locomotion keeps authored eye-local bind; eye palette follows authored parent deformation'",
            eyes.left,
            eyes.right,
            eyes.parent,
        );
    }

    Ok(Some(PlayerAnimationRuntimeBinding {
        clips,
        active_state: L::Idle,
        active_slot: locomotion_slot(L::Idle),
        skeleton: skeleton.clone(),
        source_to_model,
        time_seconds: 0.0,
        current_locals,
        sampled_target_locals,
        transition_from_locals,
        palette_scratch,
        bind_joint_frames,
        joint_frames_scratch,
        helper_mirror_pairs,
        eye_contract,
        head_follow,
        equipment_ready_pose,
        equipment_aim_pose,
        equipment_reload_pose,
        equipment_ready_sample_phase,
        equipment_time_seconds: 0.0,
        equipment_ready_rotation_weights,
        equipment_aim_rotation_weights,
        equipment_reload_rotation_weights,
        equipment_overlay_locals: bind_locals,
        equipment_ik,
    }))
}

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

/// Physical right-hand master frame for held weapons. Constraint/prop targets are forbidden.
pub(crate) fn player_right_hand_weapon_frame(
    world: &newengine_ecs::World,
    player: EntityId,
) -> Option<Mat4> {
    player_prop_frame(
        world,
        player,
        &["r_palm", "r_wrist", "DEF-hand.R", "hand.R"],
    )
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
    stable_hand_grip_frame(
        world,
        player,
        &["r_hand_prop_attachment", "r_hand_prop"],
        &["r_palm", "r_wrist", "DEF-hand.R", "hand.R"],
    )
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

pub(crate) fn tick_player_skin_animation(world: &mut newengine_ecs::World, dt: f32) {
    let dt = if dt.is_finite() && dt > 0.0 {
        dt.min(0.1)
    } else {
        0.0
    };
    let players = world
        .query::<PlayerAnimationRuntimeBinding>()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();

    for player in players {
        let animation_state = world
            .get::<newengine_engine_runtime::gameplay::PlayerAnimationState>(player)
            .copied()
            .unwrap_or_default();
        let rifle_aim_alpha = super::equipment_visual::equipped_weapon_aim_alpha(world, player);
        let rifle_recoil_alpha =
            super::equipment_visual::equipped_weapon_recoil_alpha(world, player);
        let first_person_active = world
            .resource::<newengine_engine_runtime::gameplay::PlayerViewState>()
            .copied()
            .unwrap_or_default()
            .first_person_active;
        let rifle_view_forward_model = if first_person_active || rifle_aim_alpha > 0.001 {
            player_rifle_view_forward_model(world, player)
        } else {
            None
        };
        let weapon_presentation = world
            .get::<newengine_engine_runtime::gameplay::EquippedWeaponBinding>(player)
            .copied()
            .and_then(|equipped| {
                world
                    .resource::<newengine_engine_runtime::gameplay::ItemCatalog>()?
                    .get(equipped.item)
                    .map(|definition| definition.weapon_presentation.clone().sanitized())
            })
            .filter(|presentation| presentation.enabled);
        let equipment_presentation_active = weapon_presentation.is_some()
            && world
                .get::<PlayerAnimationRuntimeBinding>(player)
                .is_some_and(|binding| {
                    binding.equipment_ready_pose.is_some()
                        || binding.equipment_aim_pose.is_some()
                        || binding.equipment_reload_pose.is_some()
                        || binding.equipment_ik.is_some()
                });
        let rifle_reload_progress = if equipment_presentation_active {
            world
                .get::<newengine_engine_runtime::gameplay::PlayerWeaponState>(player)
                .and_then(|state| {
                    (state.reload_remaining > 0.0).then(|| {
                        let duration = world
                            .get::<newengine_engine_runtime::gameplay::HitscanWeaponTuning>(player)
                            .map(|tuning| tuning.sanitized().reload_duration)
                            .filter(|duration| *duration > 1.0e-4)
                            .unwrap_or(2.0);
                        (1.0 - state.reload_remaining / duration).clamp(0.0, 1.0)
                    })
                })
        } else {
            None
        };
        let (palette, clip_ref, active_state) = {
            let Some(binding) = world.get_mut::<PlayerAnimationRuntimeBinding>(player) else {
                continue;
            };
            binding.equipment_time_seconds += dt;
            let desired_slot = binding.resolve_slot(animation_state.locomotion);
            let state_changed = binding.active_state != animation_state.locomotion;
            let slot_changed = binding.active_slot != desired_slot;
            let transitioned = state_changed || slot_changed;
            if slot_changed {
                // Cross-fade from the pose that was actually visible, not merely from
                // the previous clip. This keeps hands/forearms continuous even if the
                // player changes locomotion state again before the prior fade finishes.
                binding
                    .transition_from_locals
                    .clone_from(&binding.current_locals);
                binding.active_slot = desired_slot;
                binding.time_seconds = 0.0;
            }
            if state_changed {
                // A semantic transition is not necessarily a clip transition. Fall can
                // intentionally resolve to the active Jump slot when no authored fall
                // clip exists. Preserve playback time in that case so the airborne
                // phase continues through the apex instead of restarting the jump.
                binding.active_state = animation_state.locomotion;
            }
            if !slot_changed {
                let playback_rate = match animation_state.locomotion {
                    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Walk => {
                        (animation_state.normalized_speed / 0.40).clamp(0.65, 1.45)
                    }
                    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Run => {
                        (animation_state.normalized_speed / 0.85).clamp(0.75, 1.45)
                    }
                    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::Sprint => {
                        animation_state.normalized_speed.clamp(1.0, 1.65)
                    }
                    newengine_engine_runtime::gameplay::PlayerLocomotionAnimation::CrouchWalk => {
                        // Authored crouch speed is ~1.0 m/s while normalized_speed is expressed
                        // against the 3.0 m/s run speed. Keep foot cadence centered at 1x at
                        // full crouch speed and only stretch modestly near the movement threshold.
                        (animation_state.normalized_speed / 0.333_333_34).clamp(0.70, 1.25)
                    }
                    _ => 1.0,
                };
                binding.time_seconds += dt * playback_rate;
            }

            let active_slot = binding.active_slot;
            let active_state = binding.active_state;
            let active_clip = binding.clips[active_slot]
                .as_ref()
                .expect("resolved player animation slot must contain a clip");
            let clip_ref = active_clip.clip_ref.clone();
            if transitioned {
                newengine_ulog_api::ulog::info!(
                    "game-ready: player locomotion animation transition player={} state='{}' clip='{}' duration={:.3}s normalized_speed={:.3}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    active_clip.clip.duration_seconds,
                    animation_state.normalized_speed
                );
            }
            if let Err(error) = active_clip.clip.sample_local_pose_for_skeleton(
                binding.time_seconds,
                &binding.skeleton,
                &mut binding.sampled_target_locals,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: player animation sample failed player={} state='{}' clip='{}': {}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    error
                );
                continue;
            }

            if equipment_presentation_active {
                if let Some(progress) = rifle_reload_progress {
                    let overlay = binding.equipment_reload_pose.as_ref();
                    let overlay_ref = overlay
                        .map(|clip| clip.clip_ref.as_str())
                        .unwrap_or("<none>");
                    if let Err(error) = apply_equipment_rotation_overlay(
                        overlay,
                        &binding.skeleton,
                        &mut binding.equipment_overlay_locals,
                        &mut binding.sampled_target_locals,
                        progress,
                        binding.equipment_reload_rotation_weights.as_slice(),
                        1.0,
                    ) {
                        newengine_ulog_api::ulog::warn!(
                            "game-ready: authored equipment reload overlay failed player={} ref='{}' phase={:.3}: {}",
                            player.stable_u64(),
                            overlay_ref,
                            progress,
                            error,
                        );
                    }
                } else {
                    if let Err(error) = apply_equipment_rotation_overlay(
                        binding.equipment_ready_pose.as_ref(),
                        &binding.skeleton,
                        &mut binding.equipment_overlay_locals,
                        &mut binding.sampled_target_locals,
                        binding.equipment_ready_sample_phase,
                        binding.equipment_ready_rotation_weights.as_slice(),
                        1.0,
                    ) {
                        newengine_ulog_api::ulog::warn!(
                            "game-ready: authored equipment ready overlay failed player={}: {}",
                            player.stable_u64(),
                            error,
                        );
                    }
                    if rifle_aim_alpha > 0.001 {
                        let aim_phase = binding
                            .equipment_aim_pose
                            .as_ref()
                            .map(|clip| {
                                let duration = clip.clip.duration_seconds.max(1.0 / 30.0);
                                (binding.equipment_time_seconds.rem_euclid(duration) / duration)
                                    .clamp(0.0, 1.0)
                            })
                            .unwrap_or(0.0);
                        if let Err(error) = apply_equipment_rotation_overlay(
                            binding.equipment_aim_pose.as_ref(),
                            &binding.skeleton,
                            &mut binding.equipment_overlay_locals,
                            &mut binding.sampled_target_locals,
                            aim_phase,
                            binding.equipment_aim_rotation_weights.as_slice(),
                            rifle_aim_alpha,
                        ) {
                            newengine_ulog_api::ulog::warn!(
                                "game-ready: authored equipment aim overlay failed player={} phase={:.3} alpha={:.3}: {}",
                                player.stable_u64(),
                                aim_phase,
                                rifle_aim_alpha,
                                error,
                            );
                        }
                    }
                }
            }
            synchronize_helper_pose(
                &binding.helper_mirror_pairs,
                &mut binding.sampled_target_locals,
            );

            let alpha = if state_changed && !slot_changed {
                // Same-slot semantic continuation (notably Jump -> Fall fallback) must
                // not re-enter a cross-fade against stale transition_from_locals.
                1.0
            } else {
                animation_state.transition_alpha.clamp(0.0, 1.0)
            };
            if alpha < 1.0 {
                if let Err(error) = blend_local_poses(
                    &binding.transition_from_locals,
                    &binding.sampled_target_locals,
                    alpha,
                    &mut binding.current_locals,
                ) {
                    newengine_ulog_api::ulog::warn!(
                        "game-ready: player animation transition failed player={} state='{}' clip='{}': {}",
                        player.stable_u64(),
                        active_state.clip_hint(),
                        clip_ref,
                        error
                    );
                    continue;
                }
            } else {
                binding
                    .current_locals
                    .clone_from(&binding.sampled_target_locals);
            }

            if equipment_presentation_active {
                match apply_equipped_weapon_support_ik(
                    weapon_presentation
                        .as_ref()
                        .expect("active equipment presentation requires weapon descriptor"),
                    binding.equipment_ik.as_ref(),
                    &binding.skeleton,
                    binding.source_to_model,
                    &mut binding.current_locals,
                    &mut binding.joint_frames_scratch,
                    rifle_view_forward_model,
                    rifle_aim_alpha,
                    rifle_recoil_alpha,
                    rifle_reload_progress
                        .map(|progress| progress <= 0.08 || progress >= 0.92)
                        .unwrap_or(true),
                    rifle_reload_progress
                        .map(|progress| progress <= 0.08 || progress >= 0.92)
                        .unwrap_or(true),
                ) {
                    Ok(Some(error)) if error > 0.025 => {
                        newengine_ulog_api::ulog::warn!(
                            "game-ready: authored equipment support IK residual player={} error_m={:.5}",
                            player.stable_u64(),
                            error,
                        );
                    }
                    Ok(_) => {}
                    Err(error) => {
                        newengine_ulog_api::ulog::warn!(
                            "game-ready: authored equipment support IK failed player={}: {}",
                            player.stable_u64(),
                            error,
                        );
                    }
                }
            }
            synchronize_helper_pose(&binding.helper_mirror_pairs, &mut binding.current_locals);
            if let Err(error) = stabilize_eye_locals(
                binding.eye_contract.as_ref(),
                &binding.skeleton,
                &mut binding.current_locals,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: authored eye-local stabilization failed player={} clip='{}': {}",
                    player.stable_u64(),
                    clip_ref,
                    error
                );
                continue;
            }

            if let Err(error) = build_skin_palette_from_local_pose(
                &binding.skeleton,
                binding.source_to_model,
                &binding.current_locals,
                &mut binding.palette_scratch,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: player skin palette update failed player={} state='{}' clip='{}': {}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    error
                );
                continue;
            }
            if let Err(error) = apply_detached_head_follow_palette(
                binding.head_follow.as_ref(),
                &mut binding.palette_scratch,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: detached face/head follow projection failed player={} clip='{}': {}",
                    player.stable_u64(),
                    clip_ref,
                    error
                );
                continue;
            }
            if let Err(error) =
                validate_eye_palette(binding.eye_contract.as_ref(), &binding.palette_scratch)
            {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: authored eye palette rejected player={} clip='{}': {}",
                    player.stable_u64(),
                    clip_ref,
                    error
                );
                continue;
            }
            if transitioned {
                debug_dump_eye_matrices(
                    binding.eye_contract.as_ref(),
                    &binding.bind_joint_frames,
                    &binding.current_locals,
                    &binding.palette_scratch,
                    &format!("transition:{clip_ref}"),
                );
            }
            binding.joint_frames_scratch.clear();
            binding
                .joint_frames_scratch
                .reserve(binding.skeleton.joints.len());
            for index in 0..binding.skeleton.joints.len() {
                // Skin palette is a deformation matrix. Multiplying it by the authored bind
                // frame reconstructs the absolute current-frame joint transform after all
                // animation/head-follow corrections: P * (S*B) = S*A.
                let frame = binding.palette_scratch[index] * binding.bind_joint_frames[index];
                binding.joint_frames_scratch.push(frame);
            }
            let expected_palette_joints = binding.skeleton.joints.len();
            if let Err(error) = super::validation::validate_player_palette(
                &binding.palette_scratch,
                expected_palette_joints,
                &clip_ref,
            ) {
                newengine_ulog_api::ulog::warn!(
                    "game-ready: unstable player skin palette rejected player={} state='{}' clip='{}': {}",
                    player.stable_u64(),
                    active_state.clip_hint(),
                    clip_ref,
                    error
                );
                continue;
            }
            (
                std::mem::take(&mut binding.palette_scratch),
                clip_ref,
                active_state,
            )
        };

        let recycled_palette = if let Some(pose) =
            world.get_mut::<newengine_engine_runtime::gameplay::PlayerSkinPose>(player)
        {
            let recycled = std::mem::replace(&mut pose.palette, palette);
            pose.revision = pose.revision.saturating_add(1).max(1);
            Some(recycled)
        } else {
            let _ = world.insert(
                player,
                newengine_engine_runtime::gameplay::PlayerSkinPose {
                    palette,
                    revision: 1,
                },
            );
            None
        };
        if let Some(recycled_palette) = recycled_palette {
            if let Some(binding) = world.get_mut::<PlayerAnimationRuntimeBinding>(player) {
                binding.palette_scratch = recycled_palette;
            }
        }
        if dt > 0.0
            && world
                .get::<newengine_engine_runtime::gameplay::PlayerSkinPose>(player)
                .is_some_and(|pose| pose.revision == 2)
        {
            newengine_ulog_api::ulog::info!(
                "game-ready: first animated player palette committed player={} state='{}' clip='{}'",
                player.stable_u64(),
                active_state.clip_hint(),
                clip_ref
            );
        }
    }
}

#[cfg(test)]
mod transition_tests {
    use super::*;

    #[test]
    fn rifle_ready_pole_ik_converges_without_moving_stock_anchored_weapon() {
        use newengine_model_skeleton_api::{ModelSkeletonAnchors, ModelSkeletonJointMetadata};

        let names = [
            "root",
            "spined",
            "r_shoulder",
            "r_elbow",
            "r_wrist",
            "r_palm",
            "l_shoulder",
            "l_elbow",
            "l_wrist",
            "l_palm",
        ];
        let joint = |index: u32, parent_index: Option<u32>, position_ls: [f32; 3]| {
            ModelSkeletonJointMetadata {
                index,
                tag: index,
                name: names[index as usize].to_owned(),
                parent: parent_index.map(|parent| names[parent as usize].to_owned()),
                parent_index,
                position_ls,
                rotation_ls: [0.0, 0.0, 0.0, 1.0],
                scale_ls: [1.0, 1.0, 1.0],
                flags: Vec::new(),
            }
        };
        let skeleton = ModelSkeletonMetadata {
            source: "test".to_owned(),
            source_format: "test".to_owned(),
            container_magic: "TEST".to_owned(),
            byte_len: 0,
            content_hash: String::new(),
            decode_status: "ok".to_owned(),
            joints: vec![
                joint(0, None, [0.0, 0.0, 0.0]),
                joint(1, Some(0), [0.0, 1.285_745, 0.0]),
                joint(2, Some(1), [-0.17, 0.06, 0.0]),
                // Real Abby arm lengths are roughly 0.26 m upper arm and 0.25 m forearm/hand.
                joint(3, Some(2), [0.0, -0.26, 0.0]),
                joint(4, Some(3), [0.0, -0.24, 0.0]),
                joint(5, Some(4), [0.0, -0.015, 0.0]),
                joint(6, Some(1), [0.17, 0.06, 0.0]),
                joint(7, Some(6), [0.0, -0.26, 0.0]),
                joint(8, Some(7), [0.0, -0.24, 0.0]),
                joint(9, Some(8), [0.0, -0.015, 0.0]),
            ],
            anchors: ModelSkeletonAnchors {
                root: "root".to_owned(),
                hips: "root".to_owned(),
                head: "spined".to_owned(),
                left_hand: "l_palm".to_owned(),
                right_hand: "r_palm".to_owned(),
                left_foot: "root".to_owned(),
                right_foot: "root".to_owned(),
                eye: "spined".to_owned(),
                eye_height: 0.0,
            },
        };
        let mut pose = skeleton
            .joints
            .iter()
            .map(|joint| JointLocalPose {
                translation: joint.position_ls,
                rotation: joint.rotation_ls,
                scale: Some(joint.scale_ls),
            })
            .collect::<Vec<_>>();
        let rig = build_weapon_arm_ik_rig(&skeleton).expect("rifle IK rig");
        let source_to_model = [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];
        let mut frames = Vec::new();
        rebuild_model_joint_frames(&skeleton, source_to_model, &pose, &mut frames)
            .expect("initial frames");
        let contract_before = crate::weapon_grip::rifle_ready_solve_contract(
            frames[rig.chest],
            frames[rig.right_shoulder],
            frames[rig.left_shoulder],
        )
        .expect("ReadyHold solve contract");
        let root_before = contract_before.root;
        let right_target = crate::weapon_grip::weapon_ready_right_palm_position(root_before);
        let left_target = crate::weapon_grip::weapon_ready_left_palm_position(root_before);
        let initial_error = (
            (frames[rig.right_palm].transform_point3(Vec3::ZERO) - right_target).length(),
            (frames[rig.left_palm].transform_point3(Vec3::ZERO) - left_target).length(),
        );

        let final_error = apply_equipped_rifle_support_ik(
            Some(&rig),
            &skeleton,
            source_to_model,
            &mut pose,
            &mut frames,
            None,
            0.0,
            0.0,
            true,
        )
        .expect("bilateral ReadyHold IK")
        .expect("IK enabled");

        let final_right =
            (frames[rig.right_palm].transform_point3(Vec3::ZERO) - right_target).length();
        let final_left =
            (frames[rig.left_palm].transform_point3(Vec3::ZERO) - left_target).length();
        assert!(
            final_right < initial_error.0,
            "right initial={} final={final_right}",
            initial_error.0
        );
        assert!(
            final_left < initial_error.1,
            "left initial={} final={final_left}",
            initial_error.1
        );
        assert!(final_error < 0.035, "final={final_error}");

        let contract_after = crate::weapon_grip::rifle_ready_solve_contract(
            frames[rig.chest],
            frames[rig.right_shoulder],
            frames[rig.left_shoulder],
        )
        .expect("ReadyHold solve contract after IK");
        let root_after = contract_after.root;
        assert!((root_before.position - root_after.position).length() < 1.0e-6);
        assert!(root_before.rotation.dot(root_after.rotation).abs() > 0.999_999);
        assert!((contract_after.stock_contact - contract_after.shoulder_pocket).length() < 1.0e-6);
    }

    #[test]
    fn detached_control_and_face_share_the_same_canonical_headb_delta() {
        let rig = DetachedHeadFollowRig {
            headb_driver: 0,
            control_followers: vec![1],
            face_followers: vec![2],
        };
        let mut palette = vec![Mat4::IDENTITY; 3];
        palette[0] = Mat4::from_translation(Vec3::new(0.2, 0.1, -0.3));
        palette[1] = Mat4::from_translation(Vec3::new(0.0, 0.02, 0.0));
        palette[2] = Mat4::from_translation(Vec3::new(0.0, 0.03, 0.0));

        apply_detached_head_follow_palette(Some(&rig), &mut palette).expect("projection");

        let control = palette[1].transform_point3(Vec3::ZERO);
        assert!((control.x - 0.2).abs() < 1.0e-5);
        assert!((control.y - 0.12).abs() < 1.0e-5);
        assert!((control.z + 0.3).abs() < 1.0e-5);

        // The face gets headb + its own detached deformation only. It must not
        // receive the MCH control deformation a second time (old result y=0.15).
        let face = palette[2].transform_point3(Vec3::ZERO);
        assert!((face.x - 0.2).abs() < 1.0e-5);
        assert!((face.y - 0.13).abs() < 1.0e-5);
        assert!((face.z + 0.3).abs() < 1.0e-5);
    }

    #[test]
    fn native_abby_eye_palette_enforces_parent_deformation_invariant() {
        let contract = EyeRuntimeContract {
            parent: 0,
            left: 1,
            right: 2,
        };
        let head_delta = Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, 1.0, 1.0),
            Quat::from_rotation_y(0.25),
            Vec3::new(0.2, 0.1, -0.3),
        );
        let mut palette = vec![head_delta, head_delta, head_delta];
        validate_eye_palette(Some(&contract), &palette).expect("stable eyes");

        palette[contract.left] = Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, 1.0, 1.0),
            Quat::from_rotation_x(0.08),
            Vec3::ZERO,
        ) * palette[contract.left];
        let error = validate_eye_palette(Some(&contract), &palette)
            .expect_err("extra eye deformation must be rejected");
        assert!(error.contains("eye palette drift"));
    }

    #[test]
    fn local_pose_crossfade_preserves_endpoints_and_shortest_quaternion_path() {
        let from = [JointLocalPose {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: Some([1.0, 1.0, 1.0]),
        }];
        let to = [JointLocalPose {
            translation: [2.0, 4.0, 6.0],
            // Same identity rotation with opposite quaternion sign.
            rotation: [0.0, 0.0, 0.0, -1.0],
            scale: Some([1.0, 1.0, 1.0]),
        }];
        let mut out = Vec::new();
        blend_local_poses(&from, &to, 0.5, &mut out).expect("blend");
        assert_eq!(out.len(), 1);
        assert!((out[0].translation[0] - 1.0).abs() <= 1.0e-6);
        assert!((out[0].translation[1] - 2.0).abs() <= 1.0e-6);
        assert!((out[0].translation[2] - 3.0).abs() <= 1.0e-6);
        assert!(out[0].rotation[0].abs() <= 1.0e-6);
        assert!(out[0].rotation[1].abs() <= 1.0e-6);
        assert!(out[0].rotation[2].abs() <= 1.0e-6);
        assert!((out[0].rotation[3].abs() - 1.0).abs() <= 1.0e-6);
    }
}
