fn terrain_height(world: &newengine_ecs::World, terrain: EntityId, x: f32, z: f32) -> f32 {
    world
        .get::<ProceduralTerrain>(terrain)
        .map(|t| t.heightfield.sample_height_local(x, z))
        .unwrap_or(0.0)
}

#[derive(Clone, Copy, Debug)]
struct TreePlacement {
    index: u32,
    position: Vec3,
    yaw: f32,
    scale: f32,
}

#[inline]
fn hash_cell(seed: u64, x: i32, z: i32, salt: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64 ^ seed ^ salt;
    h = h.wrapping_mul(0x100_0000_01b3) ^ (x as i64 as u64);
    h = h.wrapping_mul(0x100_0000_01b3) ^ (z as i64 as u64);
    h ^ (h >> 32)
}

#[inline]
fn unit_from_hash(h: u64) -> f32 {
    ((h >> 40) as u32 as f32) / ((1u32 << 24) as f32)
}

#[inline]
fn choose_foliage_prefab<'a>(
    prefabs: &'a [GameReadyPrefabSpec],
    id: &str,
) -> Option<&'a GameReadyPrefabSpec> {
    prefabs
        .iter()
        .find(|p| p.enabled && p.id == id)
        .or_else(|| prefabs.iter().find(|p| p.enabled && p.proxy == "runtime_gltf_mesh"))
        .or_else(|| prefabs.iter().find(|p| p.enabled && !p.source.trim().is_empty()))
}

fn collect_tree_placements(
    world: &newengine_ecs::World,
    terrain: EntityId,
    spec: &GameReadyFoliageSpec,
    player_start: Vec3,
) -> Vec<TreePlacement> {
    if !spec.enabled {
        return Vec::new();
    }

    let settings = {
        let Some(terrain_data) = world.get::<ProceduralTerrain>(terrain) else {
            return Vec::new();
        };
        terrain_data.heightfield.settings()
    };
    let half_x = settings.size_x * 0.5 - spec.edge_margin;
    let half_z = settings.size_z * 0.5 - spec.edge_margin;
    if half_x <= 0.5 || half_z <= 0.5 {
        return Vec::new();
    }

    let min_player_dist2 = spec.min_player_distance * spec.min_player_distance;
    let mut placements = Vec::with_capacity(spec.max_count.min(512) as usize);

    for gz in spec.grid_min..=spec.grid_max {
        for gx in spec.grid_min..=spec.grid_max {
            if placements.len() as u32 >= spec.max_count {
                return placements;
            }

            let gate = unit_from_hash(hash_cell(spec.seed, gx, gz, 0xa11c_e101));
            if gate > spec.gate_threshold {
                continue;
            }

            let jx = (unit_from_hash(hash_cell(spec.seed, gx, gz, 0x41f0_0001)) * 2.0 - 1.0)
                * spec.spacing
                * spec.jitter;
            let jz = (unit_from_hash(hash_cell(spec.seed, gx, gz, 0x41f0_0002)) * 2.0 - 1.0)
                * spec.spacing
                * spec.jitter;
            let x = gx as f32 * spec.spacing + jx;
            let z = gz as f32 * spec.spacing + jz;
            if x.abs() > half_x || z.abs() > half_z {
                continue;
            }

            let dx = x - player_start.x;
            let dz = z - player_start.z;
            if dx * dx + dz * dz < min_player_dist2 {
                continue;
            }

            let y = terrain_height(world, terrain, x, z) + spec.surface_offset;
            let scale_t = unit_from_hash(hash_cell(spec.seed, gx, gz, 0x51ca_1e00));
            let scale = spec.min_scale + (spec.max_scale - spec.min_scale) * scale_t;
            let yaw = unit_from_hash(hash_cell(spec.seed, gx, gz, 0x7a77_0001)) * core::f32::consts::TAU;

            placements.push(TreePlacement {
                index: placements.len() as u32,
                position: Vec3::new(x, y, z),
                yaw,
                scale,
            });
        }
    }

    placements
}

