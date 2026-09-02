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
