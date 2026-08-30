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

#[derive(Clone, Copy, Debug)]
struct ResolvedJointBlendRule {
    joint_index: usize,
    joint_tag: u32,
    weight: f32,
    channels: newengine_engine_runtime::gameplay::PlayerJointChannels,
}

fn resolve_joint_blend_rules(
    skeleton: &ModelSkeletonMetadata,
    rules: &[newengine_engine_runtime::gameplay::PlayerJointRotationWeight],
) -> Result<Vec<ResolvedJointBlendRule>, String> {
    let mut resolved = Vec::with_capacity(rules.len());
    for rule in rules {
        let joint_name = rule.joint.trim();
        let index = skeleton
            .joints
            .iter()
            .position(|joint| joint.name == joint_name)
            .ok_or_else(|| {
                format!(
                    "authored animation layer joint is absent from skeleton joint='{joint_name}'"
                )
            })?;
        if !rule.weight.is_finite() || !(0.0..=1.0).contains(&rule.weight) || !rule.channels.any() {
            return Err(format!(
                "authored animation layer rule is invalid joint='{joint_name}' weight={} channels={:?}",
                rule.weight, rule.channels
            ));
        }
        resolved.push(ResolvedJointBlendRule {
            joint_index: index,
            joint_tag: skeleton.joints[index].tag,
            weight: rule.weight,
            channels: rule.channels,
        });
    }
    Ok(resolved)
}

fn resolve_foot_joint_binding(skeleton: &ModelSkeletonMetadata) -> Option<PlayerFootJointBinding> {
    fn find_joint(skeleton: &ModelSkeletonMetadata, authored: &str, left: bool) -> Option<usize> {
        let root = skeleton.anchors.root.as_str();
        let hips = skeleton.anchors.hips.as_str();
        if !authored.trim().is_empty() && authored != root && authored != hips {
            if let Some(index) = skeleton
                .joints
                .iter()
                .position(|joint| joint.name == authored)
            {
                return Some(index);
            }
        }
        let patterns: &[&str] = if left {
            &[
                "left_foot",
                "foot_l",
                "l_foot",
                "leftfoot",
                "left_ankle",
                "ankle_l",
                "l_ankle",
            ]
        } else {
            &[
                "right_foot",
                "foot_r",
                "r_foot",
                "rightfoot",
                "right_ankle",
                "ankle_r",
                "r_ankle",
            ]
        };
        skeleton.joints.iter().position(|joint| {
            let name = joint
                .name
                .to_ascii_lowercase()
                .replace('.', "_")
                .replace(':', "_")
                .replace('-', "_");
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
    /// Definition-authored local-pose copy rules resolved to this skeleton.
    helper_pose_copies: Vec<ResolvedJointCopyRule>,
    /// Imported Rigify control/face branches need the authored constraint order restored:
    /// deform body -> animated neck/head controls -> face/eyes deform branches.
    eye_contract: Option<EyeRuntimeContract>,
    head_follow: Option<DetachedHeadFollowRig>,
    noclip_pose: Option<PlayerAnimationRuntimeClip>,
    noclip_time_seconds: f32,
    noclip_active: bool,
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
    equipment_ready_rotation_weights: Vec<ResolvedJointBlendRule>,
    equipment_aim_rotation_weights: Vec<ResolvedJointBlendRule>,
    equipment_reload_rotation_weights: Vec<ResolvedJointBlendRule>,
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

#[inline]
fn blend_joint_scale_only(dst: &mut JointLocalPose, src: &JointLocalPose, weight: f32) {
    let weight = if weight.is_finite() {
        weight.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let from = dst.scale.unwrap_or([1.0, 1.0, 1.0]);
    let to = src.scale.unwrap_or([1.0, 1.0, 1.0]);
    dst.scale = Some([
        from[0] + (to[0] - from[0]) * weight,
        from[1] + (to[1] - from[1]) * weight,
        from[2] + (to[2] - from[2]) * weight,
    ]);
}

fn apply_character_rotation_overlay(
    clip: Option<&PlayerAnimationRuntimeClip>,
    skeleton: &ModelSkeletonMetadata,
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
    clip.clip
        .sample_local_pose_bound(sample_time, animation_runtime, &clip.binding, scratch)?;
    for (index, (dst, src)) in target.iter_mut().zip(scratch.iter()).enumerate() {
        let Some(joint) = skeleton.joints.get(index) else {
            continue;
        };
        // Untracked clip channels are bind-pose completion, not authored overlay data. Preserve
        // the current base locomotion pose unless this clip explicitly owns the joint tag.
        if clip.clip.joint_tags.contains(&joint.tag) {
            blend_joint_rotation_only(dst, src, 1.0);
        }
    }
    Ok(())
}

fn apply_equipment_rotation_overlay(
    clip: Option<&PlayerAnimationRuntimeClip>,
    animation_runtime: &AnimationSkeletonRuntime,
    scratch: &mut Vec<JointLocalPose>,
    target: &mut [JointLocalPose],
    normalized_phase: f32,
    weights: &[ResolvedJointBlendRule],
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
        .sample_local_pose_bound(sample_time, animation_runtime, &clip.binding, scratch)?;
    for rule in weights {
        // Sampling returns a complete pose by filling absent clip channels from bind pose. That
        // fallback is not layer-authored data. A project layer may only modify a joint when the
        // selected clip explicitly owns that joint tag; otherwise the live base pose is preserved.
        if !clip.clip.joint_tags.contains(&rule.joint_tag) {
            continue;
        }
        if let (Some(dst), Some(src)) = (
            target.get_mut(rule.joint_index),
            scratch.get(rule.joint_index),
        ) {
            let effective_weight = (rule.weight * weight_scale).clamp(0.0, 1.0);
            if rule.channels.translation {
                blend_joint_translation_only(dst, src, effective_weight);
            }
            if rule.channels.rotation {
                blend_joint_rotation_only(dst, src, effective_weight);
            }
            if rule.channels.scale {
                blend_joint_scale_only(dst, src, effective_weight);
            }
        }
    }
    Ok(())
}
