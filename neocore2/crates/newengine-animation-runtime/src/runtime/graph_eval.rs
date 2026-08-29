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
        let sample_scale = sample_pose
            .scale
            .or(bind[index].scale)
            .unwrap_or([1.0; 3]);
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
        base[index].translation = vec3_array(
            base_translation + (layer_translation - bind_translation) * joint_weight,
        );

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

fn blend_tree_segment(
    samples: &[CompiledBlendSample1D],
    value: f32,
) -> (usize, usize, f32) {
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

fn point_segment_projection(
    point: [f32; 2],
    a: [f32; 2],
    b: [f32; 2],
) -> (f32, f32) {
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
            let (t, distance2) = point_segment_projection(
                point,
                samples[left].position,
                samples[right].position,
            );
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
                best = Some((distance2, edge_key.0, edge_key.1, if left <= right { t } else { 1.0 - t }));
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

fn sync_marker_time(track: &CompiledSyncMarkerTrack, marker_index: usize) -> Option<f32> {
    track
        .markers
        .iter()
        .find(|marker| marker.marker_index == marker_index)
        .map(|marker| marker.time_seconds)
}

fn sync_marker_interval_phase(
    clip: &AnimationClip,
    track: &CompiledSyncMarkerTrack,
    playback_time_seconds: f32,
) -> Option<SyncMarkerIntervalPhase> {
    if track.markers.len() < 2 || clip.duration_seconds <= 1.0e-6 {
        return None;
    }
    let duration = clip.duration_seconds;
    let playback = playback_time_seconds.max(0.0);
    let clip_cycle = (playback / duration).floor() as i64;
    let local_time = playback.rem_euclid(duration);
    let next_index = track
        .markers
        .iter()
        .position(|marker| marker.time_seconds > local_time)
        .unwrap_or(0);
    let previous_index = if next_index == 0 {
        track.markers.len() - 1
    } else {
        next_index - 1
    };
    let previous = track.markers[previous_index];
    let next = track.markers[next_index];
    let previous_clip_cycle = if next_index == 0 && local_time < track.markers[0].time_seconds {
        clip_cycle - 1
    } else {
        clip_cycle
    };
    let start = previous.time_seconds + previous_clip_cycle as f32 * duration;
    let next_clip_cycle = if next_index == 0 {
        previous_clip_cycle + 1
    } else {
        previous_clip_cycle
    };
    let end = next.time_seconds + next_clip_cycle as f32 * duration;
    let marker_zero_time = sync_marker_time(track, 0)?;
    let semantic_cycle = previous_clip_cycle
        - i64::from(previous.time_seconds + 1.0e-6 < marker_zero_time);
    let span = (end - start).max(1.0e-6);
    Some(SyncMarkerIntervalPhase {
        from_marker: previous.marker_index,
        to_marker: next.marker_index,
        alpha: ((playback - start) / span).clamp(0.0, 1.0),
        semantic_cycle,
    })
}

fn map_marker_phase_raw(
    target_clip: &AnimationClip,
    target_track: &CompiledSyncMarkerTrack,
    phase: SyncMarkerIntervalPhase,
) -> Option<f32> {
    let target_duration = target_clip.duration_seconds.max(1.0e-6);
    let pair_index = target_track.markers.iter().enumerate().find_map(|(index, marker)| {
        let next = target_track.markers[(index + 1) % target_track.markers.len()];
        (marker.marker_index == phase.from_marker && next.marker_index == phase.to_marker)
            .then_some(index)
    })?;
    let start_marker = target_track.markers[pair_index];
    let next_index = (pair_index + 1) % target_track.markers.len();
    let next_marker = target_track.markers[next_index];
    let marker_zero_time = sync_marker_time(target_track, 0)?;
    let target_clip_cycle = phase.semantic_cycle
        + i64::from(start_marker.time_seconds + 1.0e-6 < marker_zero_time);
    let start = start_marker.time_seconds + target_clip_cycle as f32 * target_duration;
    let end_cycle = if next_index == 0 {
        target_clip_cycle + 1
    } else {
        target_clip_cycle
    };
    let end = next_marker.time_seconds + end_cycle as f32 * target_duration;
    Some(start + (end - start) * phase.alpha)
}

fn remap_marker_playback_time(
    source_clip: &AnimationClip,
    source_track: &CompiledSyncMarkerTrack,
    target_clip: &AnimationClip,
    target_track: &CompiledSyncMarkerTrack,
    source_playback_time_seconds: f32,
) -> Option<f32> {
    let phase = sync_marker_interval_phase(source_clip, source_track, source_playback_time_seconds)?;
    let raw = map_marker_phase_raw(target_clip, target_track, phase)?;
    // Marker rotations may make the mathematically corresponding target occurrence precede t=0.
    // Apply one constant whole-cycle offset chosen from the source origin so the remapped playback
    // clock stays non-negative and monotonic instead of wrapping backward at a marker boundary.
    let origin_phase = sync_marker_interval_phase(source_clip, source_track, 0.0)?;
    let origin_raw = map_marker_phase_raw(target_clip, target_track, origin_phase)?;
    let target_duration = target_clip.duration_seconds.max(1.0e-6);
    let cycle_offset = if origin_raw < 0.0 {
        (-origin_raw / target_duration).ceil()
    } else {
        0.0
    };
    Some(raw + cycle_offset * target_duration)
}

fn marker_matched_playback_time(
    graph: &CompiledAnimationGraph,
    marker_group_index: usize,
    source_clip_index: usize,
    target_clip_index: usize,
    source_playback_time_seconds: f32,
) -> Option<f32> {
    let source_track = graph
        .sync_marker_tracks
        .get(marker_group_index)?
        .get(source_clip_index)?
        .as_ref()?;
    let target_track = graph
        .sync_marker_tracks
        .get(marker_group_index)?
        .get(target_clip_index)?
        .as_ref()?;
    remap_marker_playback_time(
        &graph.clips[source_clip_index].clip,
        source_track,
        &graph.clips[target_clip_index].clip,
        target_track,
        source_playback_time_seconds,
    )
}

fn sync_matched_motion_time(
    graph: &CompiledAnimationGraph,
    source_motion: &CompiledAnimationMotion,
    source_motion_time: f32,
    target_motion: &CompiledAnimationMotion,
) -> Option<f32> {
    let source_sync = source_motion.sync()?;
    let target_sync = target_motion.sync()?;
    if source_sync.key != target_sync.key {
        return None;
    }
    let source_playback = source_motion_time * source_motion.first_speed();
    if let (Some(source_group), Some(target_group)) = (
        source_sync.marker_group_index,
        target_sync.marker_group_index,
    ) {
        if source_group == target_group {
            if let Some(target_playback) = marker_matched_playback_time(
                graph,
                source_group,
                source_motion.first_clip_index(),
                target_motion.first_clip_index(),
                source_playback,
            ) {
                return Some(target_playback / target_motion.first_speed().max(1.0e-6));
            }
        }
    }
    let phase = motion_unwrapped_phase(graph, source_motion, source_motion_time);
    let target_duration = motion_reference_duration(graph, target_motion);
    Some(phase * target_duration / target_motion.first_speed().max(1.0e-6))
}

fn synchronized_blend_sample_playback_time(
    graph: &CompiledAnimationGraph,
    leader_clip_index: usize,
    leader_speed: f32,
    target_clip_index: usize,
    target_speed: f32,
    sync: Option<&CompiledMotionSync>,
    motion_time_seconds: f32,
) -> f32 {
    let Some(sync) = sync else {
        return motion_time_seconds * target_speed;
    };
    let leader_playback = motion_time_seconds * leader_speed;
    if target_clip_index == leader_clip_index {
        return leader_playback;
    }
    if let Some(group_index) = sync.marker_group_index {
        if let Some(mapped) = marker_matched_playback_time(
            graph,
            group_index,
            leader_clip_index,
            target_clip_index,
            leader_playback,
        ) {
            return mapped;
        }
    }
    let leader_duration = graph.clips[leader_clip_index].clip.duration_seconds.max(1.0e-6);
    let phase = leader_playback / leader_duration;
    phase * graph.clips[target_clip_index].clip.duration_seconds.max(1.0e-6)
}

fn sample_playback_time(
    graph: &CompiledAnimationGraph,
    motion: &CompiledAnimationMotion,
    sample_index: usize,
    motion_time_seconds: f32,
) -> f32 {
    match motion {
        CompiledAnimationMotion::Clip { speed, .. } => motion_time_seconds * *speed,
        CompiledAnimationMotion::Blend1D { samples, sync, .. } => {
            let sample = samples[sample_index];
            synchronized_blend_sample_playback_time(
                graph,
                samples[0].clip_index,
                samples[0].speed,
                sample.clip_index,
                sample.speed,
                sync.as_ref(),
                motion_time_seconds,
            )
        }
        CompiledAnimationMotion::Blend2D { samples, sync, .. } => {
            let sample = samples[sample_index];
            synchronized_blend_sample_playback_time(
                graph,
                samples[0].clip_index,
                samples[0].speed,
                sample.clip_index,
                sample.speed,
                sync.as_ref(),
                motion_time_seconds,
            )
        }
    }
}

fn seek_motion_cursors(
    graph: &CompiledAnimationGraph,
    motion: &CompiledAnimationMotion,
    motion_time_seconds: f32,
    runtime: &mut MotionPlaybackRuntime,
) -> Result<(), String> {
    if runtime.cursors.len() != motion.sample_count() {
        return Err("animation graph motion cursor shape mismatch".to_owned());
    }
    for sample_index in 0..runtime.cursors.len() {
        let time = sample_playback_time(graph, motion, sample_index, motion_time_seconds);
        if time <= f32::EPSILON {
            runtime.cursors[sample_index].restart();
        } else {
            runtime.cursors[sample_index].seek(time)?;
        }
    }
    Ok(())
}

struct ClipEventEmission<'a> {
    graph: &'a CompiledAnimationGraph,
    clip_index: usize,
    playback_time_seconds: f32,
    emit: bool,
    source: AnimationGraphEventSource,
    blend_weight: f32,
}

fn append_clip_events(
    emission: ClipEventEmission<'_>,
    cursor: &mut AnimationEventCursor,
    event_scratch: &mut Vec<AnimationEventOccurrence>,
    out: &mut Vec<AnimationGraphEventOccurrence>,
) -> Result<(), String> {
    event_scratch.clear();
    let clip = &emission.graph.clips[emission.clip_index].clip;
    cursor.advance(clip, emission.playback_time_seconds, event_scratch)?;
    if emission.emit {
        out.extend(event_scratch.iter().copied().map(|occurrence| {
            AnimationGraphEventOccurrence {
                source: emission.source,
                clip_index: emission.clip_index,
                event_index: occurrence.event_index,
                playback_time_seconds: occurrence.playback_time_seconds,
                loop_index: occurrence.loop_index,
                blend_weight: emission.blend_weight,
            }
        }));
    }
    Ok(())
}

struct MotionEvaluationContext<'a> {
    graph: &'a CompiledAnimationGraph,
    skeleton: &'a AnimationSkeletonRuntime,
    parameters: &'a [AnimationGraphParameterValue],
    source: AnimationGraphEventSource,
    emit_events: bool,
    source_weight: f32,
}

