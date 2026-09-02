#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuthoredLookState {
    Relaxed,
    Crouch,
    Tense,
    CoverLowLeft,
    CoverLowRight,
    Prone,
    Supine,
    Rope,
    Ladder,
    SwimIdle,
    Injured,
    RelaxedInjured,
}

impl AuthoredLookState {
    #[inline]
    const fn contextual(self) -> bool {
        !matches!(self, Self::Relaxed | Self::Crouch | Self::Tense)
    }
}

#[derive(Clone, Copy, Debug)]
struct AuthoredLookJointDelta {
    translation: [f32; 3],
    rotation: [f32; 4],
    scale_ratio: [f32; 3],
}

impl Default for AuthoredLookJointDelta {
    fn default() -> Self {
        Self {
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale_ratio: [1.0; 3],
        }
    }
}

#[derive(Clone, Debug)]
struct AuthoredLookSample {
    coord: [f32; 2],
    deltas: Vec<AuthoredLookJointDelta>,
}

#[derive(Clone, Copy, Debug)]
struct AuthoredLookBlend {
    indices: [usize; 3],
    weights: [f32; 3],
    count: usize,
    projected: [f32; 2],
}

impl AuthoredLookBlend {
    fn single(index: usize, coord: [f32; 2]) -> Self {
        Self {
            indices: [index, index, index],
            weights: [1.0, 0.0, 0.0],
            count: 1,
            projected: coord,
        }
    }
}

