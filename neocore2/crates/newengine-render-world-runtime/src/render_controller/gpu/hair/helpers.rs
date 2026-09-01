use super::*;

pub(super) fn scene_bindings_ready(
    scene: &HairSceneV1,
    registry: &HairGroomRegistryV1,
    poses: Option<&HairSkinPoseRegistryV1>,
    skinning_supported: bool,
    capsules_supported: bool,
) -> bool {
    scene
        .instances
        .iter()
        .filter(|instance| instance.quality != HairQualityTier::Off)
        .all(|instance| {
            let Some(groom) = registry.get(&instance.groom) else {
                return false;
            };
            if instance.simulation.collision == HairCollisionMode::Capsules
                && !groom.collision_capsules.is_empty()
                && !capsules_supported
            {
                return false;
            }
            let Some(pose_id) = instance.skin_pose_id else {
                return true;
            };
            if !skinning_supported {
                return false;
            }
            let Some(pose) = poses.and_then(|poses| poses.get(pose_id)) else {
                return false;
            };
            let joint_count = pose.joint_deforms.len();
            groom
                .guide_strands
                .iter()
                .all(|strand| usize::from(strand.root_joint_index) < joint_count)
                && groom
                    .collision_capsules
                    .iter()
                    .all(|capsule| usize::from(capsule.joint_index) < joint_count)
        })
}