struct MotionEvaluationScratch<'a> {
    a: &'a mut Vec<JointLocalPose>,
    b: &'a mut Vec<JointLocalPose>,
    event_scratch: &'a mut Vec<AnimationEventOccurrence>,
    events: &'a mut Vec<AnimationGraphEventOccurrence>,
}

fn evaluate_motion(
    context: MotionEvaluationContext<'_>,
    motion: &CompiledAnimationMotion,
    motion_time_seconds: f32,
    runtime: &mut MotionPlaybackRuntime,
    out_pose: &mut Vec<JointLocalPose>,
    scratch: MotionEvaluationScratch<'_>,
) -> Result<MotionEvaluationMeta, String> {
    let graph = context.graph;
    if runtime.cursors.len() != motion.sample_count() {
        return Err("animation graph motion/runtime sample shape mismatch".to_owned());
    }
    match motion {
        CompiledAnimationMotion::Clip { clip_index, .. } => {
            let playback_time = sample_playback_time(graph, motion, 0, motion_time_seconds);
            let compiled_clip = &graph.clips[*clip_index];
            compiled_clip.clip.sample_local_pose_bound(
                playback_time,
                context.skeleton,
                &compiled_clip.binding,
                out_pose,
            )?;
            append_clip_events(
                ClipEventEmission {
                    graph,
                    clip_index: *clip_index,
                    playback_time_seconds: playback_time,
                    emit: context.emit_events,
                    source: context.source,
                    blend_weight: context.source_weight,
                },
                &mut runtime.cursors[0],
                scratch.event_scratch,
                scratch.events,
            )?;
            let mut meta = MotionEvaluationMeta::default();
            meta.add_root_source(0, *clip_index, playback_time, 1.0);
            Ok(meta)
        }
        CompiledAnimationMotion::Blend1D {
            parameter_index,
            samples,
            ..
        } => {
            let value = context
                .parameters
                .get(*parameter_index)
                .and_then(|parameter| parameter.as_float())
                .ok_or_else(|| "animation graph blend1d parameter/runtime type mismatch".to_owned())?;
            let (left_index, right_index, alpha) = blend_tree_segment(samples, value);
            let left = samples[left_index];
            let right = samples[right_index];
            let dominant_sample = if left_index == right_index || alpha < 0.5 {
                left_index
            } else {
                right_index
            };

            // Keep inactive cursors synchronized to current graph time without synthesizing their
            // historical markers if the blend parameter later enters their segment.
            for sample_index in 0..samples.len() {
                if sample_index == left_index || sample_index == right_index {
                    continue;
                }
                let playback_time =
                    sample_playback_time(graph, motion, sample_index, motion_time_seconds);
                runtime.cursors[sample_index].seek(playback_time)?;
            }

            let left_time = sample_playback_time(graph, motion, left_index, motion_time_seconds);
            let left_clip = &graph.clips[left.clip_index];
            left_clip.clip.sample_local_pose_bound(
                left_time,
                context.skeleton,
                &left_clip.binding,
                scratch.a,
            )?;
            append_clip_events(
                ClipEventEmission {
                    graph,
                    clip_index: left.clip_index,
                    playback_time_seconds: left_time,
                    emit: context.emit_events && dominant_sample == left_index,
                    source: context.source,
                    blend_weight: context.source_weight
                        * if left_index == right_index { 1.0 } else { 1.0 - alpha },
                },
                &mut runtime.cursors[left_index],
                scratch.event_scratch,
                scratch.events,
            )?;

            if left_index == right_index {
                out_pose.clear();
                out_pose.extend_from_slice(scratch.a);
                let mut meta = MotionEvaluationMeta::default();
                meta.add_root_source(left_index, left.clip_index, left_time, 1.0);
                return Ok(meta);
            }

            let right_time = sample_playback_time(graph, motion, right_index, motion_time_seconds);
            let right_clip = &graph.clips[right.clip_index];
            right_clip.clip.sample_local_pose_bound(
                right_time,
                context.skeleton,
                &right_clip.binding,
                scratch.b,
            )?;
            append_clip_events(
                ClipEventEmission {
                    graph,
                    clip_index: right.clip_index,
                    playback_time_seconds: right_time,
                    emit: context.emit_events && dominant_sample == right_index,
                    source: context.source,
                    blend_weight: context.source_weight * alpha,
                },
                &mut runtime.cursors[right_index],
                scratch.event_scratch,
                scratch.events,
            )?;
            blend_pose_arrays(scratch.a, scratch.b, alpha, out_pose)?;
            let mut meta = MotionEvaluationMeta::default();
            meta.add_root_source(left_index, left.clip_index, left_time, 1.0 - alpha);
            meta.add_root_source(right_index, right.clip_index, right_time, alpha);
            Ok(meta)
        }
        CompiledAnimationMotion::Blend2D {
            parameter_x_index,
            parameter_y_index,
            samples,
            domain,
            ..
        } => {
            let x = context
                .parameters
                .get(*parameter_x_index)
                .and_then(|parameter| parameter.as_float())
                .ok_or_else(|| "animation graph blend2d x parameter/runtime type mismatch".to_owned())?;
            let y = context
                .parameters
                .get(*parameter_y_index)
                .and_then(|parameter| parameter.as_float())
                .ok_or_else(|| "animation graph blend2d y parameter/runtime type mismatch".to_owned())?;
            let weights = blend2d_weights(samples, domain, x, y);
            let dominant = weights.dominant();

            // Inactive samples track the current synchronized clock without replaying historical
            // markers if the 2D query later enters their region.
            for sample_index in 0..samples.len() {
                if weights.contains(sample_index) {
                    continue;
                }
                let playback_time =
                    sample_playback_time(graph, motion, sample_index, motion_time_seconds);
                runtime.cursors[sample_index].seek(playback_time)?;
            }

            let mut accumulated_weight = 0.0_f32;
            let mut meta = MotionEvaluationMeta::default();
            let mut has_reference_pose = false;
            for entry in &weights.entries[..weights.len] {
                let sample = samples[entry.sample_index];
                let playback_time =
                    sample_playback_time(graph, motion, entry.sample_index, motion_time_seconds);
                let compiled_clip = &graph.clips[sample.clip_index];
                compiled_clip.clip.sample_local_pose_bound(
                    playback_time,
                    context.skeleton,
                    &compiled_clip.binding,
                    scratch.a,
                )?;
                append_clip_events(
                    ClipEventEmission {
                        graph,
                        clip_index: sample.clip_index,
                        playback_time_seconds: playback_time,
                        emit: context.emit_events && entry.sample_index == dominant.sample_index,
                        source: context.source,
                        blend_weight: context.source_weight * entry.weight,
                    },
                    &mut runtime.cursors[entry.sample_index],
                    scratch.event_scratch,
                    scratch.events,
                )?;
                if !has_reference_pose {
                    scratch.b.clone_from(scratch.a);
                    has_reference_pose = true;
                }
                accumulate_weighted_pose_sample(
                    out_pose,
                    scratch.a,
                    scratch.b,
                    context.skeleton.bind_locals(),
                    entry.weight,
                    accumulated_weight <= 1.0e-8,
                )?;
                accumulated_weight += entry.weight;
                meta.add_root_source(
                    entry.sample_index,
                    sample.clip_index,
                    playback_time,
                    entry.weight,
                );
            }
            finish_weighted_pose(out_pose, accumulated_weight)?;
            Ok(meta)
        }
    }
}

