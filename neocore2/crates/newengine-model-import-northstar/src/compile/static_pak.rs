use super::*;

/// Compiles one authored NorthStar PC geometry package into a rigid runtime YDD.
///
/// Some prop/weapon packages carry a small skin stream while the hierarchy lives in a
/// separate authoring package. For assets explicitly declared as rigid bind-pose props,
/// the decoded vertex positions are already in model space and the runtime does not need
/// the source skin. The opt-in flag is mandatory so character/skinned geometry can never
/// lose its skin silently.
pub fn compile_static_pak(
    request: &StaticPakCompileRequest,
) -> Result<StaticPakCompileReport, String> {
    let name = request.name.trim();
    if name.is_empty() {
        return Err("static PAK asset name must not be empty".to_owned());
    }
    let material_ref = request
        .material_ref
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "static PAK compile requires a material_ref".to_owned())?
        .to_owned();

    let pak = PakFile::parse(read_file(&request.package_path)?)?;
    let mut geometry = decode_geometry_lod0(&pak)?;
    if geometry.meshes.is_empty() {
        return Err("static PAK source contains no LOD0 meshes".to_owned());
    }

    let has_source_skin = geometry.meshes.iter().any(|mesh| mesh.skin.is_some());
    if has_source_skin && !request.bake_skinned_bind_pose {
        return Err(format!(
            "static PAK source contains skin streams package='{}'; set bake_skinned_bind_pose only for an authored rigid bind-pose asset",
            request.package_path.display()
        ));
    }

    if let Some(matrix) = request.source_to_model {
        let transform = validate_rigid_source_to_model(matrix)?;
        for mesh in &mut geometry.meshes {
            transform_mesh_to_model_space(mesh, transform)?;
        }
    }

    let mut bounds_min = Vec3::splat(f32::INFINITY);
    let mut bounds_max = Vec3::splat(f32::NEG_INFINITY);
    let mut meshes = Vec::with_capacity(geometry.meshes.len());
    let mut vertex_count = 0usize;
    let mut index_count = 0usize;

    for mesh in geometry.meshes {
        let mesh_min = Vec3::new(mesh.bounds_min[0], mesh.bounds_min[1], mesh.bounds_min[2]);
        let mesh_max = Vec3::new(mesh.bounds_max[0], mesh.bounds_max[1], mesh.bounds_max[2]);
        if !mesh_min.is_finite()
            || !mesh_max.is_finite()
            || !(mesh_min.x <= mesh_max.x && mesh_min.y <= mesh_max.y && mesh_min.z <= mesh_max.z)
        {
            return Err(format!(
                "static PAK mesh has invalid bounds mesh='{}'",
                mesh.name
            ));
        }
        bounds_min = bounds_min.min(mesh_min);
        bounds_max = bounds_max.max(mesh_max);
        vertex_count = vertex_count.saturating_add(mesh.vertices.len());
        index_count = index_count.saturating_add(mesh.indices.len());
        meshes.push(YddBinaryMesh {
            name: mesh.name,
            material_ref: Some(material_ref.clone()),
            bounds_min: mesh.bounds_min,
            bounds_max: mesh.bounds_max,
            vertices: mesh.vertices,
            // Explicit bind-pose rigid bake: source skin is intentionally not published.
            skin: None,
            indices: mesh.indices,
        });
    }

    if !bounds_min.is_finite() || !bounds_max.is_finite() {
        return Err("static PAK aggregate bounds are invalid".to_owned());
    }

    let source_name = request
        .package_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("source.pak");
    let source_mode = if has_source_skin {
        "bind_pose_rigid_bake"
    } else {
        "rigid_static"
    };
    let document = YddBinaryDocument {
        entries: vec![YddBinaryEntry {
            name: name.to_owned(),
            source_path: format!("northstar.pc://{source_name}#{source_mode}"),
            properties_ref: None,
            bounds_min: [bounds_min.x, bounds_min.y, bounds_min.z],
            bounds_max: [bounds_max.x, bounds_max.y, bounds_max.z],
            skin_source_to_model: None,
            meshes,
        }],
    };
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
                "failed to create static YDD output directory '{}': {error}",
                parent.display()
            )
        })?;
    }
    write_atomic(&request.output_path, &file)?;

    Ok(StaticPakCompileReport {
        ydd_path: request.output_path.clone(),
        mesh_count: document.entries[0].meshes.len(),
        vertex_count,
        index_count,
        bind_pose_baked: has_source_skin,
    })
}
