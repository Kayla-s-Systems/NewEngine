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

    // Naughty Dog range records can mix additive channels with static absolute bind/base fillers
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

impl AuthoredLookPoseSpace {
    fn build(
        role: &'static str,
        base_clip: &PlayerAnimationRuntimeClip,
        range_clip: &PlayerAnimationRuntimeClip,
        skeleton: &ModelSkeletonMetadata,
        animation_runtime: &AnimationSkeletonRuntime,
        coordinate_joint: usize,
    ) -> Result<Self, String> {
        let frame_count = range_clip.clip.frame_count();
        if frame_count < 2 {
            return Err(format!(
                "authored look pose-space role={role} requires >=2 samples, got {frame_count}"
            ));
        }
        if coordinate_joint >= skeleton.joints.len() {
            return Err(format!(
                "authored look pose-space role={role} coordinate joint outside skeleton index={coordinate_joint} joints={}",
                skeleton.joints.len()
            ));
        }

        let base_pose = sample_look_base_pose(base_clip, animation_runtime)?;
        let mut base_frames = Vec::with_capacity(skeleton.joints.len());
        rebuild_model_joint_frames(animation_runtime, &base_pose, &mut base_frames)?;
        let base_coordinate_frame = *base_frames.get(coordinate_joint).ok_or_else(|| {
            format!("authored look pose-space role={role} missing base coordinate frame")
        })?;

        let candidate_joints = range_clip
            .clip
            .joint_tags
            .iter()
            .filter_map(|tag| find_skeleton_joint_for_tag(skeleton, *tag))
            .collect::<Vec<_>>();
        if candidate_joints.is_empty() {
            return Err(format!(
                "authored look pose-space role={role} range has no skeleton joints"
            ));
        }

        let mut raw_range_poses = Vec::with_capacity(frame_count);
        for frame in 0..frame_count {
            raw_range_poses.push(sample_look_range_raw_frame(
                range_clip,
                frame,
                animation_runtime,
            )?);
        }
        let channel_policies = candidate_joints
            .iter()
            .copied()
            .map(|joint| {
                (
                    joint,
                    look_channel_policy(joint, &base_pose, &raw_range_poses),
                )
            })
            .collect::<Vec<_>>();

        let mut sampled_poses = Vec::with_capacity(frame_count);
        let mut coords = Vec::with_capacity(frame_count);
        for (frame, raw_pose) in raw_range_poses.iter().enumerate() {
            let pose = compose_look_range_frame(&base_pose, raw_pose, &channel_policies);
            let mut frames = Vec::with_capacity(skeleton.joints.len());
            rebuild_model_joint_frames(animation_runtime, &pose, &mut frames)?;
            let coordinate_frame = *frames.get(coordinate_joint).ok_or_else(|| {
                format!(
                    "authored look pose-space role={role} missing sample coordinate frame={frame}"
                )
            })?;
            coords.push(look_coordinate_from_frames(
                base_coordinate_frame,
                coordinate_frame,
            ));
            sampled_poses.push(pose);
        }

        let mut joints = Vec::new();
        for joint in candidate_joints {
            let Some(base) = base_pose.get(joint) else {
                continue;
            };
            let base_q = look_quat(base);
            let base_s = look_scale(base);
            let changed = sampled_poses.iter().any(|pose| {
                let Some(sample) = pose.get(joint) else {
                    return false;
                };
                let sample_q = look_quat(sample);
                let sample_s = look_scale(sample);
                sample
                    .translation
                    .iter()
                    .zip(base.translation.iter())
                    .any(|(a, b)| (a - b).abs() > 1.0e-6)
                    || quat_component_delta(base_q, sample_q) > 1.0e-6
                    || sample_s
                        .iter()
                        .zip(base_s.iter())
                        .any(|(a, b)| (a - b).abs() > 1.0e-6)
            });
            if changed {
                joints.push(joint);
            }
        }
        if joints.is_empty() {
            return Err(format!(
                "authored look pose-space role={role} contains no pose delta against authored base"
            ));
        }

        let mut samples = Vec::with_capacity(frame_count);
        for (coord, pose) in coords.into_iter().zip(sampled_poses.iter()) {
            let mut deltas = Vec::with_capacity(joints.len());
            for &joint in &joints {
                let base = base_pose[joint];
                let sample = pose[joint];
                let base_q = look_quat(&base);
                let sample_q = look_quat(&sample);
                let delta_q = (base_q.inverse() * sample_q).normalize_or_identity();
                let base_s = look_scale(&base);
                let sample_s = look_scale(&sample);
                deltas.push(AuthoredLookJointDelta {
                    translation: [
                        sample.translation[0] - base.translation[0],
                        sample.translation[1] - base.translation[1],
                        sample.translation[2] - base.translation[2],
                    ],
                    rotation: [delta_q.x, delta_q.y, delta_q.z, delta_q.w],
                    scale_ratio: [
                        if base_s[0].abs() > 1.0e-8 {
                            sample_s[0] / base_s[0]
                        } else {
                            1.0
                        },
                        if base_s[1].abs() > 1.0e-8 {
                            sample_s[1] / base_s[1]
                        } else {
                            1.0
                        },
                        if base_s[2].abs() > 1.0e-8 {
                            sample_s[2] / base_s[2]
                        } else {
                            1.0
                        },
                    ],
                });
            }
            samples.push(AuthoredLookSample { coord, deltas });
        }

        let mut triangles = Vec::new();
        for a in 0..samples.len() {
            for b in (a + 1)..samples.len() {
                for c in (b + 1)..samples.len() {
                    let pa = samples[a].coord;
                    let pb = samples[b].coord;
                    let pc = samples[c].coord;
                    let area2 =
                        (pb[0] - pa[0]) * (pc[1] - pa[1]) - (pb[1] - pa[1]) * (pc[0] - pa[0]);
                    if area2.abs() > 1.0e-6 {
                        triangles.push([a, b, c]);
                    }
                }
            }
        }

        // Use the native sample lattice itself as the turn hysteresis scale. This avoids an engine
        // magic angle: denser authored ranges hand off to body turning sooner than coarse ranges.
        let mut nearest_spacing = Vec::new();
        for i in 0..samples.len() {
            let mut nearest = f32::INFINITY;
            for j in 0..samples.len() {
                if i == j {
                    continue;
                }
                let d = coord_distance_sq(samples[i].coord, samples[j].coord).sqrt();
                if d.is_finite() && d > 1.0e-4 {
                    nearest = nearest.min(d);
                }
            }
            if nearest.is_finite() {
                nearest_spacing.push(nearest);
            }
        }
        nearest_spacing.sort_by(|a, b| a.total_cmp(b));
        let turn_hysteresis_radians = nearest_spacing
            .get(nearest_spacing.len() / 2)
            .copied()
            .unwrap_or(0.0)
            * 0.5;

        Ok(Self {
            role,
            joints,
            samples,
            triangles,
            turn_hysteresis_radians,
        })
    }

