use super::*;

pub(super) fn validate_geometry_sanity(
    meshes: &[crate::geometry::ImportMesh],
) -> Result<(), String> {
    const MAX_CHARACTER_EXTENT: f32 = 100.0;
    for mesh in meshes {
        if mesh.vertices.is_empty() || mesh.indices.is_empty() || mesh.indices.len() % 3 != 0 {
            return Err(format!(
                "invalid runtime geometry mesh='{}' vertices={} indices={}",
                mesh.name,
                mesh.vertices.len(),
                mesh.indices.len()
            ));
        }
        if mesh
            .bounds_min
            .iter()
            .chain(mesh.bounds_max.iter())
            .any(|value| !value.is_finite())
        {
            return Err(format!("non-finite runtime bounds mesh='{}'", mesh.name));
        }
        let extent = [
            mesh.bounds_max[0] - mesh.bounds_min[0],
            mesh.bounds_max[1] - mesh.bounds_min[1],
            mesh.bounds_max[2] - mesh.bounds_min[2],
        ];
        if extent
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0 || *value > MAX_CHARACTER_EXTENT)
        {
            return Err(format!(
                "implausible runtime bounds mesh='{}' min={:?} max={:?}",
                mesh.name, mesh.bounds_min, mesh.bounds_max
            ));
        }
        for (vertex_index, vertex) in mesh.vertices.iter().enumerate() {
            if vertex
                .position
                .iter()
                .chain(vertex.normal.iter())
                .chain(vertex.uv0.iter())
                .any(|value| !value.is_finite())
            {
                return Err(format!(
                    "non-finite runtime vertex mesh='{}' vertex={vertex_index}",
                    mesh.name
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_native_eye_contract(
    meshes: &[crate::geometry::ImportMesh],
    skeleton: &DecodedSkeleton,
) -> Result<(), String> {
    let Some(eye_mesh) = meshes.iter().find(|mesh| {
        let name = mesh.name.to_ascii_lowercase();
        let material = mesh
            .source_material
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        name.contains("abby_eyes_") || material.contains("/abby/abby-eyes:")
    }) else {
        return Ok(());
    };

    let joint_index = |name: &str| {
        skeleton
            .joints
            .iter()
            .position(|joint| joint.name == name)
            .ok_or_else(|| format!("native Abby eye mesh requires skeleton joint '{name}'"))
    };
    let left = joint_index("l_eyeball")?;
    let right = joint_index("r_eyeball")?;
    if skeleton.joints[left].parent_index != skeleton.joints[right].parent_index {
        return Err("native Abby eyeballs do not share the same authored parent".to_owned());
    }
    let parent = skeleton.joints[left]
        .parent_index
        .ok_or_else(|| "native Abby eyeballs have no authored parent".to_owned())?
        as usize;
    let parent_name = skeleton
        .joints
        .get(parent)
        .map(|joint| joint.name.as_str())
        .ok_or_else(|| format!("native Abby eyeball parent outside skeleton index={parent}"))?;
    if parent_name != "headb" {
        return Err(format!(
            "native Abby eyeballs must remain direct children of headb parent={} name='{}'",
            parent, parent_name
        ));
    }

    let mut globals = vec![Mat4::IDENTITY; skeleton.joints.len()];
    for (index, joint) in skeleton.joints.iter().enumerate() {
        let local = Mat4::from_scale_rotation_translation(
            Vec3::new(joint.scale_ls[0], joint.scale_ls[1], joint.scale_ls[2]),
            Quat::from_xyzw(
                joint.rotation_ls[0],
                joint.rotation_ls[1],
                joint.rotation_ls[2],
                joint.rotation_ls[3],
            )
            .normalize_or_identity(),
            Vec3::new(
                joint.position_ls[0],
                joint.position_ls[1],
                joint.position_ls[2],
            ),
        );
        globals[index] = joint
            .parent_index
            .map(|parent| globals[parent as usize] * local)
            .unwrap_or(local);
    }

    let (left_scale, left_rotation, left_center) = globals[left].to_scale_rotation_translation();
    let (right_scale, right_rotation, right_center) =
        globals[right].to_scale_rotation_translation();
    if !left_scale.is_finite()
        || !right_scale.is_finite()
        || !left_rotation.is_finite()
        || !right_rotation.is_finite()
        || !left_center.is_finite()
        || !right_center.is_finite()
    {
        return Err("native Abby eye bind basis contains non-finite values".to_owned());
    }
    let scale_delta_vec = left_scale - right_scale;
    let scale_delta = scale_delta_vec
        .x
        .abs()
        .max(scale_delta_vec.y.abs())
        .max(scale_delta_vec.z.abs());
    let basis_dot = left_rotation
        .normalize_or_identity()
        .dot(right_rotation.normalize_or_identity())
        .abs();
    if scale_delta > 1.0e-4 || basis_dot < 0.9999 {
        return Err(format!(
            "native Abby eye bind bases diverge scale_delta={scale_delta:.8} rotation_dot={basis_dot:.8}"
        ));
    }
    let canonical_basis_dot = left_rotation
        .normalize_or_identity()
        .dot(Quat::IDENTITY)
        .abs()
        .min(
            right_rotation
                .normalize_or_identity()
                .dot(Quat::IDENTITY)
                .abs(),
        );
    if canonical_basis_dot < 0.9999 {
        return Err(format!(
            "native Abby eye global basis no longer matches authored canonical XYZ basis rotation_dot={canonical_basis_dot:.8}"
        ));
    }

    let Some(skin) = eye_mesh.skin.as_deref() else {
        return Err("native Abby eye mesh has no skin stream".to_owned());
    };
    if skin.len() != eye_mesh.vertices.len() {
        return Err(format!(
            "native Abby eye skin/vertex count mismatch skin={} vertices={}",
            skin.len(),
            eye_mesh.vertices.len()
        ));
    }

    let mut uv_min = [f32::INFINITY; 2];
    let mut uv_max = [f32::NEG_INFINITY; 2];
    let mut max_non_eye_weight = 0.0_f32;
    let mut left_vertices = 0usize;
    let mut right_vertices = 0usize;
    let mut max_center_distance = [0.0_f32; 2];
    for (vertex, skin) in eye_mesh.vertices.iter().zip(skin.iter()) {
        for component in 0..2 {
            if !vertex.uv0[component].is_finite() {
                return Err("native Abby eye UV0 contains non-finite values".to_owned());
            }
            uv_min[component] = uv_min[component].min(vertex.uv0[component]);
            uv_max[component] = uv_max[component].max(vertex.uv0[component]);
        }

        let mut left_weight = 0.0_f32;
        let mut right_weight = 0.0_f32;
        for (&joint, &weight) in skin
            .joints
            .iter()
            .chain(skin.joints_extra.iter())
            .zip(skin.weights.iter().chain(skin.weights_extra.iter()))
        {
            if usize::from(joint) == left {
                left_weight += weight;
            } else if usize::from(joint) == right {
                right_weight += weight;
            }
        }
        let non_eye_weight = (1.0 - left_weight - right_weight).max(0.0);
        max_non_eye_weight = max_non_eye_weight.max(non_eye_weight);
        let position = Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
        if left_weight >= right_weight {
            left_vertices += 1;
            max_center_distance[0] = max_center_distance[0].max(position.distance(left_center));
        } else {
            right_vertices += 1;
            max_center_distance[1] = max_center_distance[1].max(position.distance(right_center));
        }
    }

    let uv_span = [uv_max[0] - uv_min[0], uv_max[1] - uv_min[1]];
    if uv_span[0] < 0.75 || uv_span[1] < 0.75 {
        return Err(format!(
            "native Abby eye UV0 collapsed/squashed u=[{:.6},{:.6}] v=[{:.6},{:.6}] span=[{:.6},{:.6}]",
            uv_min[0], uv_max[0], uv_min[1], uv_max[1], uv_span[0], uv_span[1]
        ));
    }
    let uv_aspect = uv_span[0] / uv_span[1].max(1.0e-8);
    if !(0.90..=1.10).contains(&uv_aspect) {
        return Err(format!(
            "native Abby eye UV0 anisotropy exceeds diagnostic contract aspect={uv_aspect:.6} span=[{:.6},{:.6}]",
            uv_span[0], uv_span[1]
        ));
    }
    if max_non_eye_weight > 1.0e-3 {
        return Err(format!(
            "native Abby eye mesh leaks skin weight outside l/r eyeball joints max_non_eye_weight={max_non_eye_weight:.8}"
        ));
    }
    if left_vertices == 0 || right_vertices == 0 {
        return Err(format!(
            "native Abby eye mesh did not resolve both eyeballs left_vertices={left_vertices} right_vertices={right_vertices}"
        ));
    }
    if max_center_distance[0] > 0.03 || max_center_distance[1] > 0.03 {
        return Err(format!(
            "native Abby eye geometry is displaced from authored bind centers left_max={:.6} right_max={:.6}",
            max_center_distance[0], max_center_distance[1]
        ));
    }

    Ok(())
}

pub(super) fn validate_skin_joint_range(
    mesh: &crate::geometry::ImportMesh,
    joint_count: usize,
    source: &Path,
) -> Result<(), String> {
    let Some(skin) = mesh.skin.as_deref() else {
        return Ok(());
    };
    for (vertex_index, vertex) in skin.iter().enumerate() {
        for (&joint, &weight) in vertex
            .joints
            .iter()
            .chain(vertex.joints_extra.iter())
            .zip(vertex.weights.iter().chain(vertex.weights_extra.iter()))
        {
            if weight > 0.0 && joint as usize >= joint_count {
                return Err(format!(
                    "skin joint outside skeleton source='{}' mesh='{}' vertex={} joint={} joints={}",
                    source.display(),
                    mesh.name,
                    vertex_index,
                    joint,
                    joint_count
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_skin_contract(
    meshes: &[YddBinaryMesh],
    skeleton_joint_count: usize,
) -> Result<(), String> {
    const WEIGHT_EPSILON: f32 = 1.0e-4;
    for mesh in meshes {
        let Some(skin) = mesh.skin.as_ref() else {
            continue;
        };
        if skin.len() != mesh.vertices.len() {
            return Err(format!(
                "native skin stream length mismatch mesh='{}' skin={} vertices={}",
                mesh.name,
                skin.len(),
                mesh.vertices.len()
            ));
        }
        for (vertex_index, vertex) in skin.iter().enumerate() {
            let mut sum = 0.0_f32;
            for (joint, weight) in vertex
                .joints
                .iter()
                .zip(vertex.weights.iter())
                .chain(vertex.joints_extra.iter().zip(vertex.weights_extra.iter()))
            {
                if !weight.is_finite() || *weight < 0.0 {
                    return Err(format!(
                        "native skin has invalid weight mesh='{}' vertex={} weight={}",
                        mesh.name, vertex_index, weight
                    ));
                }
                if *weight > 0.0 && usize::from(*joint) >= skeleton_joint_count {
                    return Err(format!(
                        "native skin joint outside Abby skeleton mesh='{}' vertex={} joint={} joints={}",
                        mesh.name, vertex_index, joint, skeleton_joint_count
                    ));
                }
                sum += *weight;
            }
            if !sum.is_finite() || (sum - 1.0).abs() > WEIGHT_EPSILON {
                return Err(format!(
                    "native skin weights are not normalized mesh='{}' vertex={} sum={sum}",
                    mesh.name, vertex_index
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn encode_nef8(
    raw_body: &[u8],
    content_kind: u32,
    schema_version: u16,
    entry_count: u32,
) -> Result<Vec<u8>, String> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(raw_body)
        .map_err(|error| format!("NEF8 deflate write failed: {error}"))?;
    let stored = encoder
        .finish()
        .map_err(|error| format!("NEF8 deflate finish failed: {error}"))?;
    encode_list_file(ListFileEncodeRequest {
        content_kind,
        content_schema_version: schema_version,
        entry_count,
        additional_flags: 0,
        min_size_class: 5,
        header_metadata: &[],
        body_stored: &stored,
        body_uncompressed_len: raw_body.len() as u64,
        body_raw_hash: None,
        stable_file_id: None,
        import_settings_hash: None,
    })
}
