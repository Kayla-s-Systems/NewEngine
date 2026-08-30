fn world_scale(world: &World, entity: EntityId) -> Vec3 {
    if let Some(global) = world.get::<GlobalTransform>(entity) {
        let (scale, _, _) = global.0.to_scale_rotation_translation();
        if scale.x.is_finite() && scale.y.is_finite() && scale.z.is_finite() {
            return scale;
        }
    }
    world
        .get::<Transform>(entity)
        .map(|transform| transform.scale)
        .unwrap_or(Vec3::ONE)
}

fn zone_metrics(zone: &ResolvedEnvironmentZone, point: Vec3) -> Option<(f32, f32)> {
    let local = zone.rotation.inverse() * (point - zone.center);
    let ax = local.x.abs();
    let ay = local.y.abs();
    let az = local.z.abs();
    if ax > zone.half_extents.x || ay > zone.half_extents.y || az > zone.half_extents.z {
        return None;
    }

    let dx = zone.half_extents.x - ax;
    let dy = zone.half_extents.y - ay;
    let dz = zone.half_extents.z - az;
    let edge_distance = dx.min(dy).min(dz).max(0.0);
    let blend = zone.zone.blend_distance;
    let influence = if blend <= 1.0e-5 {
        1.0
    } else {
        (edge_distance / blend).clamp(0.0, 1.0)
    };
    let normalized_center_distance = (ax / zone.half_extents.x.max(1.0e-5))
        .max(ay / zone.half_extents.y.max(1.0e-5))
        .max(az / zone.half_extents.z.max(1.0e-5));
    Some((influence, normalized_center_distance))
}

fn select_membership(zones: &[ResolvedEnvironmentZone], point: Vec3) -> Option<ZoneMembership> {
    // Membership resolution runs once per emitter. Preserve the exact authored ordering
    // (priority desc, center distance asc, stable key asc) without allocating and sorting.
    let mut best: Option<(usize, i32, f32, f32, u64)> = None;
    for (zone_index, zone) in zones.iter().enumerate() {
        let Some((influence, normalized_center_distance)) = zone_metrics(zone, point) else {
            continue;
        };
        let candidate = (
            zone_index,
            zone.zone.priority,
            normalized_center_distance,
            influence,
            zone.stable_key,
        );
        let replace = match best {
            None => true,
            Some((_, best_priority, best_distance, _, best_stable_key)) => {
                candidate.1 > best_priority
                    || (candidate.1 == best_priority
                        && (candidate.2.total_cmp(&best_distance).is_lt()
                            || (candidate.2.total_cmp(&best_distance).is_eq()
                                && candidate.4 < best_stable_key)))
            }
        };
        if replace {
            best = Some(candidate);
        }
    }
    best.map(
        |(zone_index, _, _, influence, _)| ZoneMembership {
            zone_index,
            influence,
        },
    )
}

const SPEED_OF_SOUND_MPS: f32 = 343.0;

fn fresh_reflection_observation(
    observation: Option<&AudioEarlyReflectionObservation>,
    source_world: Vec3,
    receiver_world: Vec3,
) -> Option<&AudioEarlyReflectionObservation> {
    observation.filter(|observation| {
        vec3_array_distance(observation.source_position, source_world) <= 0.75
            && vec3_array_distance(observation.listener_position, receiver_world) <= 0.75
            && !observation.paths.is_empty()
    })
}

#[inline]
fn direction_array(origin: Vec3, target: Vec3) -> [f32; 3] {
    let delta = target - origin;
    let length = delta.length();
    if !length.is_finite() || length <= 1.0e-5 {
        [0.0; 3]
    } else {
        [delta.x / length, delta.y / length, delta.z / length]
    }
}