fn phase_matched_target_time(
    graph: &CompiledAnimationGraph,
    source_state: usize,
    source_time: f32,
    target_state: usize,
) -> f32 {
    sync_matched_motion_time(
        graph,
        &graph.states[source_state].motion,
        source_time,
        &graph.states[target_state].motion,
    )
    .unwrap_or(0.0)
}


impl AnimationGraphInstance {
    fn begin_transition(
        &mut self,
        graph: &CompiledAnimationGraph,
        transition: ActiveTransitionRuntime,
    ) -> Result<(), String> {
        let source_time = self.states[transition.from_state].time_seconds;
        let target_time = phase_matched_target_time(
            graph,
            transition.from_state,
            source_time,
            transition.to_state,
        );
        self.states[transition.to_state].time_seconds = target_time;
        seek_motion_cursors(
            graph,
            &graph.states[transition.to_state].motion,
            target_time,
            &mut self.states[transition.to_state].motion,
        )?;
        if !transition.source_is_frozen {
            self.frozen_transition_pose.clear();
        }
        self.transition = Some(transition);
        self.root_motion_source = None;
        Ok(())
    }

    fn freeze_last_base_pose(&mut self) -> bool {
        if self.last_base_pose.is_empty() {
            self.frozen_transition_pose.clear();
            false
        } else {
            self.frozen_transition_pose.clone_from(&self.last_base_pose);
            true
        }
    }

