fn component_count(accessor_type: &str) -> Result<usize, String> {
    match accessor_type {
        "SCALAR" => Ok(1),
        "VEC2" => Ok(2),
        "VEC3" => Ok(3),
        "VEC4" => Ok(4),
        "MAT4" => Ok(16),
        other => Err(format!("gltf: unsupported accessor type '{other}'")),
    }
}

#[inline]
fn component_size(component_type: u64) -> Result<usize, String> {
    match component_type {
        5120 | 5121 => Ok(1),
        5122 | 5123 => Ok(2),
        5125 | 5126 => Ok(4),
        other => Err(format!("gltf: unsupported component type {other}")),
    }
}

fn accessor_layout<'a>(
    gltf: &'a serde_json::Value,
    accessor_index: usize,
) -> Result<(&'a serde_json::Value, &'a serde_json::Value, usize, usize, usize, u64, usize), String> {
    let accessors = value_array(gltf, "accessors")?;
    let buffer_views = value_array(gltf, "bufferViews")?;
    let accessor = value_index(accessors, accessor_index, "accessor")?;
    let view_index = u64_field(accessor, "bufferView", None)? as usize;
    let view = value_index(buffer_views, view_index, "bufferView")?;
    let component_type = u64_field(accessor, "componentType", None)?;
    let accessor_type = str_field(accessor, "type").ok_or_else(|| "gltf: accessor has no type".to_owned())?;
    let components = component_count(accessor_type)?;
    let elem_size = component_size(component_type)? * components;
    let stride = u64_field(view, "byteStride", Some(elem_size as u64))? as usize;
    let offset = u64_field(view, "byteOffset", Some(0))? as usize
        + u64_field(accessor, "byteOffset", Some(0))? as usize;
    let count = u64_field(accessor, "count", None)? as usize;
    Ok((accessor, view, offset, stride, components, component_type, count))
}

#[inline]
fn read_f32_at(bytes: &[u8], offset: usize) -> Result<f32, String> {
    let b = bytes
        .get(offset..offset.saturating_add(4))
        .ok_or_else(|| format!("gltf: f32 read out of bounds offset={offset}"))?;
    Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn read_accessor_vec3(gltf: &serde_json::Value, bin: &[u8], accessor_index: usize) -> Result<Vec<[f32; 3]>, String> {
    let (_accessor, _view, offset, stride, components, component_type, count) = accessor_layout(gltf, accessor_index)?;
    if component_type != 5126 || components < 3 {
        return Err(format!("gltf: accessor {accessor_index} is not FLOAT VEC3"));
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = offset + i * stride;
        out.push([read_f32_at(bin, o)?, read_f32_at(bin, o + 4)?, read_f32_at(bin, o + 8)?]);
    }
    Ok(out)
}

fn read_accessor_vec2(gltf: &serde_json::Value, bin: &[u8], accessor_index: usize, fallback_count: usize) -> Result<Vec<[f32; 2]>, String> {
    let (_accessor, _view, offset, stride, components, component_type, count) = accessor_layout(gltf, accessor_index)?;
    if component_type != 5126 || components < 2 {
        return Err(format!("gltf: accessor {accessor_index} is not FLOAT VEC2"));
    }
    let mut out = Vec::with_capacity(count.max(fallback_count));
    for i in 0..count {
        let o = offset + i * stride;
        out.push([read_f32_at(bin, o)?, read_f32_at(bin, o + 4)?]);
    }
    if out.len() < fallback_count {
        out.resize(fallback_count, [0.0, 0.0]);
    }
    Ok(out)
}

fn read_accessor_indices(gltf: &serde_json::Value, bin: &[u8], accessor_index: usize) -> Result<Vec<u32>, String> {
    let (_accessor, _view, offset, stride, components, component_type, count) = accessor_layout(gltf, accessor_index)?;
    if components != 1 {
        return Err(format!("gltf: index accessor {accessor_index} is not scalar"));
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = offset + i * stride;
        let idx = match component_type {
            5121 => *bin.get(o).ok_or_else(|| format!("gltf: u8 index read out of bounds offset={o}"))? as u32,
            5123 => {
                let b = bin.get(o..o + 2).ok_or_else(|| format!("gltf: u16 index read out of bounds offset={o}"))?;
                u16::from_le_bytes([b[0], b[1]]) as u32
            }
            5125 => {
                let b = bin.get(o..o + 4).ok_or_else(|| format!("gltf: u32 index read out of bounds offset={o}"))?;
                u32::from_le_bytes([b[0], b[1], b[2], b[3]])
            }
            other => return Err(format!("gltf: unsupported index component type {other}")),
        };
        out.push(idx);
    }
    Ok(out)
}

#[inline]
fn parse_vec3_field(v: &serde_json::Value, key: &str, default: Vec3) -> Vec3 {
    let Some(a) = v.get(key).and_then(|x| x.as_array()) else { return default; };
    if a.len() < 3 {
        return default;
    }
    Vec3::new(
        a[0].as_f64().unwrap_or(default.x as f64) as f32,
        a[1].as_f64().unwrap_or(default.y as f64) as f32,
        a[2].as_f64().unwrap_or(default.z as f64) as f32,
    )
}

#[inline]
fn parse_quat_field(v: &serde_json::Value, key: &str, default: Quat) -> Quat {
    let Some(a) = v.get(key).and_then(|x| x.as_array()) else { return default; };
    if a.len() < 4 {
        return default;
    }
    Quat::from_xyzw(
        a[0].as_f64().unwrap_or(default.x as f64) as f32,
        a[1].as_f64().unwrap_or(default.y as f64) as f32,
        a[2].as_f64().unwrap_or(default.z as f64) as f32,
        a[3].as_f64().unwrap_or(default.w as f64) as f32,
    )
    .normalize_or_identity()
}

fn parse_node_matrix(node: &serde_json::Value) -> Mat4 {
    if let Some(a) = node.get("matrix").and_then(|x| x.as_array()) {
        if a.len() >= 16 {
            let mut m = [0.0f32; 16];
            for i in 0..16 {
                m[i] = a[i].as_f64().unwrap_or(if i % 5 == 0 { 1.0 } else { 0.0 }) as f32;
            }
            return Mat4::from_cols_array(&m);
        }
    }

    let t = parse_vec3_field(node, "translation", Vec3::ZERO);
    let r = parse_quat_field(node, "rotation", Quat::IDENTITY);
    let s = parse_vec3_field(node, "scale", Vec3::ONE);
    Mat4::from_scale_rotation_translation(s, r, t)
}

fn material_slot_for_index(gltf: &serde_json::Value, material_index: Option<usize>) -> String {
    let Some(index) = material_index else { return "Default".to_owned(); };
    value_array(gltf, "materials")
        .ok()
        .and_then(|materials| materials.get(index))
        .and_then(|m| str_field(m, "name"))
        .unwrap_or("Default")
        .to_owned()
}


fn recompute_mesh_bounds(mesh: &mut PrimitiveMesh) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for v in &mesh.vertices {
        let p = Vec3::new(v.pos[0], v.pos[1], v.pos[2]);
        min = min.min(p);
        max = max.max(p);
    }
    if mesh.vertices.is_empty() || !min.is_finite() || !max.is_finite() {
        mesh.bounds_center = Vec3::ZERO;
        mesh.bounds_radius = 0.001;
        return;
    }
    mesh.bounds_center = (min + max) * 0.5;
    let mut radius = 0.0f32;
    for v in &mesh.vertices {
        let p = Vec3::new(v.pos[0], v.pos[1], v.pos[2]);
        radius = radius.max((p - mesh.bounds_center).length());
    }
    mesh.bounds_radius = radius.max(0.001);
}

