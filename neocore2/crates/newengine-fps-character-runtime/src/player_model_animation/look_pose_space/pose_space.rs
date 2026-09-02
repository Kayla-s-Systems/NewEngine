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
