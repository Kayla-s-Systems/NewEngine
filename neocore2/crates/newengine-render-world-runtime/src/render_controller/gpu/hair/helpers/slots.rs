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

