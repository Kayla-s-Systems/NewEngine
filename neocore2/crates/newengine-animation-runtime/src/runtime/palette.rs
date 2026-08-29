/// Builds and validates the authored bind-pose skin palette.
///
/// A correct bind pose must reduce to identity in model space for every joint.
/// This is intentionally computed through the same hierarchy/source-space math as
/// animated palettes instead of returning `Mat4::IDENTITY` blindly, so malformed
/// skeleton hierarchy or source transforms fail before reaching the GPU.
pub fn build_bind_pose_palette(
    skeleton: &ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    out_palette: &mut Vec<Mat4>,
) -> Result<(), String> {
    let joint_count = skeleton.joints.len();
    if joint_count == 0 {
        return Err("bind-pose palette requires at least one skeleton joint".to_owned());
    }
    for (index, joint) in skeleton.joints.iter().enumerate() {
        if joint.index as usize != index {
            return Err(format!(
                "skeleton joint indices must be dense index={} authored={}",
                index, joint.index
            ));
        }
    }
    let bind_locals = skeleton
        .joints
        .iter()
        .map(|joint| JointLocalPose {
            translation: joint.position_ls,
            rotation: joint.rotation_ls,
            scale: Some(joint.scale_ls),
        })
        .collect::<Vec<_>>();
    let bind_globals = build_globals(skeleton, &bind_locals)?;
    let source_to_model = Mat4::from_cols_array(&source_to_model);
    if !affine_invertible(source_to_model) {
        return Err("skin source-to-model transform is singular/non-finite".to_owned());
    }
    let model_to_source = source_to_model.inverse();
    out_palette.clear();
    out_palette.reserve(joint_count);
    let mut max_identity_error = 0.0_f32;
    let mut max_joint = 0usize;
    for (index, bind) in bind_globals.into_iter().enumerate() {
        if !affine_invertible(bind) {
            return Err(format!("bind global matrix is singular joint={index}"));
        }
        let palette = source_to_model * (bind * bind.inverse()) * model_to_source;
        let values = palette.to_cols_array();
        if values.iter().any(|value| !value.is_finite()) {
            return Err(format!(
                "bind-pose palette contains non-finite value joint={index}"
            ));
        }
        let identity = Mat4::IDENTITY.to_cols_array();
        let error = values
            .iter()
            .zip(identity.iter())
            .map(|(actual, expected)| (actual - expected).abs())
            .fold(0.0_f32, f32::max);
        if error > max_identity_error {
            max_identity_error = error;
            max_joint = index;
        }
        out_palette.push(palette);
    }
    const MAX_BIND_PALETTE_IDENTITY_ERROR: f32 = 1.0e-4;
    if max_identity_error > MAX_BIND_PALETTE_IDENTITY_ERROR {
        return Err(format!(
            "bind-pose palette is not identity max_error={max_identity_error:.8} joint={max_joint} limit={MAX_BIND_PALETTE_IDENTITY_ERROR}"
        ));
    }
    Ok(())
}

/// Builds skin matrices in baked model space.
///
/// The clip and skeleton remain in the authored source space. The final conjugation
/// by `source_to_model` is what makes a palette valid for vertices whose positions
/// were baked through an importer transform (for example RAGE Z-up -> NewEngine Y-up).
pub fn build_skin_palette(
    clip: &AnimationClip,
    skeleton: &ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    time_seconds: f32,
    sampled_locals: &mut Vec<JointLocalPose>,
    out_palette: &mut Vec<Mat4>,
) -> Result<(), String> {
    clip.sample_local_pose_for_skeleton(time_seconds, skeleton, sampled_locals)?;
    build_skin_palette_from_local_pose(skeleton, source_to_model, sampled_locals, out_palette)
}

