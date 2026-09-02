use super::*;

fn imported_joint_local_matrix(joint: &crate::skeleton::ImportedJoint) -> Mat4 {
    Mat4::from_scale_rotation_translation(
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
    )
}

pub(super) fn imported_joint_globals(skeleton: &DecodedSkeleton) -> Result<Vec<Mat4>, String> {
    let mut globals = vec![Mat4::IDENTITY; skeleton.joints.len()];
    let mut done = vec![false; skeleton.joints.len()];
    let mut remaining = skeleton.joints.len();
    while remaining > 0 {
        let mut progress = false;
        for (index, joint) in skeleton.joints.iter().enumerate() {
            if done[index] {
                continue;
            }
            if joint
                .parent_index
                .is_some_and(|parent| !done[parent as usize])
            {
                continue;
            }
            let local = imported_joint_local_matrix(joint);
            globals[index] = joint
                .parent_index
                .map(|parent| globals[parent as usize] * local)
                .unwrap_or(local);
            done[index] = true;
            remaining -= 1;
            progress = true;
        }
        if !progress {
            return Err(
                "rigid-joint extraction found an unresolvable skeleton hierarchy".to_owned(),
            );
        }
    }
    Ok(globals)
}

#[inline]
fn dominant_skin_joint(
    vertex: &newengine_asset_format_nef8::ydd_binary::YddBinarySkinVertex,
) -> u16 {
    vertex
        .joints
        .iter()
        .chain(vertex.joints_extra.iter())
        .copied()
        .zip(
            vertex
                .weights
                .iter()
                .chain(vertex.weights_extra.iter())
                .copied(),
        )
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(joint, _)| joint)
        .unwrap_or(0)
}