fn normalize_prefab_meshes(parts: &mut [DecodedPrefabMeshPart], target_height: f32) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for part in parts.iter() {
        for v in &part.mesh.vertices {
            let p = Vec3::new(v.pos[0], v.pos[1], v.pos[2]);
            min = min.min(p);
            max = max.max(p);
        }
    }
    if !min.is_finite() || !max.is_finite() || max.y <= min.y + 1.0e-5 {
        return;
    }
    let height = max.y - min.y;
    let scale = (target_height / height).clamp(0.0001, 100.0);
    let center_x = (min.x + max.x) * 0.5;
    let center_z = (min.z + max.z) * 0.5;

    for part in parts.iter_mut() {
        for v in &mut part.mesh.vertices {
            let p = Vec3::new(v.pos[0], v.pos[1], v.pos[2]);
            let p = Vec3::new(p.x - center_x, p.y - min.y, p.z - center_z) * scale;
            v.pos = [p.x, p.y, p.z];
        }
        recompute_mesh_bounds(&mut part.mesh);
    }

    log::info!(
        "game-ready: prefab GLTF normalized source_height={:.3} target_height={:.3} scale={:.5}",
        height,
        target_height,
        scale
    );
}

fn decode_gltf_primitive_mesh(
    gltf: &serde_json::Value,
    bin: &[u8],
    transform: Mat4,
    primitive: &serde_json::Value,
) -> Result<PrimitiveMesh, String> {
    let attrs = primitive
        .get("attributes")
        .and_then(|x| x.as_object())
        .ok_or_else(|| "gltf: primitive has no attributes".to_owned())?;
    let pos_accessor = attrs
        .get("POSITION")
        .and_then(|x| x.as_u64())
        .ok_or_else(|| "gltf: primitive has no POSITION accessor".to_owned())? as usize;
    let positions = read_accessor_vec3(gltf, bin, pos_accessor)?;
    let normals = match attrs.get("NORMAL").and_then(|x| x.as_u64()) {
        Some(i) => read_accessor_vec3(gltf, bin, i as usize)?,
        None => vec![[0.0, 1.0, 0.0]; positions.len()],
    };
    let uvs = match attrs.get("TEXCOORD_0").and_then(|x| x.as_u64()) {
        Some(i) => read_accessor_vec2(gltf, bin, i as usize, positions.len())?,
        None => vec![[0.0, 0.0]; positions.len()],
    };
    let indices = match primitive.get("indices").and_then(|x| x.as_u64()) {
        Some(i) => read_accessor_indices(gltf, bin, i as usize)?,
        None => (0..positions.len() as u32).collect::<Vec<_>>(),
    };
    if indices.len() % 3 != 0 {
        return Err(format!("gltf: primitive index count is not triangular indices={}", indices.len()));
    }

    let mut vertices = Vec::with_capacity(positions.len());
    for i in 0..positions.len() {
        let p = positions[i];
        let n = normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
        let uv = uvs.get(i).copied().unwrap_or([0.0, 0.0]);
        let wp = transform.transform_point3(Vec3::new(p[0], p[1], p[2]));
        let wn = transform.transform_vector3(Vec3::new(n[0], n[1], n[2])).normalize_or_zero();
        vertices.push(PrimitiveVertex {
            pos: [wp.x, wp.y, wp.z],
            nrm: [wn.x, wn.y, wn.z],
            uv,
        });
    }

    let mut mesh = PrimitiveMesh {
        vertices,
        indices,
        bounds_center: Vec3::ZERO,
        bounds_radius: 0.001,
    };
    recompute_mesh_bounds(&mut mesh);
    Ok(mesh)
}

