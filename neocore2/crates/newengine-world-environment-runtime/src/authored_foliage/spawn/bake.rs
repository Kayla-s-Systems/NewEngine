fn bake_foliage_mesh(
    prims: &PrimitiveRegistry,
    part: &RuntimePrefabMeshPart,
    placements: &[TreePlacement],
) -> Result<PrimitiveMesh, String> {
    let source = prims.build_mesh(part.primitive_id).map_err(|error| {
        format!(
            "foliage source primitive unavailable id={:?} slot='{}' err='{}'",
            part.primitive_id, part.material_slot, error
        )
    })?;
    let vertex_capacity = source
        .vertices
        .len()
        .checked_mul(placements.len())
        .ok_or_else(|| "foliage vertex capacity overflow".to_owned())?;
    let index_capacity = source
        .indices
        .len()
        .checked_mul(placements.len())
        .ok_or_else(|| "foliage index capacity overflow".to_owned())?;
    let mut mesh = PrimitiveMesh {
        vertices: Vec::with_capacity(vertex_capacity),
        indices: Vec::with_capacity(index_capacity),
        bounds_center: Vec3::ZERO,
        bounds_radius: 0.001,
    };

    for placement in placements {
        let base_vertex = u32::try_from(mesh.vertices.len())
            .map_err(|_| "foliage batch exceeds u32 vertex addressing".to_owned())?;
        let yaw = Quat::from_rotation_y(placement.yaw);
        let uniform_scale = placement.scale.max(0.001);
        for vertex in &source.vertices {
            let local_position = Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]);
            let local_normal = Vec3::new(vertex.nrm[0], vertex.nrm[1], vertex.nrm[2]);
            let world_position = placement.position + yaw * (local_position * uniform_scale);
            let world_normal = (yaw * local_normal).normalize_or_zero();
            mesh.vertices.push(PrimitiveVertex {
                pos: [world_position.x, world_position.y, world_position.z],
                nrm: [world_normal.x, world_normal.y, world_normal.z],
                uv: vertex.uv,
            });
        }
        for &index in &source.indices {
            mesh.indices
                .push(base_vertex.checked_add(index).ok_or_else(|| {
                    "foliage batch index overflow while rebasing source mesh".to_owned()
                })?);
        }
    }

    recompute_ydd_mesh_bounds(&mut mesh);
    Ok(mesh)
}