#[derive(Clone, Debug)]
struct RuntimePrefabMeshPart {
    primitive_id: PrimitiveId,
    material_slot: String,
    material_id: MaterialId,
    color: [f32; 4],
}

#[derive(Clone, Debug)]
struct DecodedPrefabMeshPart {
    primitive_id: PrimitiveId,
    name: String,
    material_slot: String,
    mesh: PrimitiveMesh,
}

#[inline]
fn prefab_asset_raw_bytes(logical_path: &str) -> Result<Vec<u8>, String> {
    let assets = AssetServiceClient::new(default_host_api());
    assets
        .raw_bytes_v1(logical_path)
        .map_err(|e| format!("AssetManager raw read failed path='{logical_path}' err='{e}'"))
}

#[inline]
fn value_array<'a>(v: &'a serde_json::Value, key: &str) -> Result<&'a Vec<serde_json::Value>, String> {
    v.get(key)
        .and_then(|x| x.as_array())
        .ok_or_else(|| format!("gltf: missing array '{key}'"))
}

#[inline]
fn value_index<'a>(arr: &'a [serde_json::Value], index: usize, what: &str) -> Result<&'a serde_json::Value, String> {
    arr.get(index)
        .ok_or_else(|| format!("gltf: {what} index out of range index={index} len={}", arr.len()))
}

#[inline]
fn u64_field(v: &serde_json::Value, key: &str, default: Option<u64>) -> Result<u64, String> {
    match v.get(key).and_then(|x| x.as_u64()) {
        Some(x) => Ok(x),
        None => default.ok_or_else(|| format!("gltf: missing integer field '{key}'")),
    }
}

#[inline]
fn str_field<'a>(v: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|x| x.as_str()).map(str::trim).filter(|x| !x.is_empty())
}

#[inline]
fn logical_dir(path: &str) -> &str {
    path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("")
}

fn join_logical_path(base_dir: &str, rel: &str) -> Result<String, String> {
    let rel = rel.trim().replace('\\', "/");
    if rel.is_empty() {
        return Err("empty relative asset path".to_owned());
    }
    if rel.starts_with("data:") {
        return Err("embedded data: URIs are not used by this runtime path; use AssetManager VFS files".to_owned());
    }
    if rel.contains("://") || rel.starts_with('/') {
        return Err(format!("external/absolute asset URI is not allowed: '{rel}'"));
    }

    let mut parts = Vec::<&str>::new();
    for part in base_dir.split('/').chain(rel.split('/')) {
        match part {
            "" | "." => {}
            ".." => {
                let _ = parts.pop();
            }
            x => parts.push(x),
        }
    }
    Ok(parts.join("/"))
}

fn load_prefab_logical_asset(prefab: &GameReadyPrefabSpec) -> Result<String, String> {
    let bytes = prefab_asset_raw_bytes(&prefab.source)?;
    let doc: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("prefab json parse failed source='{}' err='{e}'", prefab.source))?;
    let logical_asset = str_field(&doc, "logical_asset")
        .ok_or_else(|| format!("prefab source='{}' has no logical_asset", prefab.source))?;

    // Prefer the authored logical asset path exactly as declared. If it is a
    // relative sidecar path such as "scene.gltf", resolve it against the prefab
    // document directory. Both probes go through AssetManager raw VFS access.
    if prefab_asset_raw_bytes(logical_asset).is_ok() {
        Ok(logical_asset.to_owned())
    } else {
        join_logical_path(logical_dir(&prefab.source), logical_asset)
    }
}

#[inline]
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

