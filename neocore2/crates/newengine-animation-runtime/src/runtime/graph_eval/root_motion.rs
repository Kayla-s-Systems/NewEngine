fn sample_bound_joint_wrapped(
    clip: &AnimationClip,
    binding: &AnimationClipBinding,
    skeleton: &AnimationSkeletonRuntime,
    skeleton_joint: usize,
    time_seconds: f32,
) -> Result<JointLocalPose, String> {
    if skeleton_joint >= skeleton.joint_count() {
        return Err(format!(
            "animation root-motion joint index outside skeleton joint={skeleton_joint}"
        ));
    }
    let Some(clip_joint) = binding
        .clip_joint_to_skeleton
        .iter()
        .position(|joint| *joint == skeleton_joint)
    else {
        return Ok(skeleton.bind_locals()[skeleton_joint]);
    };
    let frame_count = clip.frame_count();
    if frame_count == 0 {
        return Err(format!("animation clip '{}' contains no frames", clip.name));
    }
    let duration = clip.duration_seconds.max(1.0e-6);
    let mut t = time_seconds.max(0.0);
    if clip.looped {
        t = t.rem_euclid(duration);
    } else {
        t = t.min(duration);
    }
    let mut frame_position = t * clip.sample_rate_hz.max(1.0e-6);
    if clip.looped {
        frame_position = frame_position.rem_euclid(frame_count as f32);
    } else {
        frame_position = frame_position.min((frame_count - 1) as f32);
    }
    let base = frame_position.floor() as usize;
    let alpha = frame_position - base as f32;
    let frame0 = base.min(frame_count - 1);
    let frame1 = if clip.looped {
        (frame0 + 1) % frame_count
    } else {
        (frame0 + 1).min(frame_count - 1)
    };
    let joint_count = clip.joint_count();
    let a = clip.poses[frame0 * joint_count + clip_joint];
    let b = clip.poses[frame1 * joint_count + clip_joint];
    Ok(blend_local_pose(a, b, alpha))
}

fn clip_cycle_end_time(clip: &AnimationClip) -> f32 {
    if clip.frame_count() <= 1 {
        return 0.0;
    }
    (((clip.frame_count() - 1) as f32) / clip.sample_rate_hz.max(1.0e-6))
        .min((clip.duration_seconds - 1.0e-6).max(0.0))
}

fn quat_pow(mut value: Quat, mut exponent: u64) -> Quat {
    let mut result = Quat::IDENTITY;
    value = value.normalize_or_identity();
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = (result * value).normalize_or_identity();
        }
        exponent >>= 1;
        if exponent != 0 {
            value = (value * value).normalize_or_identity();
        }
    }
    result
}

fn sample_bound_joint_unwrapped(
    clip: &AnimationClip,
    binding: &AnimationClipBinding,
    skeleton: &AnimationSkeletonRuntime,
    skeleton_joint: usize,
    playback_time_seconds: f32,
) -> Result<JointLocalPose, String> {
    if !clip.looped {
        return sample_bound_joint_wrapped(
            clip,
            binding,
            skeleton,
            skeleton_joint,
            playback_time_seconds,
        );
    }
    let duration = clip.duration_seconds.max(1.0e-6);
    let loop_index = (playback_time_seconds / duration).floor().max(0.0) as u64;
    let local_time = playback_time_seconds.rem_euclid(duration);
    let mut local =
        sample_bound_joint_wrapped(clip, binding, skeleton, skeleton_joint, local_time)?;
    if loop_index == 0 {
        return Ok(local);
    }
    let start = sample_bound_joint_wrapped(clip, binding, skeleton, skeleton_joint, 0.0)?;
    let end = sample_bound_joint_wrapped(
        clip,
        binding,
        skeleton,
        skeleton_joint,
        clip_cycle_end_time(clip),
    )?;
    let cycle_translation = vec3(end.translation) - vec3(start.translation);
    local.translation = vec3_array(vec3(local.translation) + cycle_translation * loop_index as f32);
    let start_rotation = quat(start.rotation).normalize_or_identity();
    let end_rotation = quat(end.rotation).normalize_or_identity();
    let cycle_rotation = (end_rotation * start_rotation.inverse()).normalize_or_identity();
    local.rotation = quat_array(
        (quat_pow(cycle_rotation, loop_index) * quat(local.rotation).normalize_or_identity())
            .normalize_or_identity(),
    );
    Ok(local)
}

fn root_motion_delta_between(
    clip: &AnimationClip,
    binding: &AnimationClipBinding,
    skeleton: &AnimationSkeletonRuntime,
    root_joint: usize,
    previous_time_seconds: f32,
    current_time_seconds: f32,
) -> Result<AnimationRootMotionDelta, String> {
    let previous =
        sample_bound_joint_unwrapped(clip, binding, skeleton, root_joint, previous_time_seconds)?;
    let current =
        sample_bound_joint_unwrapped(clip, binding, skeleton, root_joint, current_time_seconds)?;
    let translation = vec3(current.translation) - vec3(previous.translation);
    let rotation = (quat(current.rotation).normalize_or_identity()
        * quat(previous.rotation).normalize_or_identity().inverse())
    .normalize_or_identity();
    Ok(AnimationRootMotionDelta {
        translation: vec3_array(translation),
        rotation: quat_array(rotation),
        valid: true,
    })
}