/// Builds absolute animated joint frames in baked model space from a sampled local pose.
///
/// Unlike the skin palette, these matrices do not contain inverse-bind correction. They are
/// suitable for attachment points, animated collision proxies, sockets, cloth drivers and other
/// secondary-motion systems that must consume the current animation pose before solving.
pub fn build_model_joint_frames_from_local_pose(
    skeleton: &ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    sampled_locals: &[JointLocalPose],
    out_frames: &mut Vec<Mat4>,
) -> Result<(), String> {
    let joint_count = skeleton.joints.len();
    if sampled_locals.len() != joint_count {
        return Err(format!(
            "animation local pose count mismatch poses={} skeleton={joint_count}",
            sampled_locals.len()
        ));
    }
    for (index, joint) in skeleton.joints.iter().enumerate() {
        if joint.index as usize != index {
            return Err(format!(
                "skeleton joint indices must be dense index={} authored={}",
                index, joint.index
            ));
        }
    }
    let globals = build_globals(skeleton, sampled_locals)?;
    let source_to_model = Mat4::from_cols_array(&source_to_model);
    if !affine_invertible(source_to_model) {
        return Err("joint-frame source-to-model transform is singular/non-finite".to_owned());
    }
    out_frames.clear();
    out_frames.reserve(joint_count);
    for (index, global) in globals.into_iter().enumerate() {
        let frame = source_to_model * global;
        if frame.to_cols_array().iter().any(|value| !value.is_finite()) {
            return Err(format!(
                "animated joint frame contains non-finite value joint={index}"
            ));
        }
        out_frames.push(frame);
    }
    Ok(())
}

/// Builds a model-space skin palette from an already sampled/blended local pose.
///
/// This is the composition point used by locomotion cross-fades: animation sampling
/// and pose blending stay separate from inverse-bind palette construction, so callers
/// never have to interpolate skin matrices directly.
pub fn build_skin_palette_from_local_pose(
    skeleton: &ModelSkeletonMetadata,
    source_to_model: [f32; 16],
    sampled_locals: &[JointLocalPose],
    out_palette: &mut Vec<Mat4>,
) -> Result<(), String> {
    let joint_count = skeleton.joints.len();
    if sampled_locals.len() != joint_count {
        return Err(format!(
            "animation local pose count mismatch poses={} skeleton={joint_count}",
            sampled_locals.len()
        ));
    }
    for (index, joint) in skeleton.joints.iter().enumerate() {
        if joint.index as usize != index {
            return Err(format!(
                "skeleton joint indices must be dense index={} authored={}",
                index, joint.index
            ));
        }
    }
    let bind_locals = skeleton
        .joints
        .iter()
        .map(|joint| JointLocalPose {
            translation: joint.position_ls,
            rotation: joint.rotation_ls,
            scale: Some(joint.scale_ls),
        })
        .collect::<Vec<_>>();
    let bind_globals = build_globals(skeleton, &bind_locals)?;
    let animated_globals = build_globals(skeleton, sampled_locals)?;
    let source_to_model = Mat4::from_cols_array(&source_to_model);
    if !affine_invertible(source_to_model) {
        return Err("skin source-to-model transform is singular/non-finite".to_owned());
    }
    let model_to_source = source_to_model.inverse();
    out_palette.clear();
    out_palette.reserve(joint_count);
    for index in 0..joint_count {
        let bind = bind_globals[index];
        if !affine_invertible(bind) {
            return Err(format!("bind global matrix is singular joint={index}"));
        }
        let source_palette = animated_globals[index] * bind.inverse();
        let palette = source_to_model * source_palette * model_to_source;
        if palette
            .to_cols_array()
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(format!(
                "animated skin palette contains non-finite value joint={index}"
            ));
        }
        out_palette.push(palette);
    }
    Ok(())
}

fn build_globals(
    skeleton: &ModelSkeletonMetadata,
    locals: &[JointLocalPose],
) -> Result<Vec<Mat4>, String> {
    let joint_count = skeleton.joints.len();
    if locals.len() != joint_count {
        return Err(format!(
            "local pose count mismatch poses={} joints={joint_count}",
            locals.len()
        ));
    }
    let mut globals = vec![Mat4::IDENTITY; joint_count];
    let mut resolved = vec![false; joint_count];
    let mut remaining = joint_count;
    while remaining > 0 {
        let mut progress = false;
        for (index, joint) in skeleton.joints.iter().enumerate() {
            if resolved[index] {
                continue;
            }
            let parent = joint.parent_index.map(|value| value as usize);
            if parent.is_some_and(|parent| parent >= joint_count) {
                return Err(format!(
                    "skeleton parent index outside joint table joint={index}"
                ));
            }
            if let Some(parent) = parent {
                if !resolved[parent] {
                    continue;
                }
            }
            let local = locals[index].matrix(joint.scale_ls);
            globals[index] = parent
                .map(|parent| globals[parent] * local)
                .unwrap_or(local);
            resolved[index] = true;
            remaining -= 1;
            progress = true;
        }
        if !progress {
            return Err("skeleton hierarchy contains a cycle/unresolvable parent".to_owned());
        }
    }
    Ok(globals)
}
