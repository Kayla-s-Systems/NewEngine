#[derive(Clone, Debug)]
struct PlayerAnimationRuntimeClip {
    clip_ref: String,
    clip: std::sync::Arc<AnimationClip>,
    binding: AnimationClipBinding,
    event_cursor: AnimationEventCursor,
}


#[derive(Clone, Copy, Debug)]
struct PlayerFootJointBinding {
    left: usize,
    right: usize,
}

fn resolve_foot_joint_binding(skeleton: &ModelSkeletonMetadata) -> Option<PlayerFootJointBinding> {
    fn find_joint(skeleton: &ModelSkeletonMetadata, authored: &str, left: bool) -> Option<usize> {
        let root = skeleton.anchors.root.as_str();
        let hips = skeleton.anchors.hips.as_str();
        if !authored.trim().is_empty() && authored != root && authored != hips {
            if let Some(index) = skeleton.joints.iter().position(|joint| joint.name == authored) {
                return Some(index);
            }
        }
        let patterns: &[&str] = if left {
            &["left_foot", "foot_l", "l_foot", "leftfoot", "left_ankle", "ankle_l", "l_ankle"]
        } else {
            &["right_foot", "foot_r", "r_foot", "rightfoot", "right_ankle", "ankle_r", "r_ankle"]
        };
        skeleton.joints.iter().position(|joint| {
            let name = joint.name.to_ascii_lowercase().replace('.', "_").replace(':', "_").replace('-', "_");
            patterns.iter().any(|pattern| {
                name == *pattern || name.starts_with(pattern) || name.ends_with(pattern)
            })
        })
    }

    let left = find_joint(skeleton, &skeleton.anchors.left_foot, true)?;
    let right = find_joint(skeleton, &skeleton.anchors.right_foot, false)?;
    (left != right).then_some(PlayerFootJointBinding { left, right })
}

#[derive(Clone, Debug)]
pub(super) struct PlayerAnimationRuntimeBinding {
    clips: [Option<PlayerAnimationRuntimeClip>; 8],
    active_state: newengine_engine_runtime::gameplay::PlayerLocomotionAnimation,
    active_slot: usize,
    locomotion_graph: std::sync::Arc<CompiledAnimationGraph>,
    locomotion_graph_instance: AnimationGraphInstance,
    locomotion_graph_evaluation: AnimationGraphEvaluation,
    skeleton: ModelSkeletonMetadata,
    animation_runtime: AnimationSkeletonRuntime,
    /// Pose currently visible on the character. This is preserved when a new
    /// locomotion state interrupts an in-flight cross-fade.
    current_locals: Vec<JointLocalPose>,
    sampled_target_locals: Vec<JointLocalPose>,
    palette_scratch: Vec<Mat4>,
    /// Absolute bind joint frames in baked model space. Current animated frames are derived as
    /// `skin_palette * bind_frame`, after all pose/follower corrections but before braid solve.
    bind_joint_frames: Vec<Mat4>,
    joint_frames_scratch: Vec<Mat4>,
    foot_joints: Option<PlayerFootJointBinding>,
    braid_secondary_motion: Option<AbbyBraidRuntime>,
    /// Mirrored North Star deform/helper branches must follow their primary joints.
    helper_mirror_pairs: Vec<(usize, usize)>,
    /// Imported Rigify control/face branches need the authored constraint order restored:
    /// deform body -> animated neck/head controls -> face/eyes deform branches.
    eye_contract: Option<EyeRuntimeContract>,
    head_follow: Option<DetachedHeadFollowRig>,
    equipment_ready_pose: Option<PlayerAnimationRuntimeClip>,
    equipment_aim_pose: Option<PlayerAnimationRuntimeClip>,
    equipment_reload_pose: Option<PlayerAnimationRuntimeClip>,
    unarmed_ready_pose: Option<PlayerAnimationRuntimeClip>,
    unarmed_attack_pose: Option<PlayerAnimationRuntimeClip>,
    unarmed_attack_sequence: u64,
    unarmed_attack_time_seconds: f32,
    equipment_ready_sample_phase: f32,
    equipment_time_seconds: f32,
    equipment_reload_active: bool,
    equipment_ready_rotation_weights: Vec<(String, f32)>,
    equipment_aim_rotation_weights: Vec<(String, f32)>,
    equipment_reload_rotation_weights: Vec<(String, f32)>,
    equipment_overlay_locals: Vec<JointLocalPose>,
    equipment_ik: Option<WeaponArmIkRig>,
    /// Torso-owned, reach-fitted weapon root before secondary dynamics. Render consumes this exact root.
    equipment_resolved_weapon_root: Option<crate::weapon_grip::WeaponRootTransform>,
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
        resolve_locomotion_slot(&self.clips, state)
    }
}