    /// Explicit state request used by validated `BlendToState` intents/tools. Authored transition
    /// tables remain the normal automatic path, but callers do not need a GameReady-specific
    /// controller to request a graph state. Explicit requests are authoritative and may interrupt
    /// any active transition; when a previously evaluated pose exists, it becomes the frozen source
    /// of the new blend so the request cannot snap to either endpoint of the interrupted transition.
    pub fn blend_to_state(
        &mut self,
        graph: &CompiledAnimationGraph,
        state: &str,
        blend_seconds: f32,
    ) -> Result<(), String> {
        if !blend_seconds.is_finite() || blend_seconds < 0.0 || blend_seconds > 60.0 {
            return Err(format!(
                "animation graph '{}' explicit blend duration is invalid duration={blend_seconds}",
                graph.name
            ));
        }
        let to_state = graph.state_index(state).ok_or_else(|| {
            format!("animation graph '{}' has no state '{state}'", graph.name)
        })?;

        if let Some(active) = self.transition {
            if active.to_state == to_state {
                return Ok(());
            }
            let from_state = active.to_state;
            let source_is_frozen = self.freeze_last_base_pose();
            self.active_state = from_state;
            return self.begin_transition(
                graph,
                ActiveTransitionRuntime {
                    from_state,
                    to_state,
                    elapsed_seconds: 0.0,
                    blend_seconds,
                    source_is_frozen,
                    group_id: None,
                    interruption: AnimationTransitionInterruptionPolicy::Never,
                },
            );
        }

        if self.active_state == to_state {
            return Ok(());
        }
        let from_state = self.active_state;
        self.begin_transition(
            graph,
            ActiveTransitionRuntime {
                from_state,
                to_state,
                elapsed_seconds: 0.0,
                blend_seconds,
                source_is_frozen: false,
                group_id: None,
                interruption: AnimationTransitionInterruptionPolicy::Never,
            },
        )
    }