fn material_for_slot(slot: &str, materials: DemoMaterials, palette: &GameReadyPaletteSpec) -> (MaterialId, [f32; 4]) {
    let s = slot.to_ascii_lowercase();
    if s.contains("leaf") {
        (materials.tree_leaf, palette.tree_leaf)
    } else if s.contains("branch") {
        (materials.tree_branch, palette.tree_branch)
    } else {
        (materials.tree_bark, palette.tree_bark)
    }
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

fn ensure_runtime_prefab_parts(
    prims: &mut PrimitiveRegistry,
    prefab: &GameReadyPrefabSpec,
    materials: DemoMaterials,
    palette: &GameReadyPaletteSpec,
) -> Result<Vec<RuntimePrefabMeshPart>, String> {
    let logical_asset = load_prefab_logical_asset(prefab)?;
    let decoded = decode_runtime_gltf_prefab(&logical_asset)?;
    let mut out = Vec::with_capacity(decoded.len());
    for part in decoded {
        let primitive_id = part.primitive_id;
        let name = part.name;
        let material_slot = part.material_slot;
        let mesh = part.mesh;
        let vertex_count = mesh.vertices.len();
        let index_count = mesh.indices.len();
        if !prims.is_registered(primitive_id) {
            prims.register_mesh(primitive_id, name.clone(), mesh);
            log::info!(
                "game-ready: prefab mesh registered via AssetManager source='{}' asset='{}' part='{}' material='{}' vertices={} indices={}",
                prefab.source,
                logical_asset,
                name,
                material_slot,
                vertex_count,
                index_count
            );
        }
        let (material_id, color) = material_for_slot(&material_slot, materials, palette);
        out.push(RuntimePrefabMeshPart {
            primitive_id,
            material_slot,
            material_id,
            color,
        });
    }
    Ok(out)
}

fn spawn_runtime_gltf_prefab_instance(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    root: EntityId,
    parts: &[RuntimePrefabMeshPart],
    placement: TreePlacement,
) {
    let yaw = Quat::from_rotation_y(placement.yaw);
    let scale = Vec3::splat(placement.scale);
    for (part_index, part) in parts.iter().enumerate() {
        let entity = spawn_game_primitive(
            world,
            prims,
            mats,
            PrimitiveSpawnSpec {
                parent: root,
                primitive_id: part.primitive_id,
                material_id: part.material_id,
                name: &format!("Foliage/TreeAnimate-{}/{}-{part_index}", placement.index, part.material_slot),
                position: placement.position,
                scale,
                color: part.color,
            },
        );
        if let Some(t) = world.get_mut_tracked::<Transform>(entity) {
            t.rotation = yaw;
        }
    }
}

fn spawn_foliage_prefabs(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    root: EntityId,
    terrain: EntityId,
    materials: DemoMaterials,
    palette: &GameReadyPaletteSpec,
    foliage: &GameReadyFoliageSpec,
    prefabs: &[GameReadyPrefabSpec],
    player_start: Vec3,
) {
    let Some(prefab) = choose_foliage_prefab(prefabs, &foliage.prefab) else {
        if foliage.enabled {
            log::warn!(
                "game-ready: foliage enabled but prefab id='{}' is not declared or disabled",
                foliage.prefab
            );
        }
        return;
    };

    let runtime_parts = match ensure_runtime_prefab_parts(prims, prefab, materials, palette) {
        Ok(parts) => parts,
        Err(e) => {
            log::error!(
                "game-ready: prefab id='{}' source='{}' proxy='{}' failed to load real mesh through AssetManager; foliage skipped err='{}'",
                prefab.id,
                prefab.source,
                prefab.proxy,
                e
            );
            return;
        }
    };

    let placements = collect_tree_placements(world, terrain, foliage, player_start);
    let count = placements.len();
    for placement in placements {
        spawn_runtime_gltf_prefab_instance(world, &*prims, mats, root, &runtime_parts, placement);
    }

    log::info!(
        "game-ready: foliage prefab placement prefab='{}' source='{}' proxy='{}' mode='runtime_gltf_mesh' parts={} placed={} max_count={} grid={}..{} spacing={:.2}",
        prefab.id,
        prefab.source,
        prefab.proxy,
        runtime_parts.len(),
        count,
        foliage.max_count,
        foliage.grid_min,
        foliage.grid_max,
        foliage.spacing,
    );
}

const SKYDOME_PRIMITIVE_ID: PrimitiveId = PrimitiveId(fnv1a_64("kalitech.asset.skydome.high.v1"));