fn explicit_early_reflection_field(
    preset: AudioReverbPreset,
    source_world: Vec3,
    receiver_world: Vec3,
    observation: Option<&AudioEarlyReflectionObservation>,
) -> AudioEarlyReflectionField {
    let Some(observation) = fresh_reflection_observation(observation, source_world, receiver_world)
    else {
        return AudioEarlyReflectionField::empty();
    };
    let preset = preset.sanitized();
    if preset.early_reflections_gain <= 1.0e-5 {
        return AudioEarlyReflectionField::empty();
    }
    let direct = source_world.distance(receiver_world).max(1.0e-4);
    let mut candidates =
        Vec::with_capacity(observation.paths.len() + observation.second_order_paths.len());
    for path in observation
        .paths
        .iter()
        .filter(|path| path.visible && path.path_length_m.is_finite())
    {
        let ratio = (direct / path.path_length_m.max(direct))
            .clamp(0.2, 1.0)
            .sqrt();
        let material = path.material.sanitized();
        let material_gain = if path.material_known {
            material.reflection_gain
        } else {
            1.0
        };
        let high_frequency_gain = if path.material_known {
            material.high_frequency_gain()
        } else {
            1.0
        };
        candidates.push(AudioEarlyReflectionTap {
            delay_ms: (path.excess_length_m.max(0.0) / SPEED_OF_SOUND_MPS * 1_000.0)
                .clamp(0.0, 500.0),
            gain: (preset.early_reflections_gain * ratio * material_gain).clamp(0.0, 2.0),
            high_frequency_gain,
            direction: path.arrival_direction,
            order: 1,
        });
    }
    for path in observation
        .second_order_paths
        .iter()
        .filter(|path| path.visible && path.path_length_m.is_finite())
    {
        let ratio = (direct / path.path_length_m.max(direct))
            .clamp(0.2, 1.0)
            .sqrt();
        let mut material_gain = 1.0_f32;
        let mut high_frequency_gain = 1.0_f32;
        for bounce in 0..2 {
            if path.material_known[bounce] {
                let material = path.materials[bounce].sanitized();
                material_gain *= material.reflection_gain;
                high_frequency_gain *= material.high_frequency_gain();
            }
        }
        candidates.push(AudioEarlyReflectionTap {
            delay_ms: (path.excess_length_m.max(0.0) / SPEED_OF_SOUND_MPS * 1_000.0)
                .clamp(0.0, 500.0),
            gain: (preset.early_reflections_gain * ratio * material_gain).clamp(0.0, 2.0),
            high_frequency_gain: high_frequency_gain.clamp(0.0, 1.0),
            direction: path.arrival_direction,
            order: 2,
        });
    }
    candidates.retain(|tap| tap.gain > 1.0e-5);
    candidates.sort_by(|a, b| {
        b.gain
            .total_cmp(&a.gain)
            .then_with(|| a.delay_ms.total_cmp(&b.delay_ms))
            .then_with(|| a.order.cmp(&b.order))
    });
    candidates.truncate(AUDIO_MAX_EARLY_REFLECTION_TAPS);
    let mut field = AudioEarlyReflectionField::empty();
    field.count = candidates.len() as u8;
    for (slot, tap) in field.taps.iter_mut().zip(candidates) {
        *slot = tap;
    }
    field.sanitized()
}