    pub fn evaluate(
        &mut self,
        graph: &CompiledAnimationGraph,
        skeleton: &AnimationSkeletonRuntime,
        dt_seconds: f32,
        out: &mut AnimationGraphEvaluation,
    ) -> Result<(), String> {
        if self.parameters.len() != graph.parameters.len()
            || self.states.len() != graph.states.len()
            || self.layers.len() != graph.layers.len()
        {
            return Err(format!(
                "animation graph instance does not match compiled graph '{}'",
                graph.name
            ));
        }
        if skeleton.joint_count() == 0 {
            return Err("animation graph evaluation requires a non-empty skeleton".to_owned());
        }
        let dt = if dt_seconds.is_finite() && dt_seconds > 0.0 {
            dt_seconds.min(0.25)
        } else {
            0.0
        };
        out.events.clear();
        out.root_motion = AnimationRootMotionDelta::default();
        out.transition = None;

        // Advance independent layer clocks even when their effective weight is currently zero.
        // Their inactive event cursors are seeked below, preventing historical catch-up.
        for layer in &mut self.layers {
            layer.time_seconds += dt;
        }

        // Active authored transitions may yield to ready transitions from their destination state.
        // The active transition owns the interruption policy; candidate arbitration is already
        // deterministic because each per-state table is priority-descending/authored-order-ascending.
        if let Some(active) = self.transition {
            if active.interruption != AnimationTransitionInterruptionPolicy::Never {
                let source_time = self.states[active.to_state].time_seconds;
                let selected = graph.transitions_by_state[active.to_state]
                    .iter()
                    .find(|candidate| {
                        let group_allowed = match active.interruption {
                            AnimationTransitionInterruptionPolicy::Never => false,
                            AnimationTransitionInterruptionPolicy::SameGroup => {
                                active.group_id.is_some() && active.group_id == candidate.group_id
                            }
                            AnimationTransitionInterruptionPolicy::Any => true,
                        };
                        group_allowed
                            && transition_is_ready(
                                graph,
                                candidate,
                                &self.parameters,
                                source_time,
                            )
                    })
                    .cloned();
                if let Some(candidate) = selected {
                    let source_is_frozen = self.freeze_last_base_pose();
                    self.active_state = active.to_state;
                    self.begin_transition(
                        graph,
                        ActiveTransitionRuntime {
                            from_state: candidate.from_state,
                            to_state: candidate.to_state,
                            elapsed_seconds: 0.0,
                            blend_seconds: candidate.blend_seconds,
                            source_is_frozen,
                            group_id: candidate.group_id,
                            interruption: candidate.interruption,
                        },
                    )?;
                }
            }
        }

        if let Some(mut transition) = self.transition {
            if !transition.source_is_frozen {
                let from_speed = graph.states[transition.from_state].speed;
                self.states[transition.from_state].time_seconds += dt * from_speed;
            }
            let to_speed = graph.states[transition.to_state].speed;
            self.states[transition.to_state].time_seconds += dt * to_speed;
            transition.elapsed_seconds += dt;
            self.transition = Some(transition);
        } else {
            let state_index = self.active_state;
            self.states[state_index].time_seconds += dt * graph.states[state_index].speed;
            let source_time = self.states[state_index].time_seconds;
            let selected = graph.transitions_by_state[state_index]
                .iter()
                .find(|transition| {
                    transition_is_ready(graph, transition, &self.parameters, source_time)
                })
                .cloned();
            if let Some(transition) = selected {
                self.begin_transition(
                    graph,
                    ActiveTransitionRuntime {
                        from_state: transition.from_state,
                        to_state: transition.to_state,
                        elapsed_seconds: 0.0,
                        blend_seconds: transition.blend_seconds,
                        source_is_frozen: false,
                        group_id: transition.group_id,
                        interruption: transition.interruption,
                    },
                )?;
            }
        }

        let (root_state_index, root_meta, base_sync_state_index, base_sync_state_time);

        if let Some(transition) = self.transition {
            let alpha = transition_alpha(transition);
            let to_time = self.states[transition.to_state].time_seconds;
            if transition.source_is_frozen {
                if self.frozen_transition_pose.len() != skeleton.joint_count() {
                    return Err(format!(
                        "animation graph '{}' interrupted transition has invalid frozen pose joints={} skeleton={}",
                        graph.name,
                        self.frozen_transition_pose.len(),
                        skeleton.joint_count()
                    ));
                }
                self.scratch_a.clone_from(&self.frozen_transition_pose);
                let to_meta = evaluate_motion(
                    MotionEvaluationContext {
                        graph,
                        skeleton,
                        parameters: &self.parameters,
                        source: AnimationGraphEventSource::State(transition.to_state),
                        emit_events: alpha >= 0.5,
                        source_weight: alpha,
                    },
                    &graph.states[transition.to_state].motion,
                    to_time,
                    &mut self.states[transition.to_state].motion,
                    &mut self.scratch_b,
                    MotionEvaluationScratch {
                        a: &mut self.scratch_layer,
                        b: &mut out.local_pose,
                        event_scratch: &mut self.event_scratch,
                        events: &mut out.events,
                    },
                )?;
                blend_pose_arrays(
                    &self.scratch_a,
                    &self.scratch_b,
                    alpha,
                    &mut out.local_pose,
                )?;
                root_meta = to_meta;
                root_state_index = transition.to_state;
                base_sync_state_index = transition.to_state;
                base_sync_state_time = to_time;
            } else {
                let from_time = self.states[transition.from_state].time_seconds;
                let emit_from = alpha < 0.5;
                let from_meta = evaluate_motion(
                    MotionEvaluationContext {
                        graph,
                        skeleton,
                        parameters: &self.parameters,
                        source: AnimationGraphEventSource::State(transition.from_state),
                        emit_events: emit_from,
                        source_weight: 1.0 - alpha,
                    },
                    &graph.states[transition.from_state].motion,
                    from_time,
                    &mut self.states[transition.from_state].motion,
                    &mut self.scratch_a,
                    MotionEvaluationScratch {
                        a: &mut self.scratch_layer,
                        b: &mut out.local_pose,
                        event_scratch: &mut self.event_scratch,
                        events: &mut out.events,
                    },
                )?;
                let to_meta = evaluate_motion(
                    MotionEvaluationContext {
                        graph,
                        skeleton,
                        parameters: &self.parameters,
                        source: AnimationGraphEventSource::State(transition.to_state),
                        emit_events: !emit_from,
                        source_weight: alpha,
                    },
                    &graph.states[transition.to_state].motion,
                    to_time,
                    &mut self.states[transition.to_state].motion,
                    &mut self.scratch_b,
                    MotionEvaluationScratch {
                        a: &mut self.scratch_layer,
                        b: &mut out.local_pose,
                        event_scratch: &mut self.event_scratch,
                        events: &mut out.events,
                    },
                )?;
                blend_pose_arrays(
                    &self.scratch_a,
                    &self.scratch_b,
                    alpha,
                    &mut out.local_pose,
                )?;
                root_meta = if emit_from { from_meta } else { to_meta };
                root_state_index = if emit_from {
                    transition.from_state
                } else {
                    transition.to_state
                };
                let root_state_time = self.states[root_state_index].time_seconds;
                base_sync_state_index = root_state_index;
                base_sync_state_time = root_state_time;
            }
            out.transition = Some(AnimationGraphTransitionSnapshot {
                from_state: transition.from_state,
                to_state: transition.to_state,
                alpha,
            });
            if alpha >= 1.0 - 1.0e-6 {
                self.active_state = transition.to_state;
                self.transition = None;
                self.frozen_transition_pose.clear();
            }
        } else {
            let state_index = self.active_state;
            let state_time = self.states[state_index].time_seconds;
            root_meta = evaluate_motion(
                MotionEvaluationContext {
                    graph,
                    skeleton,
                    parameters: &self.parameters,
                    source: AnimationGraphEventSource::State(state_index),
                    emit_events: true,
                    source_weight: 1.0,
                },
                &graph.states[state_index].motion,
                state_time,
                &mut self.states[state_index].motion,
                &mut out.local_pose,
                MotionEvaluationScratch {
                    a: &mut self.scratch_a,
                    b: &mut self.scratch_b,
                    event_scratch: &mut self.event_scratch,
                    events: &mut out.events,
                },
            )?;
            base_sync_state_index = state_index;
            base_sync_state_time = state_time;
            root_state_index = state_index;
        }

        let root_mode = graph.states[root_state_index].root_motion;
        if root_mode != AnimationRootMotionMode::Disabled {
            if let Some(root_joint) = graph.root_motion_joint_index {
                out.root_motion = self.extract_root_motion(
                    graph,
                    skeleton,
                    root_joint,
                    root_state_index,
                    self.states[root_state_index].time_seconds,
                    root_meta,
                )?;
            }
        } else {
            self.root_motion_source = None;
        }

        // Interruption snapshots own only the base state-machine pose. Graph layers are evaluated
        // again every frame, so caching the post-layer result here would apply them twice after an
        // interruption. Root-motion extraction is normalized in the cached source as well.
        self.last_base_pose.clone_from(&out.local_pose);
        if root_mode == AnimationRootMotionMode::ExtractAndRemove {
            if let Some(root_joint) = graph.root_motion_joint_index {
                if root_joint < self.last_base_pose.len() {
                    self.last_base_pose[root_joint] = skeleton.bind_locals()[root_joint];
                }
            }
        }

        // Layer stack is evaluated after the base state machine. Matching sync groups borrow the
        // authoritative unwrapped phase from the current event/root leader state.
        for layer_index in 0..graph.layers.len() {
            let layer = &graph.layers[layer_index];
            let weight = layer_effective_weight(layer, &self.parameters);
            let synchronized_time = sync_matched_motion_time(
                graph,
                &graph.states[base_sync_state_index].motion,
                base_sync_state_time,
                &layer.motion,
            );
            let layer_time = synchronized_time.unwrap_or(self.layers[layer_index].time_seconds);
            if weight <= 1.0e-6 {
                seek_motion_cursors(
                    graph,
                    &layer.motion,
                    layer_time,
                    &mut self.layers[layer_index].motion,
                )?;
                continue;
            }
            let emit_events = weight + 1.0e-6 >= layer.event_weight_threshold;
            let _meta = evaluate_motion(
                MotionEvaluationContext {
                    graph,
                    skeleton,
                    parameters: &self.parameters,
                    source: AnimationGraphEventSource::Layer(layer_index),
                    emit_events,
                    source_weight: weight,
                },
                &layer.motion,
                layer_time,
                &mut self.layers[layer_index].motion,
                &mut self.scratch_layer,
                MotionEvaluationScratch {
                    a: &mut self.scratch_a,
                    b: &mut self.scratch_b,
                    event_scratch: &mut self.event_scratch,
                    events: &mut out.events,
                },
            )?;
            match layer.mode {
                AnimationLayerBlendMode::Override => apply_override_layer(
                    &mut out.local_pose,
                    &self.scratch_layer,
                    &layer.mask,
                    weight,
                )?,
                AnimationLayerBlendMode::Additive => apply_additive_layer(
                    &mut out.local_pose,
                    &self.scratch_layer,
                    skeleton.bind_locals(),
                    &layer.mask,
                    weight,
                )?,
            }
        }

        if root_mode == AnimationRootMotionMode::ExtractAndRemove {
            if let Some(root_joint) = graph.root_motion_joint_index {
                if root_joint < out.local_pose.len() {
                    out.local_pose[root_joint] = skeleton.bind_locals()[root_joint];
                }
            }
        }

        out.active_state = self.active_state;
        if out.local_pose.len() != skeleton.joint_count() {
            return Err(format!(
                "animation graph '{}' produced incomplete pose joints={} skeleton={}",
                graph.name,
                out.local_pose.len(),
                skeleton.joint_count()
            ));
        }
        Ok(())
    }