    fn solve(&self, target: [f32; 2]) -> AuthoredLookBlend {
        let target = [
            if target[0].is_finite() {
                target[0]
            } else {
                0.0
            },
            if target[1].is_finite() {
                target[1]
            } else {
                0.0
            },
        ];
        if self.samples.len() == 1 {
            return AuthoredLookBlend::single(0, self.samples[0].coord);
        }

        let mut best_triangle: Option<(f32, [usize; 3], [f32; 3])> = None;
        for triangle in &self.triangles {
            let a = self.samples[triangle[0]].coord;
            let b = self.samples[triangle[1]].coord;
            let c = self.samples[triangle[2]].coord;
            let Some(weights) = barycentric_2d(target, a, b, c) else {
                continue;
            };
            if weights
                .iter()
                .any(|weight| *weight < -1.0e-4 || *weight > 1.0001)
            {
                continue;
            }
            // Prefer the most local containing triangle when the source lattice has overlapping
            // triangles; this reproduces piecewise authored interpolation without a hard-coded grid.
            let score = coord_distance_sq(target, a)
                + coord_distance_sq(target, b)
                + coord_distance_sq(target, c);
            if best_triangle.as_ref().is_none_or(|best| score < best.0) {
                best_triangle = Some((score, *triangle, weights));
            }
        }
        if let Some((_, indices, mut weights)) = best_triangle {
            for weight in &mut weights {
                *weight = weight.clamp(0.0, 1.0);
            }
            let sum = weights.iter().sum::<f32>().max(1.0e-8);
            for weight in &mut weights {
                *weight /= sum;
            }
            return AuthoredLookBlend {
                indices,
                weights,
                count: 3,
                projected: target,
            };
        }

        let mut best_edge: Option<(f32, usize, usize, f32, [f32; 2])> = None;
        for a in 0..self.samples.len() {
            for b in (a + 1)..self.samples.len() {
                let (projected, t) =
                    project_to_segment(target, self.samples[a].coord, self.samples[b].coord);
                let distance = coord_distance_sq(target, projected);
                if best_edge.as_ref().is_none_or(|best| distance < best.0) {
                    best_edge = Some((distance, a, b, t, projected));
                }
            }
        }
        if let Some((_, a, b, t, projected)) = best_edge {
            return AuthoredLookBlend {
                indices: [a, b, a],
                weights: [1.0 - t, t, 0.0],
                count: 2,
                projected,
            };
        }

        let (index, sample) = self
            .samples
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                coord_distance_sq(target, a.coord).total_cmp(&coord_distance_sq(target, b.coord))
            })
            .expect("authored look pose-space must contain samples");
        AuthoredLookBlend::single(index, sample.coord)
    }

    fn apply_blend(&self, blend: AuthoredLookBlend, pose: &mut [JointLocalPose]) {
        for (delta_index, &joint) in self.joints.iter().enumerate() {
            let Some(local) = pose.get_mut(joint) else {
                continue;
            };
            let mut translation = Vec3::ZERO;
            let mut scale_ratio = Vec3::ZERO;
            let mut qsum = [0.0_f32; 4];
            let mut reference = None::<Quat>;
            for lane in 0..blend.count {
                let weight = blend.weights[lane];
                if weight <= 0.0 {
                    continue;
                }
                let delta = self.samples[blend.indices[lane]].deltas[delta_index];
                translation += Vec3::new(
                    delta.translation[0],
                    delta.translation[1],
                    delta.translation[2],
                ) * weight;
                scale_ratio += Vec3::new(
                    delta.scale_ratio[0],
                    delta.scale_ratio[1],
                    delta.scale_ratio[2],
                ) * weight;
                let mut q = Quat::from_xyzw(
                    delta.rotation[0],
                    delta.rotation[1],
                    delta.rotation[2],
                    delta.rotation[3],
                )
                .normalize_or_identity();
                if let Some(reference) = reference {
                    if reference.dot(q) < 0.0 {
                        q = Quat::from_xyzw(-q.x, -q.y, -q.z, -q.w);
                    }
                } else {
                    reference = Some(q);
                }
                qsum[0] += q.x * weight;
                qsum[1] += q.y * weight;
                qsum[2] += q.z * weight;
                qsum[3] += q.w * weight;
            }
            local.translation[0] += translation.x;
            local.translation[1] += translation.y;
            local.translation[2] += translation.z;
            let current = look_quat(local);
            let delta_rotation =
                Quat::from_xyzw(qsum[0], qsum[1], qsum[2], qsum[3]).normalize_or_identity();
            let rotation = (current * delta_rotation).normalize_or_identity();
            local.rotation = [rotation.x, rotation.y, rotation.z, rotation.w];
            let scale = look_scale(local);
            local.scale = Some([
                scale[0] * scale_ratio.x,
                scale[1] * scale_ratio.y,
                scale[2] * scale_ratio.z,
            ]);
        }
    }
}