#[inline]
fn equipment_overlay_uses_authored_translation(name: &str) -> bool {
    matches!(
        name,
        "l_hand_prop"
            | "r_hand_prop"
            | "l_hand_prop_attachment"
            | "r_hand_prop_attachment"
    )
}

#[inline]
fn blend_joint_translation_only(dst: &mut JointLocalPose, src: &JointLocalPose, weight: f32) {
    let weight = if weight.is_finite() {
        weight.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let from = Vec3::new(dst.translation[0], dst.translation[1], dst.translation[2]);
    let to = Vec3::new(src.translation[0], src.translation[1], src.translation[2]);
    let translation = from.lerp(to, weight);
    dst.translation = [translation.x, translation.y, translation.z];
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

fn apply_character_rotation_overlay(
    clip: Option<&PlayerAnimationRuntimeClip>,
    animation_runtime: &AnimationSkeletonRuntime,
    scratch: &mut Vec<JointLocalPose>,
    target: &mut [JointLocalPose],
    normalized_phase: f32,
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
    clip.clip.sample_local_pose_bound(
        sample_time,
        animation_runtime,
        &clip.binding,
        scratch,
    )?;
    for (dst, src) in target.iter_mut().zip(scratch.iter()) {
        blend_joint_rotation_only(dst, src, 1.0);
    }
    Ok(())
}

fn apply_equipment_rotation_overlay(
    clip: Option<&PlayerAnimationRuntimeClip>,
    skeleton: &ModelSkeletonMetadata,
    animation_runtime: &AnimationSkeletonRuntime,
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
    clip.clip.sample_local_pose_bound(
        sample_time,
        animation_runtime,
        &clip.binding,
        scratch,
    )?;
    for (name, weight) in weights {
        let Some(index) = skeleton
            .joints
            .iter()
            .position(|joint| joint.name == name.as_str())
        else {
            continue;
        };
        // `sample_local_pose_for_skeleton` fills missing clip channels from bind pose so it can
        // return a complete skeleton pose. For an overlay that fallback is NOT authored data:
        // applying it would erase the live locomotion/stance channel back to bind. Only joints
        // explicitly present in the partial YCD clip are allowed to participate in this layer.
        let joint_tag = skeleton.joints[index].tag;
        if !clip.clip.joint_tags.contains(&joint_tag) {
            continue;
        }
        if let (Some(dst), Some(src)) = (target.get_mut(index), scratch.get(index)) {
            let effective_weight = (*weight * weight_scale).clamp(0.0, 1.0);
            blend_joint_rotation_only(dst, src, effective_weight);
            // Naughty Dog hand-prop joints are animated constraint/contact frames. Keeping only
            // their rotation leaves the weapon contact at bind-pose translation and visibly
            // detaches the rifle from the authored hand pose. Only these dedicated prop channels
            // inherit translation; torso/locomotion translation remains owned by the base clip.
            if equipment_overlay_uses_authored_translation(name) {
                blend_joint_translation_only(dst, src, effective_weight);
            }
        }
    }
    Ok(())
}