pub fn compile_rigid_joint_variants(
    request: &RigidJointVariantsCompileRequest,
) -> Result<RigidJointVariantsCompileReport, String> {
    if request.name.trim().is_empty() {
        return Err("rigid-joint asset name must not be empty".to_owned());
    }
    if request.joints.is_empty() {
        return Err("rigid-joint extraction requires at least one joint".to_owned());
    }
    let pak = PakFile::parse(read_file(&request.package_path)?)?;
    let geometry = decode_geometry_lod0(&pak)?;
    let skeleton = decode_skeleton_with_profile(&pak, SkeletonProfile::Generic)?;
    let globals = imported_joint_globals(&skeleton)?;
    let mut entries = Vec::with_capacity(request.joints.len());
    let mut total_meshes = 0usize;
    let mut total_vertices = 0usize;
    let mut total_indices = 0usize;

    for requested_name in &request.joints {
        let requested_name = requested_name.trim();
        if requested_name.is_empty() {
            return Err("rigid-joint extraction contains an empty joint name".to_owned());
        }
        let joint_index = skeleton
            .joints
            .iter()
            .position(|joint| joint.name == requested_name)
            .ok_or_else(|| format!("rigid-joint source has no joint '{requested_name}'"))?;
        let joint_to_local = globals[joint_index].inverse();
        if joint_to_local
            .to_cols_array()
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(format!(
                "rigid-joint '{requested_name}' bind transform is not invertible"
            ));
        }

        let mut entry_meshes = Vec::new();
        let mut entry_min = Vec3::splat(f32::INFINITY);
        let mut entry_max = Vec3::splat(f32::NEG_INFINITY);
        for source_mesh in &geometry.meshes {
            let skin = source_mesh.skin.as_ref().ok_or_else(|| {
                format!(
                    "rigid-joint source mesh '{}' has no skin stream",
                    source_mesh.name
                )
            })?;
            if skin.len() != source_mesh.vertices.len() {
                return Err(format!(
                    "rigid-joint skin/vertex mismatch mesh='{}' skin={} vertices={}",
                    source_mesh.name,
                    skin.len(),
                    source_mesh.vertices.len()
                ));
            }
            let dominant = skin.iter().map(dominant_skin_joint).collect::<Vec<_>>();
            let mut remap = std::collections::BTreeMap::<u32, u32>::new();
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            for triangle in source_mesh.indices.as_chunks::<3>().0 {
                if !triangle.iter().all(|index| {
                    dominant
                        .get(*index as usize)
                        .is_some_and(|joint| *joint as usize == joint_index)
                }) {
                    continue;
                }
                for source_index in triangle {
                    let target_index = if let Some(existing) = remap.get(source_index) {
                        *existing
                    } else {
                        let source = source_mesh
                            .vertices
                            .get(*source_index as usize)
                            .ok_or("rigid-joint source index outside vertex stream")?;
                        let source_position =
                            Vec3::new(source.position[0], source.position[1], source.position[2]);
                        let source_normal =
                            Vec3::new(source.normal[0], source.normal[1], source.normal[2]);
                        let position = joint_to_local.transform_point3(source_position);
                        let normal = joint_to_local
                            .transform_vector3(source_normal)
                            .normalize_or_zero();
                        if !position.is_finite()
                            || !normal.is_finite()
                            || normal.length_squared() <= 1.0e-10
                        {
                            return Err(format!(
                                "rigid-joint '{requested_name}' produced invalid vertex"
                            ));
                        }
                        entry_min = entry_min.min(position);
                        entry_max = entry_max.max(position);
                        let target = vertices.len() as u32;
                        vertices.push(newengine_asset_format_nef8::ydd_binary::YddBinaryVertex {
                            position: [position.x, position.y, position.z],
                            normal: [normal.x, normal.y, normal.z],
                            uv0: source.uv0,
                        });
                        remap.insert(*source_index, target);
                        target
                    };
                    indices.push(target_index);
                }
            }
            if vertices.is_empty() {
                continue;
            }
            let mesh_min = vertices
                .iter()
                .fold(Vec3::splat(f32::INFINITY), |min, vertex| {
                    min.min(Vec3::new(
                        vertex.position[0],
                        vertex.position[1],
                        vertex.position[2],
                    ))
                });
            let mesh_max = vertices
                .iter()
                .fold(Vec3::splat(f32::NEG_INFINITY), |max, vertex| {
                    max.max(Vec3::new(
                        vertex.position[0],
                        vertex.position[1],
                        vertex.position[2],
                    ))
                });
            total_vertices += vertices.len();
            total_indices += indices.len();
            total_meshes += 1;
            entry_meshes.push(YddBinaryMesh {
                name: requested_name.to_owned(),
                material_ref: request.material_ref.clone(),
                bounds_min: [mesh_min.x, mesh_min.y, mesh_min.z],
                bounds_max: [mesh_max.x, mesh_max.y, mesh_max.z],
                vertices,
                skin: None,
                indices,
            });
        }
        if entry_meshes.is_empty() {
            return Err(format!(
                "rigid-joint '{requested_name}' selected no complete source triangles"
            ));
        }
        entries.push(YddBinaryEntry {
            name: requested_name.to_owned(),
            source_path: format!(
                "northstar.pc://{}#joint={requested_name}",
                request
                    .package_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("source.pak")
            ),
            properties_ref: None,
            bounds_min: [entry_min.x, entry_min.y, entry_min.z],
            bounds_max: [entry_max.x, entry_max.y, entry_max.z],
            skin_source_to_model: None,
            meshes: entry_meshes,
        });
    }

    let document = YddBinaryDocument { entries };
    let body = encode_ydd_binary_body(&document)?;
    let file = encode_nef8(
        &body,
        LIST_FILE_CONTENT_KIND_YDD,
        YDD_BINARY_SCHEMA_VERSION as u16,
        document.entries.len() as u32,
    )?;
    if let Some(parent) = request.output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create output directory '{}': {error}",
                parent.display()
            )
        })?;
    }
    write_atomic(&request.output_path, &file)?;
    Ok(RigidJointVariantsCompileReport {
        ydd_path: request.output_path.clone(),
        entry_count: document.entries.len(),
        mesh_count: total_meshes,
        vertex_count: total_vertices,
        index_count: total_indices,
    })
}