pub(super) fn build_topology(
    scene: &HairSceneV1,
    registry: &HairGroomRegistryV1,
    poses: Option<&HairSkinPoseRegistryV1>,
) -> EngineResult<HairCpuTopology> {
    let mut points = Vec::new();
    let mut strands = Vec::new();
    let mut render_segments = Vec::new();
    let mut capsules = Vec::new();
    let mut instance_ranges = Vec::with_capacity(scene.instances.len());
    let mut rendered_strand_count = 0usize;
    let mut palette_offset = 0usize;

    for (instance_index, instance) in scene.instances.iter().enumerate() {
        let groom = registry.get(&instance.groom).ok_or_else(|| {
            EngineError::other(format!(
                "hair groom '{}' missing while building GPU topology",
                instance.groom.as_str()
            ))
        })?;
        let pose = instance
            .skin_pose_id
            .map(|pose_id| {
                poses
                    .and_then(|registry| registry.get(pose_id))
                    .ok_or_else(|| {
                        EngineError::other(format!(
                            "hair skin pose {} missing while building groom '{}'",
                            pose_id,
                            instance.groom.as_str()
                        ))
                    })
            })
            .transpose()?;
        let palette_count = pose.map(|pose| pose.joint_deforms.len()).unwrap_or(0);
        if palette_offset.saturating_add(palette_count) > HAIR_SKIN_MATRIX_CAPACITY {
            return Err(EngineError::other(format!(
                "hair skin matrices exceed GPU capacity {}",
                HAIR_SKIN_MATRIX_CAPACITY
            )));
        }
        let capsule_offset = capsules.len();
        append_groom_topology(
            &mut points,
            &mut strands,
            &mut render_segments,
            &mut capsules,
            &mut rendered_strand_count,
            instance_index,
            instance.root_transform,
            instance.quality,
            groom,
            pose,
        )?;
        let capsule_count = capsules.len().saturating_sub(capsule_offset);
        instance_ranges.push(HairInstanceGpuRanges {
            palette_offset,
            palette_count,
            capsule_offset,
            capsule_count,
        });
        palette_offset = palette_offset.saturating_add(palette_count);
    }

    if points.len() > HAIR_POINT_CAPACITY {
        return Err(EngineError::other(format!(
            "hair guide point count {} exceeds GPU capacity {}",
            points.len(),
            HAIR_POINT_CAPACITY
        )));
    }
    if strands.len() > HAIR_STRAND_CAPACITY {
        return Err(EngineError::other(format!(
            "hair guide strand count {} exceeds GPU capacity {}",
            strands.len(),
            HAIR_STRAND_CAPACITY
        )));
    }
    if render_segments.len() > HAIR_RENDER_SEGMENT_CAPACITY {
        return Err(EngineError::other(format!(
            "hair render segment count {} exceeds GPU capacity {}",
            render_segments.len(),
            HAIR_RENDER_SEGMENT_CAPACITY
        )));
    }
    if capsules.len() > HAIR_COLLISION_CAPACITY {
        return Err(EngineError::other(format!(
            "hair collision capsule count {} exceeds GPU capacity {}",
            capsules.len(),
            HAIR_COLLISION_CAPACITY
        )));
    }

    Ok(HairCpuTopology {
        counts: HairTopologyCounts {
            point_count: points.len(),
            strand_count: strands.len(),
            render_segment_count: render_segments.len(),
            rendered_strand_count,
        },
        points,
        strands,
        render_segments,
        capsules,
        instance_ranges,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_groom_topology(
    points: &mut Vec<HairSlot>,
    strands: &mut Vec<HairSlot>,
    render_segments: &mut Vec<HairSlot>,
    capsules: &mut Vec<HairSlot>,
    rendered_strand_count: &mut usize,
    instance_index: usize,
    root_transform: [f32; 16],
    quality: HairQualityTier,
    groom: &HairGroomAssetV1,
    pose: Option<&newengine_render_api::HairSkinPoseV1>,
) -> EngineResult<()> {
    let point_base = points.len();
    let model = Mat4::from_cols_array(&root_transform);
    let mut point_joint = vec![0u16; groom.guide_points.len()];
    for strand in &groom.guide_strands {
        if let Some(pose) = pose {
            if usize::from(strand.root_joint_index) >= pose.joint_deforms.len() {
                return Err(EngineError::other(format!(
                    "hair groom '{}' root joint {} outside skin pose {} joints",
                    groom.groom.as_str(),
                    strand.root_joint_index,
                    pose.joint_deforms.len()
                )));
            }
        }
        for point_index in strand.point_range() {
            point_joint[point_index] = strand.root_joint_index;
        }
    }

    let world_positions = groom
        .guide_points
        .iter()
        .enumerate()
        .map(|(point_index, point)| {
            let rest_model = Vec3::new(
                point.rest_position[0],
                point.rest_position[1],
                point.rest_position[2],
            );
            let skinned_model = pose
                .map(|pose| {
                    Mat4::from_cols_array(
                        &pose.joint_deforms[usize::from(point_joint[point_index])],
                    )
                    .transform_point3(rest_model)
                })
                .unwrap_or(rest_model);
            model.transform_point3(skinned_model)
        })
        .collect::<Vec<_>>();

    let mut parent_indices = (0..groom.guide_points.len()).collect::<Vec<_>>();
    let mut root_flags = vec![1.0f32; groom.guide_points.len()];
    let mut rest_lengths = vec![0.0f32; groom.guide_points.len()];
    let mut strand_t = vec![0.0f32; groom.guide_points.len()];

    for strand in &groom.guide_strands {
        let range = strand.point_range();
        for local in 0..range.len() {
            let point = range.start + local;
            root_flags[point] = f32::from(local == 0);
            parent_indices[point] = if local == 0 { point } else { point - 1 };
            strand_t[point] = local as f32 / (range.len().saturating_sub(1).max(1)) as f32;
            if local > 0 {
                rest_lengths[point] =
                    (world_positions[point] - world_positions[point - 1]).length();
            }
        }
    }

    for (local_index, point) in groom.guide_points.iter().enumerate() {
        let world = world_positions[local_index];
        points.push(HairSlot::from_lanes(
            [world.x, world.y, world.z, point.inverse_mass],
            [
                world.x,
                world.y,
                world.z,
                rest_lengths[local_index].max(1.0e-6),
            ],
            [
                point.rest_position[0],
                point.rest_position[1],
                point.rest_position[2],
                (point_base + parent_indices[local_index]) as f32,
            ],
            [
                root_flags[local_index],
                strand_t[local_index],
                point_joint[local_index] as f32,
                0.0,
            ],
        ));
    }

    let followers = quality_followers(groom.follow_strands_per_guide, quality);
    for strand in &groom.guide_strands {
        let first_point = point_base + strand.first_point as usize;
        let point_count = usize::from(strand.point_count);
        strands.push(HairSlot::from_lanes(
            [
                first_point as f32,
                point_count as f32,
                instance_index as f32,
                strand.group as f32,
            ],
            [
                strand.root_uv[0],
                strand.root_uv[1],
                followers as f32,
                strand.root_joint_index as f32,
            ],
            [0.0; 4],
            [0.0; 4],
        ));
        *rendered_strand_count = rendered_strand_count.saturating_add(1 + followers);

        for local_segment in 0..point_count.saturating_sub(1) {
            let a = first_point + local_segment;
            let b = a + 1;
            let t0 = local_segment as f32 / (point_count.saturating_sub(1).max(1)) as f32;
            let t1 = (local_segment + 1) as f32 / (point_count.saturating_sub(1).max(1)) as f32;
            for follower in 0..=followers {
                let follower_angle = if follower == 0 {
                    0.0
                } else {
                    deterministic_angle(first_point as u64, follower as u64)
                };
                let follower_radius_scale = if follower == 0 {
                    0.0
                } else {
                    8.0 + 4.0 * (follower as f32).sqrt()
                };
                render_segments.push(HairSlot::from_lanes(
                    [a as f32, b as f32, instance_index as f32, t0],
                    [t1, follower_angle, follower_radius_scale, follower as f32],
                    [0.0; 4],
                    [0.0; 4],
                ));
                if render_segments.len() > HAIR_RENDER_SEGMENT_CAPACITY {
                    return Err(EngineError::other(format!(
                        "hair groom '{}' expands beyond render segment capacity {} (reduce followers or guide density)",
                        groom.groom.as_str(),
                        HAIR_RENDER_SEGMENT_CAPACITY
                    )));
                }
            }
        }
    }

    for capsule in &groom.collision_capsules {
        if let Some(pose) = pose {
            if usize::from(capsule.joint_index) >= pose.joint_deforms.len() {
                return Err(EngineError::other(format!(
                    "hair groom '{}' capsule joint {} outside skin pose {} joints",
                    groom.groom.as_str(),
                    capsule.joint_index,
                    pose.joint_deforms.len()
                )));
            }
        }
        capsules.push(HairSlot::from_lanes(
            [
                capsule.local_a[0],
                capsule.local_a[1],
                capsule.local_a[2],
                capsule.radius,
            ],
            [
                capsule.local_b[0],
                capsule.local_b[1],
                capsule.local_b[2],
                capsule.joint_index as f32,
            ],
            [0.0; 4],
            [0.0; 4],
        ));
    }
    Ok(())
}

pub(super) fn build_skin_palette_slots(
    scene: &HairSceneV1,
    poses: Option<&HairSkinPoseRegistryV1>,
    ranges: &[HairInstanceGpuRanges],
) -> EngineResult<Vec<HairSlot>> {
    if scene.instances.len() != ranges.len() {
        return Err(EngineError::other(
            "hair instance/palette range count mismatch",
        ));
    }
    let total = ranges
        .last()
        .map(|range| range.palette_offset.saturating_add(range.palette_count))
        .unwrap_or(0);
    if total > HAIR_SKIN_MATRIX_CAPACITY {
        return Err(EngineError::other(
            "hair palette layout exceeds GPU capacity",
        ));
    }
    let mut slots = Vec::with_capacity(total);
    for (instance, range) in scene.instances.iter().zip(ranges) {
        let Some(pose_id) = instance.skin_pose_id else {
            if range.palette_count != 0 {
                return Err(EngineError::other(
                    "rigid hair instance has non-empty palette range",
                ));
            }
            continue;
        };
        let pose = poses
            .and_then(|registry| registry.get(pose_id))
            .ok_or_else(|| {
                EngineError::other(format!("hair skin pose {pose_id} is not resident"))
            })?;
        if pose.joint_deforms.len() != range.palette_count {
            return Err(EngineError::other(format!(
                "hair skin pose {} joint count changed {} -> {} without topology rebuild",
                pose_id,
                range.palette_count,
                pose.joint_deforms.len()
            )));
        }
        if slots.len() != range.palette_offset {
            return Err(EngineError::other(
                "hair palette ranges are not tightly packed",
            ));
        }
        slots.extend(
            pose.joint_deforms
                .iter()
                .copied()
                .map(HairSlot::from_matrix),
        );
    }
    Ok(slots)
}

pub(super) fn build_instance_slots(
    scene: &HairSceneV1,
    ranges: &[HairInstanceGpuRanges],
) -> Vec<HairSlot> {
    let mut slots = Vec::with_capacity(scene.instances.len() * HAIR_INSTANCE_SLOT_COUNT);
    for (instance_index, instance) in scene.instances.iter().enumerate() {
        let range = ranges.get(instance_index).copied().unwrap_or_default();
        slots.push(HairSlot::from_matrix(instance.root_transform));
        let simulation_mode = match instance.simulation.mode {
            HairSimulationMode::Disabled => 0.0,
            HairSimulationMode::GuideStrands => 1.0,
        };
        let collision_mode = match instance.simulation.collision {
            HairCollisionMode::None => 0.0,
            HairCollisionMode::Capsules => 1.0,
            HairCollisionMode::Sdf => 2.0,
        };
        slots.push(HairSlot::from_lanes(
            [
                instance.simulation.gravity_scale,
                instance.simulation.damping,
                instance.simulation.stretch_stiffness,
                instance.simulation.bend_stiffness,
            ],
            [
                instance.simulation.root_stiffness,
                instance.simulation.wind_response,
                instance.simulation.max_delta_seconds,
                instance.lod.simulation_distance,
            ],
            [
                instance.wind_velocity[0],
                instance.wind_velocity[1],
                instance.wind_velocity[2],
                instance.material.strand_width_mm * 0.001,
            ],
            [
                instance.material.base_color[0],
                instance.material.base_color[1],
                instance.material.base_color[2],
                instance.material.opacity,
            ],
        ));
        slots.push(HairSlot::from_lanes(
            [
                instance.material.roughness,
                instance.material.secondary_specular,
                instance.material.melanin,
                instance.material.redness,
            ],
            [
                instance.material.tip_scale,
                instance.lod.density_start_distance,
                instance.lod.density_end_distance,
                instance.lod.minimum_density,
            ],
            [
                simulation_mode,
                collision_mode,
                instance.simulation.solver_iterations as f32,
                quality_code(instance.quality),
            ],
            [
                f32::from(instance.casts_shadows),
                f32::from(instance.receives_shadows),
                0.0,
                0.0,
            ],
        ));
        slots.push(HairSlot::from_lanes(
            [
                (SKIN_MATRIX_BASE + range.palette_offset) as f32,
                range.palette_count as f32,
                (CAPSULE_BASE + range.capsule_offset) as f32,
                range.capsule_count as f32,
            ],
            [0.0; 4],
            [0.0; 4],
            [0.0; 4],
        ));
    }
    slots
}

#[allow(clippy::too_many_arguments)]
pub(super) fn encode_frame_ubo(
    view_projection: Mat4,
    camera_position: Vec3,
    camera_right: Vec3,
    camera_up: Vec3,
    dt: f32,
    directional_dir_intensity: [f32; 4],
    directional_color: [f32; 4],
    ambient_color: [f32; 4],
    counts: HairTopologyCounts,
    read_point_base: usize,
    write_point_base: usize,
    camera_forward: Vec3,
    shadow_frame: ShadowFrame,
    shadow_extent: Extent2D,
    shadow_enabled: bool,
) -> [u8; HAIR_FRAME_UBO_BYTES as usize] {
    let mut values = [0.0f32; HAIR_FRAME_UBO_BYTES as usize / 4];
    values[0..16].copy_from_slice(&view_projection.to_cols_array());
    values[16..20].copy_from_slice(&[camera_position.x, camera_position.y, camera_position.z, dt]);
    values[20..24].copy_from_slice(&[camera_right.x, camera_right.y, camera_right.z, 0.0]);
    values[24..28].copy_from_slice(&[camera_up.x, camera_up.y, camera_up.z, 0.0]);
    values[28..32].copy_from_slice(&directional_dir_intensity);
    values[32..36].copy_from_slice(&directional_color);
    values[36..40].copy_from_slice(&ambient_color);
    values[40..44].copy_from_slice(&[
        counts.point_count as f32,
        counts.strand_count as f32,
        counts.render_segment_count as f32,
        counts.rendered_strand_count as f32,
    ]);
    values[44..48].copy_from_slice(&[
        read_point_base as f32,
        write_point_base as f32,
        STRAND_BASE as f32,
        SEGMENT_BASE as f32,
    ]);
    values[48..52].copy_from_slice(&[
        INSTANCE_BASE as f32,
        HAIR_INSTANCE_SLOT_COUNT as f32,
        HAIR_SLOT_CAPACITY as f32,
        0.0,
    ]);

    let cascade_count = shadow_frame
        .cascade_count
        .clamp(1, MAX_DIRECTIONAL_SHADOW_CASCADES as u32) as usize;
    let shadows_active = shadow_enabled && shadow_frame.params[0] >= 0.5;
    values[52..56].copy_from_slice(&[
        f32::from(shadows_active),
        cascade_count as f32,
        shadow_frame.params[2].clamp(0.0, 1.0),
        shadow_frame.params[1].max(0.0),
    ]);
    let atlas_w = shadow_extent.width.max(1) as f32;
    let atlas_h = shadow_extent.height.max(1) as f32;
    values[56..60].copy_from_slice(&[atlas_w, atlas_h, 1.0 / atlas_w, 1.0 / atlas_h]);
    values[60..64].copy_from_slice(&shadow_frame.cascade_splits);

    let mut biases = [0.0f32; MAX_DIRECTIONAL_SHADOW_CASCADES];
    for (index, bias) in biases.iter_mut().enumerate().take(cascade_count) {
        *bias = hair_shadow_receiver_bias(shadow_frame, index, shadow_extent);
    }
    values[64..68].copy_from_slice(&biases);

    let forward = camera_forward.normalize_or_zero();
    values[68..72].copy_from_slice(&[forward.x, forward.y, forward.z, 0.0]);
    let mut matrix_offset = 72usize;
    for cascade_index in 0..MAX_DIRECTIONAL_SHADOW_CASCADES {
        values[matrix_offset..matrix_offset + 16].copy_from_slice(
            &shadow_frame
                .cascade(cascade_index)
                .light_mvp
                .to_cols_array(),
        );
        matrix_offset += 16;
    }
    debug_assert_eq!(matrix_offset, 136);
    f32_array_bytes(values)
}

pub(super) fn hair_shadow_receiver_bias(
    shadow_frame: ShadowFrame,
    cascade_index: usize,
    shadow_extent: Extent2D,
) -> f32 {
    let cascade = shadow_frame.cascade(cascade_index);
    let matrix = cascade.light_mvp.to_cols_array();
    // Column-major Mat4 -> geometric rows used to convert normalized depth to world/texel scale.
    let row_x = Vec3::new(matrix[0], matrix[4], matrix[8]);
    let row_y = Vec3::new(matrix[1], matrix[5], matrix[9]);
    let row_z = Vec3::new(matrix[2], matrix[6], matrix[10]);
    let tile_width = if shadow_frame.cascade_count > 1 {
        cascade.viewport.w.max(1.0)
    } else {
        shadow_extent.width.max(1) as f32
    };
    let tile_height = if shadow_frame.cascade_count > 1 {
        cascade.viewport.h.max(1.0)
    } else {
        shadow_extent.height.max(1) as f32
    };
    let world_texel_x = 2.0 / (row_x.length() * tile_width).max(1.0e-6);
    let world_texel_y = 2.0 / (row_y.length() * tile_height).max(1.0e-6);
    let world_texel = world_texel_x.max(world_texel_y);
    let depth_per_texel = (world_texel * row_z.length().max(1.0e-6)).max(1.0e-7);
    let authored_strength = if shadow_frame.params[1] > 0.0 {
        (shadow_frame.params[1] / 0.0025).clamp(0.25, 6.0)
    } else {
        1.0
    };
    (depth_per_texel * 0.65 * authored_strength).clamp(0.000002, 0.002)
}

pub(super) fn encode_shadow_ubo(
    light_view_projection: Mat4,
    directional_dir_intensity: [f32; 4],
    render_segment_count: usize,
    point_base: usize,
    cascade_index: usize,
) -> [u8; HAIR_SHADOW_UBO_BYTES as usize] {
    let mut values = [0.0f32; HAIR_SHADOW_UBO_BYTES as usize / 4];
    values[0..16].copy_from_slice(&light_view_projection.to_cols_array());
    values[16..20].copy_from_slice(&[
        directional_dir_intensity[0],
        directional_dir_intensity[1],
        directional_dir_intensity[2],
        0.0,
    ]);
    values[20..24].copy_from_slice(&[
        render_segment_count as f32,
        point_base as f32,
        SEGMENT_BASE as f32,
        INSTANCE_BASE as f32,
    ]);
    values[24..28].copy_from_slice(&[
        HAIR_INSTANCE_SLOT_COUNT as f32,
        cascade_index as f32,
        0.0,
        0.0,
    ]);
    f32_array_bytes(values)
}

pub(super) fn slots_to_bytes(slots: &[HairSlot]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(slots.len() * HAIR_SLOT_BYTES);
    for slot in slots {
        for value in slot.0 {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
    }
    bytes
}

fn f32_array_bytes<const N: usize, const B: usize>(values: [f32; N]) -> [u8; B] {
    debug_assert_eq!(B, N * 4);
    let mut out = [0u8; B];
    for (index, value) in values.into_iter().enumerate() {
        let offset = index * 4;
        out[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }
    out
}

pub(super) fn topology_key(
    scene: &HairSceneV1,
    registry_generation: u64,
    pose_layout_generation: u64,
    poses: Option<&HairSkinPoseRegistryV1>,
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash = fnv_u64(hash, registry_generation);
    hash = fnv_u64(hash, pose_layout_generation);
    for instance in &scene.instances {
        hash = fnv_u64(hash, instance.instance_id);
        for byte in instance.groom.as_str().as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash = fnv_u64(hash, quality_code(instance.quality) as u64);
        let pose_id = instance.skin_pose_id.unwrap_or(0);
        hash = fnv_u64(hash, pose_id);
        let joint_count = instance
            .skin_pose_id
            .and_then(|pose_id| poses.and_then(|registry| registry.get(pose_id)))
            .map(|pose| pose.joint_deforms.len() as u64)
            .unwrap_or(0);
        hash = fnv_u64(hash, joint_count);
    }
    hash
}

pub(super) fn shader_set_key(shaders: &HairShaderSetV1) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for value in [
        Some(shaders.simulation.as_str()),
        Some(shaders.strands_vertex.as_str()),
        Some(shaders.strands_fragment.as_str()),
        shaders.shadow_vertex.as_deref(),
        shaders.shadow_fragment.as_deref(),
    ] {
        match value {
            Some(value) => {
                hash = fnv_u64(hash, 1);
                for byte in value.as_bytes() {
                    hash ^= u64::from(*byte);
                    hash = hash.wrapping_mul(0x100000001b3);
                }
            }
            None => hash = fnv_u64(hash, 0),
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[inline]
pub(super) fn fnv_u64(mut hash: u64, value: u64) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[inline]
pub(super) fn deterministic_angle(seed: u64, follower: u64) -> f32 {
    let mixed = seed
        .wrapping_mul(0x9e3779b97f4a7c15)
        .wrapping_add(follower.wrapping_mul(0xbf58476d1ce4e5b9));
    let unit = ((mixed >> 40) & 0x00ff_ffff) as f32 / 16_777_215.0;
    unit * std::f32::consts::TAU
}

#[inline]
pub(super) fn sanitize_dt(dt: f32) -> f32 {
    if dt.is_finite() {
        dt.clamp(0.0, 0.1)
    } else {
        0.0
    }
}

#[inline]
pub(super) fn quality_followers(authored: u8, quality: HairQualityTier) -> usize {
    let authored = usize::from(authored);
    match quality {
        HairQualityTier::Off => 0,
        HairQualityTier::Low => authored.min(1),
        HairQualityTier::Medium => authored.min(3),
        HairQualityTier::High => authored.min(7),
        HairQualityTier::Ultra => authored.min(16),
    }
}

#[inline]
pub(super) fn quality_code(quality: HairQualityTier) -> f32 {
    match quality {
        HairQualityTier::Off => 0.0,
        HairQualityTier::Low => 1.0,
        HairQualityTier::Medium => 2.0,
        HairQualityTier::High => 3.0,
        HairQualityTier::Ultra => 4.0,
    }
}