impl AuthoredLookRuntimeBinding {
    fn body_space(&self, state: AuthoredLookState) -> Option<&AuthoredLookPoseSpace> {
        match state {
            AuthoredLookState::Relaxed => self.relaxed.as_ref(),
            AuthoredLookState::Crouch => self.crouch.as_ref().or(self.relaxed.as_ref()),
            AuthoredLookState::Tense => self.tense.as_ref().or(self.relaxed.as_ref()),
            AuthoredLookState::CoverLowLeft => self.cover_low_left.as_ref(),
            AuthoredLookState::CoverLowRight => self.cover_low_right.as_ref(),
            AuthoredLookState::Prone => self.prone.as_ref(),
            AuthoredLookState::Supine => self.supine.as_ref(),
            AuthoredLookState::Rope => self.rope.as_ref(),
            AuthoredLookState::Ladder => self.ladder.as_ref(),
            AuthoredLookState::SwimIdle => self.swim_idle.as_ref(),
            AuthoredLookState::Injured => self.injured.as_ref(),
            AuthoredLookState::RelaxedInjured => self.relaxed_injured.as_ref(),
        }
    }

    fn projection(
        &self,
        state: AuthoredLookState,
        yaw: f32,
        pitch: f32,
    ) -> Option<AuthoredLookProjection> {
        let body = self.body_space(state)?;
        let target = [yaw, pitch];
        let body_blend = body.solve(target);
        let after_body = [
            target[0] - body_blend.projected[0],
            target[1] - body_blend.projected[1],
        ];
        let (eye_projected, residual) = if let Some(eyes) = self.eyes.as_ref() {
            let eye_blend = eyes.solve(after_body);
            (
                eye_blend.projected,
                [
                    after_body[0] - eye_blend.projected[0],
                    after_body[1] - eye_blend.projected[1],
                ],
            )
        } else {
            ([0.0, 0.0], after_body)
        };
        Some(AuthoredLookProjection {
            body_projected: body_blend.projected,
            eye_projected,
            residual,
            turn_hysteresis_radians: body.turn_hysteresis_radians,
        })
    }

