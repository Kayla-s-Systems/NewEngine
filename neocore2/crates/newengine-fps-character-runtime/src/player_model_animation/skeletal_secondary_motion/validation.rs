fn validate_skeletal_secondary_motion(
    authored: &PlayerSkeletalSecondaryMotionRig,
) -> Result<(), String> {
    if authored.chain_joints.len() < 2 {
        return Err("skeletal secondary-motion requires at least two chain joints".to_owned());
    }
    if authored.dynamic_start == 0 || authored.dynamic_start >= authored.chain_joints.len() {
        return Err(format!(
            "skeletal secondary-motion dynamic_start={} outside movable chain range 1..{}",
            authored.dynamic_start,
            authored.chain_joints.len()
        ));
    }
    if authored.particles.is_empty() {
        return Err("skeletal secondary-motion requires particles".to_owned());
    }
    if authored.centerline_pairs.len() < 2 {
        return Err("skeletal secondary-motion requires at least two centerline pairs".to_owned());
    }
    if authored.tuning.solver_substeps == 0 || authored.tuning.solver_iterations == 0 {
        return Err(
            "skeletal secondary-motion solver substeps/iterations must be non-zero".to_owned(),
        );
    }
    if authored.tuning.stretch_reference_stiffness <= 0.0
        || authored.tuning.bend_reference_stiffness <= 0.0
    {
        return Err(
            "skeletal secondary-motion reference stiffness values must be positive".to_owned(),
        );
    }

    let particle_count = authored.particles.len();
    for (index, particle) in authored.particles.iter().enumerate() {
        if particle
            .rest_position
            .iter()
            .chain([&particle.mobility, &particle.follow, &particle.inertia])
            .any(|value| !value.is_finite())
        {
            return Err(format!(
                "skeletal secondary-motion particle[{index}] contains non-finite data"
            ));
        }
    }
    for (index, edge) in authored.edges.iter().enumerate() {
        if edge.a >= particle_count || edge.b >= particle_count {
            return Err(format!(
                "skeletal secondary-motion edge[{index}] index outside particles a={} b={} particles={particle_count}",
                edge.a, edge.b
            ));
        }
        if edge.rest_length <= 0.0
            || !edge.rest_length.is_finite()
            || !edge.stiffness.is_finite()
            || !edge.damping.is_finite()
        {
            return Err(format!(
                "skeletal secondary-motion edge[{index}] contains invalid scalar data"
            ));
        }
    }
    for (index, bend) in authored.bends.iter().enumerate() {
        if bend
            .indices
            .iter()
            .any(|&particle| particle >= particle_count)
        {
            return Err(format!(
                "skeletal secondary-motion bend[{index}] index outside particles"
            ));
        }
        if bend.weights.iter().any(|value| !value.is_finite())
            || !bend.geometry_scale.is_finite()
            || !bend.rest_scalar.is_finite()
        {
            return Err(format!(
                "skeletal secondary-motion bend[{index}] contains non-finite data"
            ));
        }
    }
    for (index, pair) in authored.centerline_pairs.iter().enumerate() {
        if pair.iter().any(|&particle| particle >= particle_count) {
            return Err(format!(
                "skeletal secondary-motion centerline pair[{index}] index outside particles"
            ));
        }
    }
    for (index, capsule) in authored.collision_capsules.iter().enumerate() {
        if capsule.joint.trim().is_empty()
            || capsule.radius <= 0.0
            || !capsule.radius.is_finite()
            || capsule
                .source_a
                .iter()
                .chain(capsule.source_b.iter())
                .any(|value| !value.is_finite())
        {
            return Err(format!(
                "skeletal secondary-motion capsule[{index}] contains invalid data"
            ));
        }
    }
    for (index, box_shape) in authored.collision_boxes.iter().enumerate() {
        if box_shape.joint.trim().is_empty()
            || box_shape
                .half_extents
                .iter()
                .any(|value| *value <= 0.0 || !value.is_finite())
            || box_shape
                .source_center
                .iter()
                .chain(box_shape.source_axes.iter().flatten())
                .any(|value| !value.is_finite())
        {
            return Err(format!(
                "skeletal secondary-motion oriented_box[{index}] contains invalid data"
            ));
        }
    }
    let tuning_values = [
        authored.tuning.teleport_reset_distance,
        authored.tuning.teleport_reset_quat_dot,
        authored.tuning.back_clearance,
        authored.tuning.max_root_acceleration,
        authored.tuning.gravity_scale,
        authored.tuning.inertia_scale,
        authored.tuning.velocity_damping,
        authored.tuning.collision_margin,
        authored.tuning.follow_scale,
        authored.tuning.stretch_reference_stiffness,
        authored.tuning.bend_reference_stiffness,
        authored.tuning.tunnel_depth,
    ];
    if tuning_values.iter().any(|value| !value.is_finite()) {
        return Err("skeletal secondary-motion tuning contains non-finite data".to_owned());
    }
    if !(0.0..=1.0).contains(&authored.tuning.teleport_reset_quat_dot) {
        return Err(
            "skeletal secondary-motion teleport_reset_quat_dot must be in [0,1]".to_owned(),
        );
    }
    Ok(())
}

fn prepare_skeletal_secondary_motion(
    parts: &[PlayerRuntimeModelPart],
    skeleton: &ModelSkeletonMetadata,
    authored: Option<&PlayerSkeletalSecondaryMotionRig>,
    source_to_model: [f32; 16],
    bind_joint_frames: &[Mat4],
) -> Result<Option<SkeletalSecondaryMotionRuntime>, String> {
    let Some(authored) = authored else {
        return Ok(None);
    };
    validate_skeletal_secondary_motion(authored)?;

    let chain_joints = authored
        .chain_joints
        .iter()
        .enumerate()
        .map(|(lane, name)| {
            secondary_motion_joint(skeleton, name.trim()).ok_or_else(|| {
                format!(
                    "skeletal secondary-motion authored chain is partial lane={lane}: missing joint '{name}'"
                )
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let has_simulated_skin = parts
        .iter()
        .filter_map(|part| part.skin.as_ref())
        .any(|skin| {
            skin.vertices.iter().any(|vertex| {
                vertex
                    .joints
                    .iter()
                    .chain(vertex.joints_extra.iter())
                    .zip(vertex.weights.iter().chain(vertex.weights_extra.iter()))
                    .any(|(&joint, &weight)| {
                        weight > 1.0e-5 && chain_joints.contains(&(joint as usize))
                    })
            })
        });
    if !has_simulated_skin {
        return Ok(None);
    }

    let runtime = SkeletalSecondaryMotionRuntime::new(
        authored,
        chain_joints,
        skeleton,
        source_to_model,
        bind_joint_frames,
    )?;
    newengine_ulog_api::ulog::info!(
        "skeletal secondary motion ready joints={} particles={} edges={} bends={} capsules={} boxes={} source='project-authored definition'",
        authored.chain_joints.len(),
        authored.particles.len(),
        authored.edges.len(),
        authored.bends.len(),
        authored.collision_capsules.len(),
        authored.collision_boxes.len(),
    );
    Ok(Some(runtime))
}

