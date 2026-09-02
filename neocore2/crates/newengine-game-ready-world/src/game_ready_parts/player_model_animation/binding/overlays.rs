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

fn apply_equipment_full_body_directional_pose(
    clip_a: Option<&PlayerAnimationRuntimeClip>,
    clip_b: Option<&PlayerAnimationRuntimeClip>,
    blend_to_b: f32,
    animation_runtime: &AnimationSkeletonRuntime,
    scratch_a: &mut Vec<JointLocalPose>,
    scratch_b: &mut Vec<JointLocalPose>,
    target: &mut [JointLocalPose],
    normalized_phase: f32,
    root_joint: Option<usize>,
    weight_scale: f32,
) -> Result<bool, String> {
    let Some(clip_a) = clip_a.or(clip_b) else {
        return Ok(false);
    };
    let clip_b = clip_b.unwrap_or(clip_a);
    let phase = if normalized_phase.is_finite() {
        normalized_phase.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let sample_a =
        (clip_a.clip.duration_seconds * phase).clamp(0.0, clip_a.clip.duration_seconds.max(0.0));
    let sample_b =
        (clip_b.clip.duration_seconds * phase).clamp(0.0, clip_b.clip.duration_seconds.max(0.0));
    clip_a
        .clip
        .sample_local_pose_bound(sample_a, animation_runtime, &clip_a.binding, scratch_a)?;
    clip_b
        .clip
        .sample_local_pose_bound(sample_b, animation_runtime, &clip_b.binding, scratch_b)?;
    let blend_to_b = if blend_to_b.is_finite() {
        blend_to_b.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let weight_scale = if weight_scale.is_finite() {
        weight_scale.clamp(0.0, 1.0)
    } else {
        1.0
    };

    let mut apply_joint = |joint_index: usize, owns_a: bool, owns_b: bool| {
        if Some(joint_index) == root_joint {
            return;
        }
        let Some(dst) = target.get_mut(joint_index) else {
            return;
        };
        let src_a = scratch_a.get(joint_index);
        let src_b = scratch_b.get(joint_index);
        let source = match (owns_a, owns_b, src_a, src_b) {
            (true, true, Some(a), Some(b)) => {
                let mut blended = *a;
                blend_joint_translation_only(&mut blended, b, blend_to_b);
                blend_joint_rotation_only(&mut blended, b, blend_to_b);
                blend_joint_scale_only(&mut blended, b, blend_to_b);
                blended
            }
            (true, _, Some(a), _) => *a,
            (_, true, _, Some(b)) => *b,
            _ => return,
        };
        let directional_availability = match (owns_a, owns_b) {
            (true, false) => 1.0 - blend_to_b,
            (false, true) => blend_to_b,
            _ => 1.0,
        };
        let weight = (weight_scale * directional_availability).clamp(0.0, 1.0);
        blend_joint_translation_only(dst, &source, weight);
        blend_joint_rotation_only(dst, &source, weight);
        blend_joint_scale_only(dst, &source, weight);
    };

    for &joint_index in clip_a.binding.skeleton_joint_indices() {
        apply_joint(
            joint_index,
            true,
            clip_b.binding.owns_skeleton_joint(joint_index),
        );
    }
    if !std::ptr::eq(clip_a, clip_b) {
        for &joint_index in clip_b.binding.skeleton_joint_indices() {
            if !clip_a.binding.owns_skeleton_joint(joint_index) {
                apply_joint(joint_index, false, true);
            }
        }
    }
    Ok(true)
}

/// Apply a domain-qualified articulation clip to exactly the skeleton joints it owns.
/// This is intentionally independent of the upper-body equipment weight mask: compact hand
/// dictionaries are projected to anatomical finger joints only, so their binding is the mask.
fn apply_equipment_owned_rotation_overlay(
    clip: Option<&PlayerAnimationRuntimeClip>,
    animation_runtime: &AnimationSkeletonRuntime,
    scratch: &mut Vec<JointLocalPose>,
    target: &mut [JointLocalPose],
    normalized_phase: f32,
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
    let weight = if weight_scale.is_finite() {
        weight_scale.clamp(0.0, 1.0)
    } else {
        1.0
    };
    for &joint_index in clip.binding.skeleton_joint_indices() {
        if let (Some(dst), Some(src)) = (target.get_mut(joint_index), scratch.get(joint_index)) {
            blend_joint_rotation_only(dst, src, weight);
        }
    }
    Ok(())
}

fn apply_equipment_additive_overlay(
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
        if !clip.clip.joint_tags.contains(&rule.joint_tag) {
            continue;
        }
        let (Some(dst), Some(delta)) = (
            target.get_mut(rule.joint_index),
            scratch.get(rule.joint_index),
        ) else {
            continue;
        };
        let weight = (rule.weight * weight_scale).clamp(0.0, 1.0);
        if rule.channels.translation {
            let base = Vec3::new(dst.translation[0], dst.translation[1], dst.translation[2]);
            let authored_delta = Vec3::new(
                delta.translation[0],
                delta.translation[1],
                delta.translation[2],
            );
            let composed = base + authored_delta * weight;
            dst.translation = [composed.x, composed.y, composed.z];
        }
        if rule.channels.rotation {
            let base = Quat::from_xyzw(
                dst.rotation[0],
                dst.rotation[1],
                dst.rotation[2],
                dst.rotation[3],
            )
            .normalize_or_identity();
            let authored_delta = Quat::from_xyzw(
                delta.rotation[0],
                delta.rotation[1],
                delta.rotation[2],
                delta.rotation[3],
            )
            .normalize_or_identity();
            let weighted_delta = Quat::IDENTITY
                .slerp(authored_delta, weight)
                .normalize_or_identity();
            let composed = (base * weighted_delta).normalize_or_identity();
            dst.rotation = [composed.x, composed.y, composed.z, composed.w];
        }
        if rule.channels.scale {
            let base = dst.scale.unwrap_or([1.0, 1.0, 1.0]);
            let authored_delta = delta.scale.unwrap_or([1.0, 1.0, 1.0]);
            dst.scale = Some([
                base[0] * (1.0 + (authored_delta[0] - 1.0) * weight),
                base[1] * (1.0 + (authored_delta[1] - 1.0) * weight),
                base[2] * (1.0 + (authored_delta[2] - 1.0) * weight),
            ]);
        }
    }
    Ok(())
}

fn apply_equipment_relative_delta_overlay(
    reference: Option<&PlayerAnimationRuntimeClip>,
    additive: Option<&PlayerAnimationRuntimeClip>,
    animation_runtime: &AnimationSkeletonRuntime,
    scratch_reference: &mut Vec<JointLocalPose>,
    scratch_additive: &mut Vec<JointLocalPose>,
    target: &mut [JointLocalPose],
    normalized_phase: f32,
    weights: &[ResolvedJointBlendRule],
    weight_scale: f32,
) -> Result<(), String> {
    let Some(additive) = additive else {
        return Ok(());
    };
    let phase = if normalized_phase.is_finite() {
        normalized_phase.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let additive_time = (additive.clip.duration_seconds * phase)
        .clamp(0.0, additive.clip.duration_seconds.max(0.0));
    additive.clip.sample_local_pose_bound(
        additive_time,
        animation_runtime,
        &additive.binding,
        scratch_additive,
    )?;
    if let Some(reference) = reference {
        let reference_time = (reference.clip.duration_seconds * phase)
            .clamp(0.0, reference.clip.duration_seconds.max(0.0));
        reference.clip.sample_local_pose_bound(
            reference_time,
            animation_runtime,
            &reference.binding,
            scratch_reference,
        )?;
    }
    for rule in weights {
        if !additive.clip.joint_tags.contains(&rule.joint_tag) {
            continue;
        }
        let (Some(dst), Some(add)) = (
            target.get_mut(rule.joint_index),
            scratch_additive.get(rule.joint_index),
        ) else {
            continue;
        };
        let reference_pose = reference
            .filter(|clip| clip.clip.joint_tags.contains(&rule.joint_tag))
            .and_then(|_| scratch_reference.get(rule.joint_index));
        let weight = (rule.weight * weight_scale).clamp(0.0, 1.0);
        if rule.channels.translation {
            let add_translation =
                Vec3::new(add.translation[0], add.translation[1], add.translation[2]);
            let reference_translation = reference_pose
                .map(|pose| {
                    Vec3::new(
                        pose.translation[0],
                        pose.translation[1],
                        pose.translation[2],
                    )
                })
                .unwrap_or(Vec3::ZERO);
            let composed = Vec3::new(dst.translation[0], dst.translation[1], dst.translation[2])
                + (add_translation - reference_translation) * weight;
            dst.translation = [composed.x, composed.y, composed.z];
        }
        if rule.channels.rotation {
            let add_rotation = Quat::from_xyzw(
                add.rotation[0],
                add.rotation[1],
                add.rotation[2],
                add.rotation[3],
            )
            .normalize_or_identity();
            let reference_rotation = reference_pose
                .map(|pose| {
                    Quat::from_xyzw(
                        pose.rotation[0],
                        pose.rotation[1],
                        pose.rotation[2],
                        pose.rotation[3],
                    )
                    .normalize_or_identity()
                })
                .unwrap_or(Quat::IDENTITY);
            let delta = (reference_rotation.inverse() * add_rotation).normalize_or_identity();
            let weighted_delta = Quat::IDENTITY.slerp(delta, weight).normalize_or_identity();
            let base = Quat::from_xyzw(
                dst.rotation[0],
                dst.rotation[1],
                dst.rotation[2],
                dst.rotation[3],
            )
            .normalize_or_identity();
            let composed = (base * weighted_delta).normalize_or_identity();
            dst.rotation = [composed.x, composed.y, composed.z, composed.w];
        }
        if rule.channels.scale {
            let add_scale = add.scale.unwrap_or([1.0, 1.0, 1.0]);
            let reference_scale = reference_pose
                .and_then(|pose| pose.scale)
                .unwrap_or([1.0, 1.0, 1.0]);
            let base = dst.scale.unwrap_or([1.0, 1.0, 1.0]);
            let ratio = [
                if reference_scale[0].abs() > 1.0e-6 {
                    add_scale[0] / reference_scale[0]
                } else {
                    1.0
                },
                if reference_scale[1].abs() > 1.0e-6 {
                    add_scale[1] / reference_scale[1]
                } else {
                    1.0
                },
                if reference_scale[2].abs() > 1.0e-6 {
                    add_scale[2] / reference_scale[2]
                } else {
                    1.0
                },
            ];
            dst.scale = Some([
                base[0] * (1.0 + (ratio[0] - 1.0) * weight),
                base[1] * (1.0 + (ratio[1] - 1.0) * weight),
                base[2] * (1.0 + (ratio[2] - 1.0) * weight),
            ]);
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
