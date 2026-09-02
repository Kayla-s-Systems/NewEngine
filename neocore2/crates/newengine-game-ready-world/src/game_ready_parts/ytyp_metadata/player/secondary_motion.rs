fn secondary_motion_values<'a>(
    value: &'a serde_json::Value,
    key: &str,
) -> Vec<&'a serde_json::Value> {
    let Some(value) = value.get(key) else {
        return Vec::new();
    };
    match value {
        serde_json::Value::Array(values) => values.iter().collect(),
        other => vec![other],
    }
}

fn secondary_motion_usize(value: &serde_json::Value) -> Option<usize> {
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .or_else(|| value.as_i64().and_then(|value| usize::try_from(value).ok()))
        .or_else(|| {
            value
                .as_str()
                .and_then(|value| value.trim().parse::<usize>().ok())
        })
}

fn secondary_motion_u8(value: &serde_json::Value) -> Option<u8> {
    secondary_motion_usize(value).and_then(|value| u8::try_from(value).ok())
}

fn secondary_motion_f32_array<const N: usize>(value: &serde_json::Value) -> Option<[f32; N]> {
    let values = value.as_array()?;
    if values.len() != N {
        return None;
    }
    let mut out = [0.0_f32; N];
    for (index, value) in values.iter().enumerate() {
        out[index] = value_f32(value)?;
    }
    Some(out)
}

fn secondary_motion_usize_array<const N: usize>(value: &serde_json::Value) -> Option<[usize; N]> {
    let values = value.as_array()?;
    if values.len() != N {
        return None;
    }
    let mut out = [0usize; N];
    for (index, value) in values.iter().enumerate() {
        out[index] = secondary_motion_usize(value)?;
    }
    Some(out)
}

fn required_secondary_motion_f32(value: &serde_json::Value, key: &str) -> Result<f32, String> {
    value
        .get(key)
        .and_then(value_f32)
        .ok_or_else(|| format!("skeletal_secondary_motion requires finite '{key}'"))
}

fn required_secondary_motion_usize(value: &serde_json::Value, key: &str) -> Result<usize, String> {
    value
        .get(key)
        .and_then(secondary_motion_usize)
        .ok_or_else(|| format!("skeletal_secondary_motion requires non-negative integer '{key}'"))
}

fn secondary_motion_collider_mode(
    value: &serde_json::Value,
) -> Result<newengine_engine_runtime::gameplay::PlayerSecondaryMotionColliderMode, String> {
    match value_string(value)
        .unwrap_or_else(|| "exterior".to_owned())
        .to_ascii_lowercase()
        .replace(['-', ' '], "_")
        .as_str()
    {
        "exterior" | "outside" => {
            Ok(newengine_engine_runtime::gameplay::PlayerSecondaryMotionColliderMode::Exterior)
        }
        "one_sided_back" | "back" => {
            Ok(newengine_engine_runtime::gameplay::PlayerSecondaryMotionColliderMode::OneSidedBack)
        }
        other => Err(format!(
            "skeletal_secondary_motion unsupported collider mode '{other}'"
        )),
    }
}

