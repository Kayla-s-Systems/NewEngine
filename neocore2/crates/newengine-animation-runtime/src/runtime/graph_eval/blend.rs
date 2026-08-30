fn blend_local_pose(a: JointLocalPose, b: JointLocalPose, alpha: f32) -> JointLocalPose {
    let alpha = alpha.clamp(0.0, 1.0);
    let translation = vec3(a.translation).lerp(vec3(b.translation), alpha);
    let qa = quat(a.rotation).normalize_or_identity();
    let qb = quat(b.rotation).normalize_or_identity();
    let rotation = qa.slerp(qb, alpha).normalize_or_identity();
    let scale = match (a.scale, b.scale) {
        (Some(a), Some(b)) => Some(vec3_array(vec3(a).lerp(vec3(b), alpha))),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };
    JointLocalPose {
        translation: vec3_array(translation),
        rotation: quat_array(rotation),
        scale,
    }
}

fn blend_pose_arrays(
    a: &[JointLocalPose],
    b: &[JointLocalPose],
    alpha: f32,
    out: &mut Vec<JointLocalPose>,
) -> Result<(), String> {
    if a.len() != b.len() {
        return Err(format!(
            "animation graph pose blend shape mismatch a={} b={}",
            a.len(),
            b.len()
        ));
    }
    out.clear();
    out.reserve(a.len());
    out.extend(
        a.iter()
            .copied()
            .zip(b.iter().copied())
            .map(|(a, b)| blend_local_pose(a, b, alpha)),
    );
    Ok(())
}

fn accumulate_weighted_pose_sample(
    out: &mut Vec<JointLocalPose>,
    sample: &[JointLocalPose],
    reference: &[JointLocalPose],
    bind: &[JointLocalPose],
    weight: f32,
    initialize: bool,
) -> Result<(), String> {
    if sample.len() != reference.len() || sample.len() != bind.len() {
        return Err(format!(
            "animation graph weighted pose shape mismatch sample={} reference={} bind={}",
            sample.len(),
            reference.len(),
            bind.len()
        ));
    }
    if initialize {
        out.clear();
        out.extend_from_slice(bind);
    } else if out.len() != sample.len() {
        return Err(format!(
            "animation graph weighted pose accumulator shape mismatch out={} sample={}",
            out.len(),
            sample.len()
        ));
    }
    for index in 0..sample.len() {
        let sample_pose = sample[index];
        let reference_rotation = quat(reference[index].rotation).normalize_or_identity();
        let mut sample_rotation = quat(sample_pose.rotation).normalize_or_identity();
        if reference_rotation.dot(sample_rotation) < 0.0 {
            sample_rotation = Quat::from_xyzw(
                -sample_rotation.x,
                -sample_rotation.y,
                -sample_rotation.z,
                -sample_rotation.w,
            );
        }
        let rotation = quat_array(sample_rotation);
        let sample_scale = sample_pose.scale.or(bind[index].scale).unwrap_or([1.0; 3]);
        if initialize {
            out[index].translation = [
                sample_pose.translation[0] * weight,
                sample_pose.translation[1] * weight,
                sample_pose.translation[2] * weight,
            ];
            out[index].rotation = [
                rotation[0] * weight,
                rotation[1] * weight,
                rotation[2] * weight,
                rotation[3] * weight,
            ];
            out[index].scale = Some([
                sample_scale[0] * weight,
                sample_scale[1] * weight,
                sample_scale[2] * weight,
            ]);
        } else {
            for (component, value) in out[index]
                .translation
                .iter_mut()
                .zip(sample_pose.translation)
            {
                *component += value * weight;
            }
            for (component, value) in out[index].rotation.iter_mut().zip(rotation) {
                *component += value * weight;
            }
            let accumulated_scale = out[index].scale.get_or_insert([0.0; 3]);
            for (component, value) in accumulated_scale.iter_mut().zip(sample_scale) {
                *component += value * weight;
            }
        }
    }
    Ok(())
}