#[derive(Clone, Debug)]
struct AuthoredLookPoseSpace {
    role: &'static str,
    joints: Vec<usize>,
    samples: Vec<AuthoredLookSample>,
    triangles: Vec<[usize; 3]>,
    turn_hysteresis_radians: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct AuthoredLookProjection {
    body_projected: [f32; 2],
    eye_projected: [f32; 2],
    residual: [f32; 2],
    turn_hysteresis_radians: f32,
}

#[derive(Clone, Debug, Default)]
struct AuthoredLookRuntimeBinding {
    relaxed: Option<AuthoredLookPoseSpace>,
    crouch: Option<AuthoredLookPoseSpace>,
    tense: Option<AuthoredLookPoseSpace>,
    cover_low_left: Option<AuthoredLookPoseSpace>,
    cover_low_right: Option<AuthoredLookPoseSpace>,
    prone: Option<AuthoredLookPoseSpace>,
    supine: Option<AuthoredLookPoseSpace>,
    rope: Option<AuthoredLookPoseSpace>,
    ladder: Option<AuthoredLookPoseSpace>,
    swim_idle: Option<AuthoredLookPoseSpace>,
    injured: Option<AuthoredLookPoseSpace>,
    relaxed_injured: Option<AuthoredLookPoseSpace>,
    eyes: Option<AuthoredLookPoseSpace>,
}

#[inline]
fn look_quat(pose: &JointLocalPose) -> Quat {
    Quat::from_xyzw(
        pose.rotation[0],
        pose.rotation[1],
        pose.rotation[2],
        pose.rotation[3],
    )
    .normalize_or_identity()
}

#[inline]
fn look_scale(pose: &JointLocalPose) -> [f32; 3] {
    pose.scale.unwrap_or([1.0, 1.0, 1.0])
}

#[inline]
fn quat_component_delta(a: Quat, b: Quat) -> f32 {
    let direct = (a.x - b.x)
        .abs()
        .max((a.y - b.y).abs())
        .max((a.z - b.z).abs())
        .max((a.w - b.w).abs());
    let antipodal = (a.x + b.x)
        .abs()
        .max((a.y + b.y).abs())
        .max((a.z + b.z).abs())
        .max((a.w + b.w).abs());
    direct.min(antipodal)
}

fn look_coordinate_from_frames(base: Mat4, sample: Mat4) -> [f32; 2] {
    let base_rotation = base
        .to_scale_rotation_translation()
        .1
        .normalize_or_identity();
    let sample_rotation = sample
        .to_scale_rotation_translation()
        .1
        .normalize_or_identity();
    let delta = (sample_rotation * base_rotation.inverse()).normalize_or_identity();
    let forward = (delta * -Vec3::Z).normalize_or_zero();
    if !forward.is_finite() || forward.length_squared() <= 1.0e-8 {
        return [0.0, 0.0];
    }
    let yaw = (-forward.x).atan2(-forward.z);
    let pitch = forward
        .y
        .atan2((forward.x * forward.x + forward.z * forward.z).sqrt());
    [yaw, pitch]
}

fn find_skeleton_joint_for_tag(skeleton: &ModelSkeletonMetadata, tag: u32) -> Option<usize> {
    let dense = tag as usize;
    if dense < skeleton.joints.len() && skeleton.joints[dense].tag == tag {
        Some(dense)
    } else {
        skeleton.joints.iter().position(|joint| joint.tag == tag)
    }
}

fn sample_look_base_pose(
    clip: &PlayerAnimationRuntimeClip,
    animation_runtime: &AnimationSkeletonRuntime,
) -> Result<Vec<JointLocalPose>, String> {
    let mut pose = animation_runtime.bind_locals().to_vec();
    clip.clip
        .sample_local_pose_bound(0.0, animation_runtime, &clip.binding, &mut pose)?;
    Ok(pose)
}

#[derive(Clone, Copy, Debug, Default)]
struct AuthoredLookChannelPolicy {
    translation_additive: bool,
    rotation_additive: bool,
    scale_multiplicative: bool,
}

fn sample_look_range_raw_frame(
    clip: &PlayerAnimationRuntimeClip,
    frame: usize,
    animation_runtime: &AnimationSkeletonRuntime,
) -> Result<Vec<JointLocalPose>, String> {
    let mut raw_pose = animation_runtime.bind_locals().to_vec();
    let sample_rate = clip.clip.sample_rate_hz.max(1.0e-6);
    let time = frame as f32 / sample_rate;
    clip.clip
        .sample_local_pose_bound(time, animation_runtime, &clip.binding, &mut raw_pose)?;
    Ok(raw_pose)
}

fn look_channel_policy(
    joint: usize,
    base_pose: &[JointLocalPose],
    raw_frames: &[Vec<JointLocalPose>],
) -> AuthoredLookChannelPolicy {
    let Some(base) = base_pose.get(joint) else {
        return AuthoredLookChannelPolicy::default();
    };
    let Some(first) = raw_frames.first().and_then(|pose| pose.get(joint)) else {
        return AuthoredLookChannelPolicy::default();
    };

    // authored range records can mix additive channels with static absolute bind/base fillers
    // in the same joint record. Treating those fillers as additive doubles local bone offsets (for
    // example the generic male eyes range repeats neck/heada/headb translations) and produces the
    // characteristic rubber/stretch deformation. Classify only channels that stay static and match
    // the companion authored base as absolute fillers; varying channels and near-zero translation /
    // identity rotation channels remain genuine additive data.
    let translation_variation = raw_frames
        .iter()
        .filter_map(|pose| pose.get(joint))
        .map(|sample| {
            Vec3::new(
                sample.translation[0] - first.translation[0],
                sample.translation[1] - first.translation[1],
                sample.translation[2] - first.translation[2],
            )
            .length()
        })
        .fold(0.0_f32, f32::max);
    let translation_to_base = Vec3::new(
        first.translation[0] - base.translation[0],
        first.translation[1] - base.translation[1],
        first.translation[2] - base.translation[2],
    )
    .length();
    let translation_absolute_filler =
        translation_variation <= 1.0e-5 && translation_to_base <= 5.0e-4;

    let first_rotation = look_quat(first);
    let base_rotation = look_quat(base);
    let rotation_variation = raw_frames
        .iter()
        .filter_map(|pose| pose.get(joint))
        .map(|sample| quat_component_delta(first_rotation, look_quat(sample)))
        .fold(0.0_f32, f32::max);
    let rotation_to_base = quat_component_delta(first_rotation, base_rotation);
    let rotation_absolute_filler = rotation_variation <= 1.0e-5 && rotation_to_base <= 5.0e-4;

    let first_scale = look_scale(first);
    let base_scale = look_scale(base);
    let scale_variation = raw_frames
        .iter()
        .filter_map(|pose| pose.get(joint))
        .map(|sample| {
            let value = look_scale(sample);
            (value[0] - first_scale[0])
                .abs()
                .max((value[1] - first_scale[1]).abs())
                .max((value[2] - first_scale[2]).abs())
        })
        .fold(0.0_f32, f32::max);
    let scale_to_base = (first_scale[0] - base_scale[0])
        .abs()
        .max((first_scale[1] - base_scale[1]).abs())
        .max((first_scale[2] - base_scale[2]).abs());
    let scale_absolute_filler = scale_variation <= 1.0e-6 && scale_to_base <= 1.0e-5;

    AuthoredLookChannelPolicy {
        translation_additive: !translation_absolute_filler,
        rotation_additive: !rotation_absolute_filler,
        scale_multiplicative: !scale_absolute_filler,
    }
}

fn compose_look_range_frame(
    base_pose: &[JointLocalPose],
    raw_pose: &[JointLocalPose],
    joints: &[(usize, AuthoredLookChannelPolicy)],
) -> Vec<JointLocalPose> {
    let mut pose = base_pose.to_vec();
    for &(joint, policy) in joints {
        let (Some(base), Some(raw), Some(dst)) = (
            base_pose.get(joint),
            raw_pose.get(joint),
            pose.get_mut(joint),
        ) else {
            continue;
        };
        if policy.translation_additive {
            dst.translation = [
                base.translation[0] + raw.translation[0],
                base.translation[1] + raw.translation[1],
                base.translation[2] + raw.translation[2],
            ];
        }
        if policy.rotation_additive {
            let rotation = (look_quat(base) * look_quat(raw)).normalize_or_identity();
            dst.rotation = [rotation.x, rotation.y, rotation.z, rotation.w];
        }
        if policy.scale_multiplicative {
            let base_scale = look_scale(base);
            let delta_scale = look_scale(raw);
            dst.scale = Some([
                base_scale[0] * delta_scale[0],
                base_scale[1] * delta_scale[1],
                base_scale[2] * delta_scale[2],
            ]);
        }
    }
    pose
}

fn barycentric_2d(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> Option<[f32; 3]> {
    let v0 = [b[0] - a[0], b[1] - a[1]];
    let v1 = [c[0] - a[0], c[1] - a[1]];
    let v2 = [p[0] - a[0], p[1] - a[1]];
    let denom = v0[0] * v1[1] - v1[0] * v0[1];
    if !denom.is_finite() || denom.abs() <= 1.0e-7 {
        return None;
    }
    let inv = 1.0 / denom;
    let v = (v2[0] * v1[1] - v1[0] * v2[1]) * inv;
    let w = (v0[0] * v2[1] - v2[0] * v0[1]) * inv;
    let u = 1.0 - v - w;
    Some([u, v, w])
}

#[inline]
fn coord_distance_sq(a: [f32; 2], b: [f32; 2]) -> f32 {
    let x = a[0] - b[0];
    let y = a[1] - b[1];
    x * x + y * y
}

fn project_to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> ([f32; 2], f32) {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    if !len2.is_finite() || len2 <= 1.0e-10 {
        return (a, 0.0);
    }
    let ap = [p[0] - a[0], p[1] - a[1]];
    let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / len2).clamp(0.0, 1.0);
    ([a[0] + ab[0] * t, a[1] + ab[1] * t], t)
}