fn parse_skeletal_secondary_motion(
    model: &serde_json::Value,
) -> Result<Option<newengine_engine_runtime::gameplay::PlayerSkeletalSecondaryMotionRig>, String> {
    use newengine_engine_runtime::gameplay::{
        PlayerSecondaryMotionBend, PlayerSecondaryMotionCapsule, PlayerSecondaryMotionEdge,
        PlayerSecondaryMotionOrientedBox, PlayerSecondaryMotionParticle,
        PlayerSecondaryMotionTuning, PlayerSkeletalSecondaryMotionRig,
    };

    let Some(root) = value_path(model, &["skeletal_secondary_motion"]) else {
        return Ok(None);
    };
    let chain = value_path(root, &["chain"])
        .map(|chain| secondary_motion_values(chain, "joint"))
        .unwrap_or_default()
        .into_iter()
        .filter_map(value_string)
        .collect::<Vec<_>>();
    if chain.len() < 2 {
        return Err(
            "skeletal_secondary_motion requires at least two authored chain joints".to_owned(),
        );
    }

    let particles_node = value_path(root, &["particles"])
        .ok_or_else(|| "skeletal_secondary_motion requires particles".to_owned())?;
    let particles = secondary_motion_values(particles_node, "particle")
        .into_iter()
        .enumerate()
        .map(|(index, particle)| {
            Ok(PlayerSecondaryMotionParticle {
                rest_position: particle
                    .get("rest_position")
                    .and_then(secondary_motion_f32_array::<3>)
                    .ok_or_else(|| {
                        format!(
                            "skeletal_secondary_motion particle[{index}] requires rest_position[3]"
                        )
                    })?,
                mobility: required_secondary_motion_f32(particle, "mobility")?,
                follow: required_secondary_motion_f32(particle, "follow")?,
                inertia: required_secondary_motion_f32(particle, "inertia")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    if particles.is_empty() {
        return Err("skeletal_secondary_motion requires at least one particle".to_owned());
    }

    let edges_node = value_path(root, &["edges"])
        .ok_or_else(|| "skeletal_secondary_motion requires edges".to_owned())?;
    let edges = secondary_motion_values(edges_node, "edge")
        .into_iter()
        .enumerate()
        .map(|(index, edge)| {
            Ok(PlayerSecondaryMotionEdge {
                a: required_secondary_motion_usize(edge, "a")?,
                b: required_secondary_motion_usize(edge, "b")?,
                rest_length: required_secondary_motion_f32(edge, "rest_length")?,
                stiffness: required_secondary_motion_f32(edge, "stiffness")?,
                damping: required_secondary_motion_f32(edge, "damping")?,
            })
            .map_err(|error: String| format!("edge[{index}]: {error}"))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let bends_node = value_path(root, &["bends"])
        .ok_or_else(|| "skeletal_secondary_motion requires bends".to_owned())?;
    let bends = secondary_motion_values(bends_node, "bend")
        .into_iter()
        .enumerate()
        .map(|(index, bend)| {
            Ok(PlayerSecondaryMotionBend {
                indices: bend
                    .get("indices")
                    .and_then(secondary_motion_usize_array::<4>)
                    .ok_or_else(|| format!("bend[{index}] requires indices[4]"))?,
                weights: bend
                    .get("weights")
                    .and_then(secondary_motion_f32_array::<4>)
                    .ok_or_else(|| format!("bend[{index}] requires weights[4]"))?,
                geometry_scale: required_secondary_motion_f32(bend, "geometry_scale")?,
                rest_scalar: required_secondary_motion_f32(bend, "rest_scalar")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let centerline_node = value_path(root, &["centerline"])
        .ok_or_else(|| "skeletal_secondary_motion requires centerline".to_owned())?;
    let centerline_pairs = secondary_motion_values(centerline_node, "pair")
        .into_iter()
        .enumerate()
        .map(|(index, pair)| {
            pair.get("indices")
                .and_then(secondary_motion_usize_array::<2>)
                .ok_or_else(|| format!("centerline pair[{index}] requires indices[2]"))
        })
        .collect::<Result<Vec<_>, String>>()?;
    if centerline_pairs.len() < 2 {
        return Err("skeletal_secondary_motion requires at least two centerline pairs".to_owned());
    }

    let colliders = value_path(root, &["colliders"])
        .ok_or_else(|| "skeletal_secondary_motion requires colliders".to_owned())?;
    let collision_capsules = secondary_motion_values(colliders, "capsule")
        .into_iter()
        .enumerate()
        .map(|(index, capsule)| {
            Ok(PlayerSecondaryMotionCapsule {
                joint: capsule
                    .get("joint")
                    .and_then(value_string)
                    .ok_or_else(|| format!("capsule[{index}] requires joint"))?,
                mode: capsule
                    .get("mode")
                    .map(secondary_motion_collider_mode)
                    .transpose()?
                    .unwrap_or_default(),
                source_a: capsule
                    .get("source_a")
                    .and_then(secondary_motion_f32_array::<3>)
                    .ok_or_else(|| format!("capsule[{index}] requires source_a[3]"))?,
                source_b: capsule
                    .get("source_b")
                    .and_then(secondary_motion_f32_array::<3>)
                    .ok_or_else(|| format!("capsule[{index}] requires source_b[3]"))?,
                radius: required_secondary_motion_f32(capsule, "radius")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let collision_boxes = secondary_motion_values(colliders, "oriented_box")
        .into_iter()
        .enumerate()
        .map(|(index, box_shape)| {
            let source_axes = [
                box_shape
                    .get("source_axis_x")
                    .and_then(secondary_motion_f32_array::<3>)
                    .ok_or_else(|| format!("oriented_box[{index}] requires source_axis_x[3]"))?,
                box_shape
                    .get("source_axis_y")
                    .and_then(secondary_motion_f32_array::<3>)
                    .ok_or_else(|| format!("oriented_box[{index}] requires source_axis_y[3]"))?,
                box_shape
                    .get("source_axis_z")
                    .and_then(secondary_motion_f32_array::<3>)
                    .ok_or_else(|| format!("oriented_box[{index}] requires source_axis_z[3]"))?,
            ];
            Ok(PlayerSecondaryMotionOrientedBox {
                joint: box_shape
                    .get("joint")
                    .and_then(value_string)
                    .ok_or_else(|| format!("oriented_box[{index}] requires joint"))?,
                mode: box_shape
                    .get("mode")
                    .map(secondary_motion_collider_mode)
                    .transpose()?
                    .unwrap_or_default(),
                source_center: box_shape
                    .get("source_center")
                    .and_then(secondary_motion_f32_array::<3>)
                    .ok_or_else(|| format!("oriented_box[{index}] requires source_center[3]"))?,
                source_axes,
                half_extents: box_shape
                    .get("half_extents")
                    .and_then(secondary_motion_f32_array::<3>)
                    .ok_or_else(|| format!("oriented_box[{index}] requires half_extents[3]"))?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let solver_substeps = root
        .get("solver_substeps")
        .and_then(secondary_motion_u8)
        .ok_or_else(|| "skeletal_secondary_motion requires solver_substeps as u8".to_owned())?;
    let solver_iterations = root
        .get("solver_iterations")
        .and_then(secondary_motion_u8)
        .ok_or_else(|| "skeletal_secondary_motion requires solver_iterations as u8".to_owned())?;

    Ok(Some(PlayerSkeletalSecondaryMotionRig {
        chain_joints: chain,
        dynamic_start: required_secondary_motion_usize(root, "dynamic_start")?,
        particles,
        edges,
        bends,
        centerline_pairs,
        collision_capsules,
        collision_boxes,
        tuning: PlayerSecondaryMotionTuning {
            teleport_reset_distance: required_secondary_motion_f32(
                root,
                "teleport_reset_distance",
            )?,
            teleport_reset_quat_dot: required_secondary_motion_f32(
                root,
                "teleport_reset_quat_dot",
            )?,
            back_clearance: required_secondary_motion_f32(root, "back_clearance")?,
            solver_substeps,
            solver_iterations,
            max_root_acceleration: required_secondary_motion_f32(root, "max_root_acceleration")?,
            gravity_scale: required_secondary_motion_f32(root, "gravity_scale")?,
            inertia_scale: required_secondary_motion_f32(root, "inertia_scale")?,
            velocity_damping: required_secondary_motion_f32(root, "velocity_damping")?,
            collision_margin: required_secondary_motion_f32(root, "collision_margin")?,
            follow_scale: required_secondary_motion_f32(root, "follow_scale")?,
            stretch_reference_stiffness: required_secondary_motion_f32(
                root,
                "stretch_reference_stiffness",
            )?,
            bend_reference_stiffness: required_secondary_motion_f32(
                root,
                "bend_reference_stiffness",
            )?,
            tunnel_depth: required_secondary_motion_f32(root, "tunnel_depth")?,
        },
    }))
}
