fn canonical_ydd_prefab_ref(prefab: &GameReadyPrefabSpec) -> Result<String, String> {
    let source = prefab.source.trim().replace('\\', "/");
    if source.is_empty() {
        return Err(format!("prefab id='{}' has no .ydd@entry source", prefab.id));
    }
    let lower = source.to_ascii_lowercase();
    if !lower.contains(".ydd@") {
        return Err(format!(
            "prefab id='{}' source='{}' rejected: runtime foliage requires .ydd@entry, not prefab/json/gltf sidecars",
            prefab.id, source
        ));
    }
    Ok(source)
}

fn ydd_body_json(logical_ref: &str) -> Result<serde_json::Value, String> {
    let assets = AssetServiceClient::new(default_host_api());
    let body = assets
        .decode_v1(&newengine_assets::AssetDecodeRequest {
            logical_path: logical_ref.to_owned(),
            output_kind: "asset.list_file_body_v1".to_owned(),
            selector: serde_json::Value::Null,
        })
        .map_err(|e| format!("AssetManager decode .ydd body failed path='{logical_ref}' err='{e}'"))?;
    serde_json::from_slice(&body).map_err(|e| format!("ydd body JSON parse failed path='{logical_ref}' err='{e}'"))
}

#[inline]
fn ydd_array<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a Vec<serde_json::Value>, String> {
    value
        .get(key)
        .and_then(|x| x.as_array())
        .ok_or_else(|| format!("ydd runtime mesh part missing array '{key}'"))
}

fn ydd_vec3(value: &serde_json::Value, key: &str, default: [f32; 3]) -> [f32; 3] {
    let Some(values) = value.get(key).and_then(|x| x.as_array()) else { return default; };
    if values.len() < 3 {
        return default;
    }
    [
        values[0].as_f64().unwrap_or(default[0] as f64) as f32,
        values[1].as_f64().unwrap_or(default[1] as f64) as f32,
        values[2].as_f64().unwrap_or(default[2] as f64) as f32,
    ]
}

fn ydd_vec2(value: &serde_json::Value, key: &str, default: [f32; 2]) -> [f32; 2] {
    let Some(values) = value.get(key).and_then(|x| x.as_array()) else { return default; };
    if values.len() < 2 {
        return default;
    }
    [
        values[0].as_f64().unwrap_or(default[0] as f64) as f32,
        values[1].as_f64().unwrap_or(default[1] as f64) as f32,
    ]
}

fn recompute_ydd_mesh_bounds(mesh: &mut PrimitiveMesh) {
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

fn decode_ydd_runtime_mesh_part(
    logical_ref: &str,
    index: usize,
    part: &serde_json::Value,
    material_ref: Option<String>,
) -> Result<DecodedPrefabMeshPart, String> {
    let name = part
        .get("name")
        .and_then(|x| x.as_str())
        .unwrap_or("YDD/Prefab/Mesh")
        .trim()
        .to_owned();
    let material_slot = part
        .get("material_slot")
        .and_then(|x| x.as_str())
        .unwrap_or("Default")
        .trim()
        .to_owned();
    let primitive_id = PrimitiveId(
        part.get("primitive_id")
            .and_then(|x| x.as_u64())
            .unwrap_or_else(|| fnv1a_64(&format!("newengine.prefab.ydd:{logical_ref}#part:{index}:mat:{material_slot}"))),
    );

    let vertices_json = ydd_array(part, "vertices")?;
    let indices_json = ydd_array(part, "indices")?;
    if vertices_json.is_empty() {
        return Err(format!("ydd runtime mesh part has no vertices path='{logical_ref}' index={index}"));
    }
    if indices_json.len() % 3 != 0 {
        return Err(format!(
            "ydd runtime mesh part index count is not triangular path='{logical_ref}' index={index} indices={}",
            indices_json.len()
        ));
    }

    let mut vertices = Vec::with_capacity(vertices_json.len());
    for vertex in vertices_json {
        vertices.push(PrimitiveVertex {
            pos: ydd_vec3(vertex, "pos", [0.0, 0.0, 0.0]),
            nrm: ydd_vec3(vertex, "nrm", [0.0, 1.0, 0.0]),
            uv: ydd_vec2(vertex, "uv", [0.0, 0.0]),
        });
    }

    let mut indices = Vec::with_capacity(indices_json.len());
    for item in indices_json {
        let Some(index_value) = item.as_u64() else {
            return Err(format!("ydd runtime mesh part index is not u64 path='{logical_ref}' part={index}"));
        };
        let index_value = u32::try_from(index_value)
            .map_err(|_| format!("ydd runtime mesh part index exceeds u32 path='{logical_ref}' part={index} index={index_value}"))?;
        if index_value as usize >= vertices.len() {
            return Err(format!(
                "ydd runtime mesh part index out of bounds path='{logical_ref}' part={index} index={} vertices={}",
                index_value,
                vertices.len()
            ));
        }
        indices.push(index_value);
    }

    let mut mesh = PrimitiveMesh {
        vertices,
        indices,
        bounds_center: Vec3::ZERO,
        bounds_radius: 0.001,
    };
    recompute_ydd_mesh_bounds(&mut mesh);

    Ok(DecodedPrefabMeshPart {
        primitive_id,
        name,
        material_slot,
        material_ref,
        mesh,
    })
}

fn ydd_material_ref_for_runtime_part(body: &serde_json::Value, index: usize) -> Option<String> {
    let mesh_part = body.get("mesh_parts")?.as_array()?.get(index)?;
    let slot_index = mesh_part.get("material_slot_index")?.as_u64()? as usize;
    let slot = body.get("material_slots")?.as_array()?.get(slot_index)?;
    let reference = slot.get("material_ref")?.as_str()?.trim().replace('\\', "/");
    (!reference.is_empty()).then_some(reference)
}

fn decode_runtime_ydd_prefab(logical_ref: &str) -> Result<Vec<DecodedPrefabMeshPart>, String> {
    let body = ydd_body_json(logical_ref)?;
    let encoding = body
        .get("mesh_encoding")
        .and_then(|x| x.as_str())
        .unwrap_or("unknown");
    if encoding != "newengine.ydd.runtime_mesh_parts.v1" {
        return Err(format!(
            "ydd drawable has no supported runtime mesh encoding path='{logical_ref}' encoding='{encoding}'"
        ));
    }
    let parts_json = ydd_array(&body, "runtime_mesh_parts")?;
    let mut parts = Vec::with_capacity(parts_json.len());
    for (index, part) in parts_json.iter().enumerate() {
        let material_ref = ydd_material_ref_for_runtime_part(&body, index);
        parts.push(decode_ydd_runtime_mesh_part(logical_ref, index, part, material_ref)?);
    }
    if parts.is_empty() {
        return Err(format!("ydd drawable has no runtime mesh parts path='{logical_ref}'"));
    }
    log::info!(
        "game-ready: ydd drawable decoded path='{}' parts={} policy='.ymap -> .ytyp -> .ydd -> .nemat -> .ytd'",
        logical_ref,
        parts.len()
    );
    Ok(parts)
}