fn finish_weighted_pose(out: &mut [JointLocalPose], total_weight: f32) -> Result<(), String> {
    if !total_weight.is_finite() || total_weight <= 1.0e-8 {
        return Err(format!(
            "animation graph weighted pose has invalid total weight {total_weight}"
        ));
    }
    let inverse = total_weight.recip();
    for pose in out {
        for component in &mut pose.translation {
            *component *= inverse;
        }
        pose.rotation = quat_array(
            Quat::from_xyzw(
                pose.rotation[0],
                pose.rotation[1],
                pose.rotation[2],
                pose.rotation[3],
            )
            .normalize_or_identity(),
        );
        if let Some(scale) = &mut pose.scale {
            for component in scale {
                *component *= inverse;
            }
        }
    }
    Ok(())
}

fn apply_override_layer(
    base: &mut [JointLocalPose],
    layer: &[JointLocalPose],
    mask: &[f32],
    weight: f32,
) -> Result<(), String> {
    if base.len() != layer.len() || base.len() != mask.len() {
        return Err("animation graph override layer pose/mask shape mismatch".to_owned());
    }
    for index in 0..base.len() {
        let joint_weight = (weight * mask[index]).clamp(0.0, 1.0);
        if joint_weight > 0.0 {
            base[index] = blend_local_pose(base[index], layer[index], joint_weight);
        }
    }
    Ok(())
}

fn apply_additive_layer(
    base: &mut [JointLocalPose],
    layer: &[JointLocalPose],
    bind: &[JointLocalPose],
    mask: &[f32],
    weight: f32,
) -> Result<(), String> {
    if base.len() != layer.len() || base.len() != bind.len() || base.len() != mask.len() {
        return Err("animation graph additive layer pose/mask shape mismatch".to_owned());
    }
    for index in 0..base.len() {
        let joint_weight = (weight * mask[index]).clamp(0.0, 1.0);
        if joint_weight <= 0.0 {
            continue;
        }
        let base_translation = vec3(base[index].translation);
        let layer_translation = vec3(layer[index].translation);
        let bind_translation = vec3(bind[index].translation);
        base[index].translation =
            vec3_array(base_translation + (layer_translation - bind_translation) * joint_weight);

        let base_rotation = quat(base[index].rotation).normalize_or_identity();
        let layer_rotation = quat(layer[index].rotation).normalize_or_identity();
        let bind_rotation = quat(bind[index].rotation).normalize_or_identity();
        let delta_rotation = (layer_rotation * bind_rotation.inverse()).normalize_or_identity();
        let weighted_delta = Quat::IDENTITY
            .slerp(delta_rotation, joint_weight)
            .normalize_or_identity();
        base[index].rotation = quat_array((base_rotation * weighted_delta).normalize_or_identity());

        let base_scale = vec3(base[index].scale.unwrap_or([1.0; 3]));
        let layer_scale = vec3(layer[index].scale.unwrap_or([1.0; 3]));
        let bind_scale = vec3(bind[index].scale.unwrap_or([1.0; 3]));
        base[index].scale = Some(vec3_array(
            base_scale + (layer_scale - bind_scale) * joint_weight,
        ));
    }
    Ok(())
}