fn decode_gltf_node_meshes(
    gltf: &serde_json::Value,
    bin: &[u8],
    node_index: usize,
    parent: Mat4,
    logical_asset: &str,
    out: &mut Vec<DecodedPrefabMeshPart>,
) -> Result<(), String> {
    let nodes = value_array(gltf, "nodes")?;
    let node = value_index(nodes, node_index, "node")?;
    let transform = parent * parse_node_matrix(node);

    if let Some(mesh_index) = node.get("mesh").and_then(|x| x.as_u64()).map(|x| x as usize) {
        let meshes = value_array(gltf, "meshes")?;
        let mesh = value_index(meshes, mesh_index, "mesh")?;
        let primitives = mesh
            .get("primitives")
            .and_then(|x| x.as_array())
            .ok_or_else(|| format!("gltf: mesh index={mesh_index} has no primitives"))?;
        for (prim_index, primitive) in primitives.iter().enumerate() {
            let material_index = primitive.get("material").and_then(|x| x.as_u64()).map(|x| x as usize);
            let material_slot = material_slot_for_index(gltf, material_index);
            let primitive_id = PrimitiveId(fnv1a_64(&format!(
                "newengine.prefab.gltf:{}#node{}:mesh{}:prim{}:mat{}",
                logical_asset, node_index, mesh_index, prim_index, material_slot
            )));
            let mesh_name = mesh
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("mesh");
            let decoded = decode_gltf_primitive_mesh(gltf, bin, transform, primitive)?;
            out.push(DecodedPrefabMeshPart {
                primitive_id,
                name: format!("Imported/Prefab/{mesh_name}/{material_slot}"),
                material_slot,
                mesh: decoded,
            });
        }
    }

    if let Some(children) = node.get("children").and_then(|x| x.as_array()) {
        for child in children {
            if let Some(child_index) = child.as_u64() {
                decode_gltf_node_meshes(gltf, bin, child_index as usize, transform, logical_asset, out)?;
            }
        }
    }

    Ok(())
}

fn decode_runtime_gltf_prefab(logical_asset: &str) -> Result<Vec<DecodedPrefabMeshPart>, String> {
    let gltf_bytes = prefab_asset_raw_bytes(logical_asset)?;
    let gltf: serde_json::Value = serde_json::from_slice(&gltf_bytes)
        .map_err(|e| format!("gltf json parse failed path='{logical_asset}' err='{e}'"))?;
    let buffers = value_array(&gltf, "buffers")?;
    let buffer0 = value_index(buffers, 0, "buffer")?;
    let buffer_uri = str_field(buffer0, "uri").ok_or_else(|| "gltf: buffer[0] has no uri".to_owned())?;
    let bin_path = join_logical_path(logical_dir(logical_asset), buffer_uri)?;
    let bin = prefab_asset_raw_bytes(&bin_path)?;

    let scenes = value_array(&gltf, "scenes")?;
    let scene_index = gltf.get("scene").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    let scene = value_index(scenes, scene_index, "scene")?;
    let root_nodes = scene
        .get("nodes")
        .and_then(|x| x.as_array())
        .ok_or_else(|| format!("gltf: scene index={scene_index} has no root nodes"))?;

    let mut parts = Vec::new();
    for node in root_nodes {
        if let Some(node_index) = node.as_u64() {
            decode_gltf_node_meshes(&gltf, &bin, node_index as usize, Mat4::IDENTITY, logical_asset, &mut parts)?;
        }
    }
    if parts.is_empty() {
        return Err(format!("gltf: no renderable mesh primitives path='{logical_asset}'"));
    }

    normalize_prefab_meshes(&mut parts, 4.8);
    Ok(parts)
}
