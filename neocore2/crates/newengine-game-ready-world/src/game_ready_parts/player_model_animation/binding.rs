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