fn blend_tree_segment(samples: &[CompiledBlendSample1D], value: f32) -> (usize, usize, f32) {
    if samples.len() == 1 || value <= samples[0].threshold {
        return (0, 0, 0.0);
    }
    let last = samples.len() - 1;
    if value >= samples[last].threshold {
        return (last, last, 0.0);
    }
    for index in 0..last {
        let a = samples[index].threshold;
        let b = samples[index + 1].threshold;
        if value >= a && value <= b {
            let alpha = if (b - a).abs() <= 1.0e-8 {
                0.0
            } else {
                ((value - a) / (b - a)).clamp(0.0, 1.0)
            };
            return (index, index + 1, alpha);
        }
    }
    (last, last, 0.0)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct WeightedBlendSample {
    sample_index: usize,
    weight: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct WeightedBlendSet {
    entries: [WeightedBlendSample; 4],
    len: usize,
}

impl WeightedBlendSet {
    fn add(&mut self, sample_index: usize, weight: f32) {
        if weight <= 1.0e-7 {
            return;
        }
        for entry in &mut self.entries[..self.len] {
            if entry.sample_index == sample_index {
                entry.weight += weight;
                return;
            }
        }
        debug_assert!(self.len < self.entries.len());
        if self.len < self.entries.len() {
            self.entries[self.len] = WeightedBlendSample {
                sample_index,
                weight,
            };
            self.len += 1;
        }
    }

    fn finish(mut self, fallback_sample: usize) -> Self {
        let total = self.entries[..self.len]
            .iter()
            .map(|entry| entry.weight.max(0.0))
            .sum::<f32>();
        if total <= 1.0e-8 {
            self.len = 1;
            self.entries[0] = WeightedBlendSample {
                sample_index: fallback_sample,
                weight: 1.0,
            };
            return self;
        }
        for entry in &mut self.entries[..self.len] {
            entry.weight = entry.weight.max(0.0) / total;
        }
        self.entries[..self.len].sort_by_key(|entry| entry.sample_index);
        self
    }

    fn contains(&self, sample_index: usize) -> bool {
        self.entries[..self.len]
            .iter()
            .any(|entry| entry.sample_index == sample_index)
    }

    fn dominant(&self) -> WeightedBlendSample {
        self.entries[..self.len]
            .iter()
            .copied()
            .max_by(|left, right| {
                left.weight
                    .total_cmp(&right.weight)
                    .then_with(|| right.sample_index.cmp(&left.sample_index))
            })
            .unwrap_or_default()
    }
}

fn blend_axis_segment(values: &[f32], value: f32) -> (usize, usize, f32) {
    if value <= values[0] {
        return (0, 0, 0.0);
    }
    let last = values.len() - 1;
    if value >= values[last] {
        return (last, last, 0.0);
    }
    for index in 0..last {
        let a = values[index];
        let b = values[index + 1];
        if value >= a && value <= b {
            return (index, index + 1, ((value - a) / (b - a)).clamp(0.0, 1.0));
        }
    }
    (last, last, 0.0)
}

fn cartesian_blend2d_weights(
    x_values: &[f32],
    y_values: &[f32],
    grid: &[usize],
    x: f32,
    y: f32,
) -> WeightedBlendSet {
    let (x0, x1, tx) = blend_axis_segment(x_values, x);
    let (y0, y1, ty) = blend_axis_segment(y_values, y);
    let width = x_values.len();
    let mut weights = WeightedBlendSet::default();
    weights.add(grid[y0 * width + x0], (1.0 - tx) * (1.0 - ty));
    weights.add(grid[y0 * width + x1], tx * (1.0 - ty));
    weights.add(grid[y1 * width + x0], (1.0 - tx) * ty);
    weights.add(grid[y1 * width + x1], tx * ty);
    weights.finish(grid[y0 * width + x0])
}

fn directional_blend2d_weights(
    center_sample: Option<usize>,
    ring: &[CompiledDirectionalSample],
    x: f32,
    y: f32,
) -> WeightedBlendSet {
    let radius = x.hypot(y);
    if radius <= 1.0e-6 {
        let fallback = center_sample.unwrap_or(ring[0].sample_index);
        let mut weights = WeightedBlendSet::default();
        weights.add(fallback, 1.0);
        return weights.finish(fallback);
    }
    let angle = normalize_angle_radians(y.atan2(x));
    let mut left = ring.len() - 1;
    let mut right = 0usize;
    let mut query = angle;
    for index in 0..ring.len() {
        let next = (index + 1) % ring.len();
        let start = ring[index].angle_radians;
        let end = if next == 0 {
            ring[next].angle_radians + std::f32::consts::TAU
        } else {
            ring[next].angle_radians
        };
        let candidate = if next == 0 && query < start {
            query + std::f32::consts::TAU
        } else {
            query
        };
        if candidate >= start && candidate <= end {
            left = index;
            right = next;
            query = candidate;
            break;
        }
    }
    let start = ring[left].angle_radians;
    let end = if right == 0 {
        ring[right].angle_radians + std::f32::consts::TAU
    } else {
        ring[right].angle_radians
    };
    let angular = ((query - start) / (end - start).max(1.0e-6)).clamp(0.0, 1.0);
    let boundary_radius =
        (ring[left].radius * (1.0 - angular) + ring[right].radius * angular).max(1.0e-6);
    let radial = if center_sample.is_some() {
        (radius / boundary_radius).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let mut weights = WeightedBlendSet::default();
    if let Some(center) = center_sample {
        weights.add(center, 1.0 - radial);
    }
    weights.add(ring[left].sample_index, radial * (1.0 - angular));
    weights.add(ring[right].sample_index, radial * angular);
    weights.finish(center_sample.unwrap_or(ring[left].sample_index))
}

fn barycentric_weights_2d(
    point: [f32; 2],
    a: [f32; 2],
    b: [f32; 2],
    c: [f32; 2],
) -> Option<[f32; 3]> {
    let v0 = [b[0] - a[0], b[1] - a[1]];
    let v1 = [c[0] - a[0], c[1] - a[1]];
    let v2 = [point[0] - a[0], point[1] - a[1]];
    let denominator = v0[0] * v1[1] - v1[0] * v0[1];
    if denominator.abs() <= 1.0e-8 {
        return None;
    }
    let v = (v2[0] * v1[1] - v1[0] * v2[1]) / denominator;
    let w = (v0[0] * v2[1] - v2[0] * v0[1]) / denominator;
    let u = 1.0 - v - w;
    Some([u, v, w])
}

fn point_segment_projection(point: [f32; 2], a: [f32; 2], b: [f32; 2]) -> (f32, f32) {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [point[0] - a[0], point[1] - a[1]];
    let length2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if length2 <= 1.0e-12 {
        0.0
    } else {
        ((ap[0] * ab[0] + ap[1] * ab[1]) / length2).clamp(0.0, 1.0)
    };
    let projected = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    let dx = point[0] - projected[0];
    let dy = point[1] - projected[1];
    (t, dx * dx + dy * dy)
}

fn triangulated_blend2d_weights(
    samples: &[CompiledBlendSample2D],
    triangles: &[CompiledBlendTriangle],
    x: f32,
    y: f32,
) -> WeightedBlendSet {
    let point = [x, y];
    for (sample_index, sample) in samples.iter().enumerate() {
        if blend2d_position_distance_squared(point, sample.position) <= 1.0e-10 {
            let mut weights = WeightedBlendSet::default();
            weights.add(sample_index, 1.0);
            return weights.finish(sample_index);
        }
    }
    for triangle in triangles {
        let [ia, ib, ic] = triangle.samples;
        let Some(barycentric) = barycentric_weights_2d(
            point,
            samples[ia].position,
            samples[ib].position,
            samples[ic].position,
        ) else {
            continue;
        };
        if barycentric.iter().all(|weight| *weight >= -1.0e-5) {
            let mut weights = WeightedBlendSet::default();
            weights.add(ia, barycentric[0].max(0.0));
            weights.add(ib, barycentric[1].max(0.0));
            weights.add(ic, barycentric[2].max(0.0));
            return weights.finish(ia);
        }
    }
    let mut best: Option<(f32, usize, usize, f32)> = None;
    for triangle in triangles {
        let [a, b, c] = triangle.samples;
        for (left, right) in [(a, b), (b, c), (c, a)] {
            let (t, distance2) =
                point_segment_projection(point, samples[left].position, samples[right].position);
            let edge_key = (left.min(right), left.max(right));
            let replace = match best {
                None => true,
                Some((best_distance2, best_left, best_right, _)) => {
                    distance2 < best_distance2 - 1.0e-8
                        || ((distance2 - best_distance2).abs() <= 1.0e-8
                            && edge_key < (best_left, best_right))
                }
            };
            if replace {
                best = Some((
                    distance2,
                    edge_key.0,
                    edge_key.1,
                    if left <= right { t } else { 1.0 - t },
                ));
            }
        }
    }
    let (_, left, right, t) = best.unwrap_or((0.0, 0, 0, 0.0));
    let mut weights = WeightedBlendSet::default();
    weights.add(left, 1.0 - t);
    weights.add(right, t);
    weights.finish(left)
}

fn blend2d_weights(
    samples: &[CompiledBlendSample2D],
    domain: &CompiledBlend2DDomain,
    x: f32,
    y: f32,
) -> WeightedBlendSet {
    match domain {
        CompiledBlend2DDomain::Cartesian {
            x_values,
            y_values,
            grid,
        } => cartesian_blend2d_weights(x_values, y_values, grid, x, y),
        CompiledBlend2DDomain::Directional {
            center_sample,
            ring,
        } => directional_blend2d_weights(*center_sample, ring, x, y),
        CompiledBlend2DDomain::Triangulated { triangles } => {
            triangulated_blend2d_weights(samples, triangles, x, y)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SyncMarkerIntervalPhase {
    from_marker: usize,
    to_marker: usize,
    alpha: f32,
    semantic_cycle: i64,
}