    fn extract_root_motion(
        &mut self,
        graph: &CompiledAnimationGraph,
        skeleton: &AnimationSkeletonRuntime,
        root_joint: usize,
        state_index: usize,
        motion_time_seconds: f32,
        meta: MotionEvaluationMeta,
    ) -> Result<AnimationRootMotionDelta, String> {
        let current_source = RootMotionRuntimeSource {
            state_index,
            motion_time_seconds,
        };
        let Some(previous) = self.root_motion_source.replace(current_source) else {
            return Ok(AnimationRootMotionDelta::default());
        };
        if previous.state_index != state_index || motion_time_seconds < previous.motion_time_seconds {
            return Ok(AnimationRootMotionDelta::default());
        }
        if meta.root_source_count == 0 {
            return Ok(AnimationRootMotionDelta::default());
        }
        let motion = &graph.states[state_index].motion;
        let mut translation = [0.0_f32; 3];
        let mut rotation_acc = [0.0_f32; 4];
        let mut total_weight = 0.0_f32;
        for source in &meta.root_sources[..meta.root_source_count] {
            if source.sample_index >= motion.sample_count() {
                return Err(format!(
                    "animation root-motion sample index outside motion samples state={} sample={} samples={}",
                    state_index,
                    source.sample_index,
                    motion.sample_count()
                ));
            }
            let previous_playback = sample_playback_time(
                graph,
                motion,
                source.sample_index,
                previous.motion_time_seconds,
            );
            if source.playback_time_seconds + 1.0e-6 < previous_playback {
                continue;
            }
            let compiled_clip = &graph.clips[source.clip_index];
            let delta = root_motion_delta_between(
                &compiled_clip.clip,
                &compiled_clip.binding,
                skeleton,
                root_joint,
                previous_playback,
                source.playback_time_seconds,
            )?;
            let weight = source.weight.max(0.0);
            for (component, value) in translation.iter_mut().zip(delta.translation) {
                *component += value * weight;
            }
            let mut rotation = delta.rotation;
            if rotation[3] < 0.0 {
                for component in &mut rotation {
                    *component = -*component;
                }
            }
            for (component, value) in rotation_acc.iter_mut().zip(rotation) {
                *component += value * weight;
            }
            total_weight += weight;
        }
        if total_weight <= 1.0e-8 {
            return Ok(AnimationRootMotionDelta::default());
        }
        let rotation = Quat::from_xyzw(
            rotation_acc[0],
            rotation_acc[1],
            rotation_acc[2],
            rotation_acc[3],
        )
        .normalize_or_identity();
        Ok(AnimationRootMotionDelta {
            translation,
            rotation: quat_array(rotation),
            valid: true,
        })
    }
}

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
    let mut local = sample_bound_joint_wrapped(
        clip,
        binding,
        skeleton,
        skeleton_joint,
        local_time,
    )?;
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
    let previous = sample_bound_joint_unwrapped(
        clip,
        binding,
        skeleton,
        root_joint,
        previous_time_seconds,
    )?;
    let current = sample_bound_joint_unwrapped(
        clip,
        binding,
        skeleton,
        root_joint,
        current_time_seconds,
    )?;
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