    fn apply(
        &self,
        state: AuthoredLookState,
        yaw: f32,
        pitch: f32,
        pose: &mut [JointLocalPose],
    ) -> Option<AuthoredLookProjection> {
        let body = self.body_space(state)?;
        let target = [yaw, pitch];
        let body_blend = body.solve(target);
        body.apply_blend(body_blend, pose);
        let after_body = [
            target[0] - body_blend.projected[0],
            target[1] - body_blend.projected[1],
        ];
        let (eye_projected, residual) = if let Some(eyes) = self.eyes.as_ref() {
            let eye_blend = eyes.solve(after_body);
            eyes.apply_blend(eye_blend, pose);
            (
                eye_blend.projected,
                [
                    after_body[0] - eye_blend.projected[0],
                    after_body[1] - eye_blend.projected[1],
                ],
            )
        } else {
            ([0.0, 0.0], after_body)
        };
        Some(AuthoredLookProjection {
            body_projected: body_blend.projected,
            eye_projected,
            residual,
            turn_hysteresis_radians: body.turn_hysteresis_radians,
        })
    }
}

fn look_coordinate_joint(
    skeleton: &ModelSkeletonMetadata,
    eye_only: bool,
) -> Result<usize, String> {
    if eye_only {
        skeleton
            .joints
            .iter()
            .position(|joint| {
                let name = joint.name.to_ascii_lowercase();
                name == "l_eyeball" || name == "left_eyeball" || name.contains("eye_l")
            })
            .ok_or_else(|| "authored eye look requires a left-eye skeleton joint".to_owned())
    } else {
        skeleton
            .joints
            .iter()
            .position(|joint| joint.name == skeleton.anchors.head)
            .ok_or_else(|| {
                format!(
                    "authored head look anchor '{}' is absent from skeleton",
                    skeleton.anchors.head
                )
            })
    }
}

fn build_authored_look_pose_space(
    role: &'static str,
    base_clip: Option<PlayerAnimationRuntimeClip>,
    range_clip: Option<PlayerAnimationRuntimeClip>,
    skeleton: &ModelSkeletonMetadata,
    animation_runtime: &AnimationSkeletonRuntime,
    eye_only: bool,
) -> Result<Option<AuthoredLookPoseSpace>, String> {
    match (base_clip, range_clip) {
        (None, None) => Ok(None),
        (Some(base), Some(range)) => {
            let coordinate_joint = look_coordinate_joint(skeleton, eye_only)?;
            AuthoredLookPoseSpace::build(
                role,
                &base,
                &range,
                skeleton,
                animation_runtime,
                coordinate_joint,
            )
            .map(Some)
        }
        (base, range) => Err(format!(
            "authored look pose-space role={role} requires base+range pair base={} range={}",
            base.as_ref()
                .map(|clip| clip.clip_ref.as_str())
                .unwrap_or("none"),
            range
                .as_ref()
                .map(|clip| clip.clip_ref.as_str())
                .unwrap_or("none")
        )),
    }
}