fn geometry_adjusted_reverb(
    zone: &ResolvedEnvironmentZone,
    source_world: Vec3,
    receiver_world: Vec3,
    preset: AudioReverbPreset,
    observation: Option<&AudioEarlyReflectionObservation>,
) -> AudioReverbPreset {
    let preset = preset.sanitized();
    if zone.zone.kind == AudioEnvironmentKind::Outdoor {
        return preset;
    }

    if let Some(observation) =
        fresh_reflection_observation(observation, source_world, receiver_world)
    {
        let mut visible = observation
            .paths
            .iter()
            .filter(|path| path.visible && path.path_length_m.is_finite())
            .collect::<Vec<_>>();
        visible.sort_by(|a, b| {
            a.path_length_m
                .total_cmp(&b.path_length_m)
                .then_with(|| a.face_index.cmp(&b.face_index))
        });
        if visible.is_empty() {
            return AudioReverbPreset {
                early_reflections_gain: 0.0,
                early_reflections_high_frequency_gain: 0.0,
                ..preset
            }
            .sanitized();
        }

        let direct = source_world.distance(receiver_world).max(1.0e-4);
        let first_excess = visible[0].excess_length_m.max(0.0);
        let fourth_index = visible.len().min(4).saturating_sub(1);
        let fourth_excess = visible[fourth_index].excess_length_m.max(first_excess);
        let visibility =
            (visible.len() as f32 / observation.paths.len().max(1) as f32).clamp(0.0, 1.0);
        let mut broadband_sum = 0.0_f32;
        let mut hf_sum = 0.0_f32;
        let mut weight_sum = 0.0_f32;
        for path in visible.iter().take(4) {
            let ratio = (direct / path.path_length_m.max(direct))
                .clamp(0.2, 1.0)
                .sqrt();
            let material_gain = if path.material_known {
                path.material.sanitized().reflection_gain
            } else {
                1.0
            };
            let hf_gain = if path.material_known {
                path.material.sanitized().high_frequency_gain()
            } else {
                1.0
            };
            broadband_sum += ratio * material_gain;
            hf_sum += ratio * material_gain * hf_gain;
            weight_sum += ratio * material_gain;
        }
        let broadband = if visible.is_empty() {
            0.0
        } else {
            broadband_sum / visible.len().min(4) as f32
        };
        let hf_retention = if weight_sum > 1.0e-5 {
            (hf_sum / weight_sum).clamp(0.0, 1.0)
        } else {
            0.0
        };
        return AudioReverbPreset {
            early_reflections_gain: (preset.early_reflections_gain * broadband * visibility.sqrt())
                .clamp(0.0, 2.0),
            early_reflections_high_frequency_gain: hf_retention,
            pre_delay_ms: (first_excess / SPEED_OF_SOUND_MPS * 1_000.0).clamp(0.0, 250.0),
            early_reflections_spread_ms: ((fourth_excess - first_excess) / SPEED_OF_SOUND_MPS
                * 1_000.0)
                .clamp(0.0, 250.0),
            ..preset
        }
        .sanitized();
    }

    // No fresh visibility/material observation yet: retain the geometric first-order baseline.
    let inverse = zone.rotation.inverse();
    let source = inverse * (source_world - zone.center);
    let receiver = inverse * (receiver_world - zone.center);
    let direct = source.distance(receiver).max(1.0e-4);
    let extents = zone.half_extents;
    let mut excess = [0.0_f32; 6];
    let faces = [
        (0usize, extents.x),
        (0usize, -extents.x),
        (1usize, extents.y),
        (1usize, -extents.y),
        (2usize, extents.z),
        (2usize, -extents.z),
    ];
    for (index, (axis, plane)) in faces.into_iter().enumerate() {
        let mut mirrored = source;
        match axis {
            0 => mirrored.x = 2.0 * plane - source.x,
            1 => mirrored.y = 2.0 * plane - source.y,
            _ => mirrored.z = 2.0 * plane - source.z,
        }
        excess[index] = (mirrored.distance(receiver) - direct).max(0.0);
    }
    excess.sort_by(f32::total_cmp);
    let first = excess[0];
    let fourth = excess[3];
    let first_path = direct + first;
    let path_ratio = (direct / first_path.max(direct)).clamp(0.2, 1.0);
    AudioReverbPreset {
        early_reflections_gain: (preset.early_reflections_gain * path_ratio.sqrt()).clamp(0.0, 2.0),
        pre_delay_ms: (first / SPEED_OF_SOUND_MPS * 1_000.0).clamp(0.0, 250.0),
        early_reflections_spread_ms: ((fourth - first) / SPEED_OF_SOUND_MPS * 1_000.0)
            .clamp(0.0, 250.0),
        ..preset
    }
    .sanitized()
}

