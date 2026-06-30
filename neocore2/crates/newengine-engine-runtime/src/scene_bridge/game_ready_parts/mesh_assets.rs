//! Mesh asset import helpers for the GameReady scene bootstrap.
//!
//! Kept separate from `assets_bootstrap.rs` so scene assembly stays readable:
//! this module owns NE3D/YDD mesh decoding and authored SkyDome primitive loading.

use super::*;

use newengine_assets::{AssetAccess, AssetDecodeRequest, ASSET_LIST_FILE_BODY_OUTPUT};
use std::time::Duration;

pub(super) fn read_u32_le(payload: &[u8], offset: &mut usize) -> Result<u32, String> {
    let end = offset.saturating_add(4);
    let bytes = payload
        .get(*offset..end)
        .ok_or_else(|| "NE3D payload truncated while reading u32".to_owned())?;
    *offset = end;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[inline]
pub(super) fn read_f32_le(payload: &[u8], offset: &mut usize) -> Result<f32, String> {
    Ok(f32::from_bits(read_u32_le(payload, offset)?))
}

pub(super) fn decode_ne3d_mesh(payload: &[u8]) -> Result<PrimitiveMesh, String> {
    if payload.len() < 20 || payload.get(0..4) != Some(b"NE3D") {
        return Err("invalid NE3D header".to_owned());
    }

    let mut offset = 4usize;
    let version = read_u32_le(payload, &mut offset)?;
    if version != 1 {
        return Err(format!("unsupported NE3D version={version}"));
    }

    let vertex_count = read_u32_le(payload, &mut offset)? as usize;
    let index_count = read_u32_le(payload, &mut offset)? as usize;
    let flags = read_u32_le(payload, &mut offset)?;

    if vertex_count == 0 || index_count == 0 {
        return Err(format!(
            "empty NE3D mesh vertices={vertex_count} indices={index_count}"
        ));
    }
    if vertex_count > 1_000_000 || index_count > 6_000_000 {
        return Err(format!(
            "NE3D mesh exceeds runtime limits vertices={vertex_count} indices={index_count}"
        ));
    }

    let has_normals = (flags & 0x1) != 0;
    let has_uvs = (flags & 0x2) != 0;

    let mut positions = Vec::with_capacity(vertex_count);
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);

    for _ in 0..vertex_count {
        let pos = [
            read_f32_le(payload, &mut offset)?,
            read_f32_le(payload, &mut offset)?,
            read_f32_le(payload, &mut offset)?,
        ];
        min.x = min.x.min(pos[0]);
        min.y = min.y.min(pos[1]);
        min.z = min.z.min(pos[2]);
        max.x = max.x.max(pos[0]);
        max.y = max.y.max(pos[1]);
        max.z = max.z.max(pos[2]);
        positions.push(pos);
    }

    let mut normals = Vec::with_capacity(vertex_count);
    if has_normals {
        for _ in 0..vertex_count {
            normals.push([
                read_f32_le(payload, &mut offset)?,
                read_f32_le(payload, &mut offset)?,
                read_f32_le(payload, &mut offset)?,
            ]);
        }
    } else {
        normals.resize(vertex_count, [0.0, 1.0, 0.0]);
    }

    let mut uvs = Vec::with_capacity(vertex_count);
    if has_uvs {
        for _ in 0..vertex_count {
            uvs.push([
                read_f32_le(payload, &mut offset)?,
                read_f32_le(payload, &mut offset)?,
            ]);
        }
    } else {
        uvs.resize(vertex_count, [0.0, 0.0]);
    }

    let mut indices = Vec::with_capacity(index_count);
    for _ in 0..index_count {
        let index = read_u32_le(payload, &mut offset)?;
        if index as usize >= vertex_count {
            return Err(format!(
                "NE3D index out of bounds index={index} vertex_count={vertex_count}"
            ));
        }
        indices.push(index);
    }

    let vertices = positions
        .into_iter()
        .zip(normals)
        .zip(uvs)
        .map(|((pos, nrm), uv)| PrimitiveVertex { pos, nrm, uv })
        .collect::<Vec<_>>();

    let bounds_center = (min + max) * 0.5;
    let mut bounds_radius = 0.0f32;
    for v in &vertices {
        let p = Vec3::new(v.pos[0], v.pos[1], v.pos[2]);
        bounds_radius = bounds_radius.max((p - bounds_center).length());
    }

    Ok(PrimitiveMesh {
        vertices,
        indices,
        bounds_center,
        bounds_radius: bounds_radius.max(0.001),
    })
}

pub(super) fn load_ne3d_mesh_asset(logical_path: &str) -> Result<PrimitiveMesh, String> {
    let assets = AssetServiceClient::new(default_host_api());
    let id = assets
        .import_v1(logical_path)
        .map_err(|e| format!("asset.import_v1 failed path='{logical_path}' err='{e}'"))?;

    wait_ready(&assets, &id, Duration::from_secs(3))
        .map_err(|e| format!("asset not ready path='{logical_path}' id='{id}' err='{e:?}'"))?;

    let (meta, payload) = assets.blob_wire_v1(&id).map_err(|e| {
        format!("asset.blob_wire_v1 failed path='{logical_path}' id='{id}' err='{e}'")
    })?;

    if !meta.contains("kalitech.model3d.meta.v1") {
        newengine_ulog_api::ulog::warn!(
            "game-ready: geometry asset meta is not model3d schema path='{}' meta='{}'",
            logical_path,
            meta
        );
    }

    decode_ne3d_mesh(&payload)
}

pub(super) fn split_ydd_asset_ref(logical_path: &str) -> Option<(&str, Option<&str>)> {
    let trimmed = logical_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (dictionary, selector) = match trimmed.split_once('@') {
        Some((dictionary, selector)) => (
            dictionary.trim(),
            Some(selector.trim()).filter(|s| !s.is_empty()),
        ),
        None => (trimmed, None),
    };
    if newengine_assets::require_asset_reference_extension(dictionary, &["ydd"], false).is_ok() {
        Some((dictionary, selector))
    } else {
        None
    }
}

pub(super) fn json_array<'a>(
    value: &'a serde_json::Value,
    label: &str,
) -> Result<&'a [serde_json::Value], String> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{label} must be an array"))
}

pub(super) fn json_f32(value: &serde_json::Value, label: &str) -> Result<f32, String> {
    value
        .as_f64()
        .map(|v| v as f32)
        .ok_or_else(|| format!("{label} must be a number"))
}

pub(super) fn json_vec3(value: &serde_json::Value, label: &str) -> Result<[f32; 3], String> {
    let arr = json_array(value, label)?;
    if arr.len() != 3 {
        return Err(format!("{label} must have 3 components, got {}", arr.len()));
    }
    Ok([
        json_f32(&arr[0], label)?,
        json_f32(&arr[1], label)?,
        json_f32(&arr[2], label)?,
    ])
}

pub(super) fn json_vec2(value: &serde_json::Value, label: &str) -> Result<[f32; 2], String> {
    let arr = json_array(value, label)?;
    if arr.len() != 2 {
        return Err(format!("{label} must have 2 components, got {}", arr.len()));
    }
    Ok([json_f32(&arr[0], label)?, json_f32(&arr[1], label)?])
}

pub(super) fn select_ydd_mesh_part<'a>(
    root: &'a serde_json::Value,
    selector: Option<&str>,
) -> Result<&'a serde_json::Value, String> {
    let parts = root
        .get("mesh_parts")
        .ok_or_else(|| "YDD payload has no mesh_parts array".to_owned())
        .and_then(|v| json_array(v, "mesh_parts"))?;
    if parts.is_empty() {
        return Err("YDD payload has no mesh parts".to_owned());
    }

    if let Some(selector) = selector {
        if let Some(part) = parts.iter().find(|part| {
            part.get("entry")
                .and_then(serde_json::Value::as_str)
                .map(|v| v.eq_ignore_ascii_case(selector))
                .unwrap_or(false)
                || part
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .map(|v| v.eq_ignore_ascii_case(selector))
                    .unwrap_or(false)
        }) {
            return Ok(part);
        }
        return Err(format!(
            "YDD selector '{selector}' was not found in mesh_parts"
        ));
    }

    Ok(&parts[0])
}

pub(super) fn decode_ydd_mesh(
    dictionary_path: &str,
    selector: Option<&str>,
    payload: &[u8],
) -> Result<PrimitiveMesh, String> {
    let root: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|e| format!("YDD payload is not valid JSON path='{dictionary_path}' err='{e}'"))?;
    if root
        .get("mesh_encoding")
        .and_then(serde_json::Value::as_str)
        .map(|encoding| encoding == "newengine.ydd.runtime_mesh_parts.v1")
        .unwrap_or(false)
    {
        let part = select_ydd_runtime_mesh_part(&root, selector)?;
        return decode_ydd_runtime_mesh_part_for_skydome(dictionary_path, selector, part);
    }
    let part = select_ydd_mesh_part(&root, selector)?;
    let streams = part
        .get("vertex_streams")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "YDD mesh part has no vertex_streams object".to_owned())?;

    let positions = streams
        .get("position_f32x3")
        .ok_or_else(|| "YDD mesh part has no position_f32x3 stream".to_owned())
        .and_then(|v| json_array(v, "position_f32x3"))?;
    let normals = streams
        .get("normal_f32x3_generated_from_position")
        .or_else(|| streams.get("normal_f32x3"))
        .and_then(serde_json::Value::as_array);
    let uvs = streams
        .get("uv0_f32x2")
        .and_then(serde_json::Value::as_array);
    let indices_json = part
        .get("indices")
        .ok_or_else(|| "YDD mesh part has no indices array".to_owned())
        .and_then(|v| json_array(v, "indices"))?;

    decode_ydd_position_stream_mesh(
        dictionary_path,
        selector,
        positions,
        normals,
        uvs,
        indices_json,
    )
}

pub(super) fn select_ydd_runtime_mesh_part<'a>(
    root: &'a serde_json::Value,
    selector: Option<&str>,
) -> Result<&'a serde_json::Value, String> {
    let parts = root
        .get("runtime_mesh_parts")
        .ok_or_else(|| "YDD payload has no runtime_mesh_parts array".to_owned())
        .and_then(|v| json_array(v, "runtime_mesh_parts"))?;
    if parts.is_empty() {
        return Err("YDD payload has no runtime mesh parts".to_owned());
    }
    if let Some(selector) = selector {
        if let Some(part) = parts.iter().find(|part| {
            part.get("entry")
                .and_then(serde_json::Value::as_str)
                .map(|v| v.eq_ignore_ascii_case(selector))
                .unwrap_or(false)
                || part
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .map(|v| v.eq_ignore_ascii_case(selector))
                    .unwrap_or(false)
        }) {
            return Ok(part);
        }
        if parts.len() == 1 {
            return Ok(&parts[0]);
        }
        return Err(format!(
            "YDD selector '{selector}' was not found in runtime_mesh_parts"
        ));
    }
    Ok(&parts[0])
}

pub(super) fn decode_ydd_runtime_mesh_part_for_skydome(
    dictionary_path: &str,
    selector: Option<&str>,
    part: &serde_json::Value,
) -> Result<PrimitiveMesh, String> {
    let vertices_json = part
        .get("vertices")
        .ok_or_else(|| "YDD runtime mesh part has no vertices array".to_owned())
        .and_then(|v| json_array(v, "vertices"))?;
    let indices_json = part
        .get("indices")
        .ok_or_else(|| "YDD runtime mesh part has no indices array".to_owned())
        .and_then(|v| json_array(v, "indices"))?;
    if vertices_json.is_empty() || indices_json.is_empty() {
        return Err(format!(
            "YDD runtime mesh is empty path='{dictionary_path}' selector='{}' vertices={} indices={}",
            selector.unwrap_or("<first>"),
            vertices_json.len(),
            indices_json.len()
        ));
    }
    if vertices_json.len() > 1_000_000 || indices_json.len() > 6_000_000 {
        return Err(format!(
            "YDD runtime mesh exceeds runtime limits vertices={} indices={}",
            vertices_json.len(),
            indices_json.len()
        ));
    }

    let mut vertices = Vec::with_capacity(vertices_json.len());
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for value in vertices_json {
        let pos = value
            .get("pos")
            .map(|v| json_vec3(v, "vertices[].pos"))
            .transpose()?
            .unwrap_or([0.0, 0.0, 0.0]);
        let nrm = value
            .get("nrm")
            .map(|v| json_vec3(v, "vertices[].nrm"))
            .transpose()?
            .unwrap_or_else(|| {
                let p = Vec3::new(pos[0], pos[1], pos[2]);
                let n = if p.length_squared() > f32::EPSILON {
                    -p.normalize()
                } else {
                    Vec3::Y
                };
                [n.x, n.y, n.z]
            });
        let uv = value
            .get("uv")
            .map(|v| json_vec2(v, "vertices[].uv"))
            .transpose()?
            .unwrap_or([0.0, 0.0]);
        min.x = min.x.min(pos[0]);
        min.y = min.y.min(pos[1]);
        min.z = min.z.min(pos[2]);
        max.x = max.x.max(pos[0]);
        max.y = max.y.max(pos[1]);
        max.z = max.z.max(pos[2]);
        vertices.push(PrimitiveVertex { pos, nrm, uv });
    }

    decode_ydd_indexed_mesh_from_vertices(dictionary_path, vertices, indices_json, min, max)
}

pub(super) fn decode_ydd_position_stream_mesh(
    dictionary_path: &str,
    selector: Option<&str>,
    positions: &[serde_json::Value],
    normals: Option<&Vec<serde_json::Value>>,
    uvs: Option<&Vec<serde_json::Value>>,
    indices_json: &[serde_json::Value],
) -> Result<PrimitiveMesh, String> {
    if positions.is_empty() || indices_json.is_empty() {
        return Err(format!(
            "YDD mesh is empty path='{dictionary_path}' selector='{}' vertices={} indices={}",
            selector.unwrap_or("<first>"),
            positions.len(),
            indices_json.len()
        ));
    }
    if positions.len() > 1_000_000 || indices_json.len() > 6_000_000 {
        return Err(format!(
            "YDD mesh exceeds runtime limits vertices={} indices={}",
            positions.len(),
            indices_json.len()
        ));
    }

    let mut vertices = Vec::with_capacity(positions.len());
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);

    for (index, value) in positions.iter().enumerate() {
        let pos = json_vec3(value, "position_f32x3[]")?;
        let nrm = normals
            .and_then(|n| n.get(index))
            .map(|v| json_vec3(v, "normal_f32x3[]"))
            .transpose()?
            .unwrap_or_else(|| {
                let p = Vec3::new(pos[0], pos[1], pos[2]);
                let n = if p.length_squared() > f32::EPSILON {
                    -p.normalize()
                } else {
                    Vec3::Y
                };
                [n.x, n.y, n.z]
            });
        let uv = uvs
            .and_then(|uvs| uvs.get(index))
            .map(|v| json_vec2(v, "uv0_f32x2[]"))
            .transpose()?
            .unwrap_or([0.0, 0.0]);

        min.x = min.x.min(pos[0]);
        min.y = min.y.min(pos[1]);
        min.z = min.z.min(pos[2]);
        max.x = max.x.max(pos[0]);
        max.y = max.y.max(pos[1]);
        max.z = max.z.max(pos[2]);
        vertices.push(PrimitiveVertex { pos, nrm, uv });
    }

    decode_ydd_indexed_mesh_from_vertices(dictionary_path, vertices, indices_json, min, max)
}

pub(super) fn decode_ydd_indexed_mesh_from_vertices(
    dictionary_path: &str,
    vertices: Vec<PrimitiveVertex>,
    indices_json: &[serde_json::Value],
    min: Vec3,
    max: Vec3,
) -> Result<PrimitiveMesh, String> {
    let mut indices = Vec::with_capacity(indices_json.len());
    for value in indices_json {
        let index = value
            .as_u64()
            .ok_or_else(|| "YDD index must be an unsigned integer".to_owned())?
            as u32;
        if index as usize >= vertices.len() {
            return Err(format!(
                "YDD index out of bounds path='{dictionary_path}' index={index} vertex_count={}",
                vertices.len()
            ));
        }
        indices.push(index);
    }

    let bounds_center = (min + max) * 0.5;
    let mut bounds_radius = 0.0f32;
    for v in &vertices {
        let p = Vec3::new(v.pos[0], v.pos[1], v.pos[2]);
        bounds_radius = bounds_radius.max((p - bounds_center).length());
    }

    Ok(PrimitiveMesh {
        vertices,
        indices,
        bounds_center,
        bounds_radius: bounds_radius.max(0.001),
    })
}

pub(super) fn load_ydd_mesh_asset(logical_path: &str) -> Result<PrimitiveMesh, String> {
    let (dictionary_path, selector) = split_ydd_asset_ref(logical_path)
        .ok_or_else(|| format!("not a .ydd asset ref path='{logical_path}'"))?;
    let assets = AssetServiceClient::new(default_host_api());
    let request = AssetDecodeRequest {
        logical_path: dictionary_path.to_owned(),
        output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
        selector: selector
            .map(|selector| serde_json::json!({ "selector": selector }))
            .unwrap_or(serde_json::Value::Null),
    };
    let body = assets.decode_v1(&request).map_err(|e| {
        format!(
            "asset.decode_v1 failed path='{dictionary_path}' output='{ASSET_LIST_FILE_BODY_OUTPUT}' err='{e}'"
        )
    })?;
    decode_ydd_mesh(dictionary_path, selector, &body)
}

pub(super) fn load_skydome_mesh_asset(logical_path: &str) -> Result<PrimitiveMesh, String> {
    if split_ydd_asset_ref(logical_path).is_some() {
        load_ydd_mesh_asset(logical_path)
    } else {
        load_ne3d_mesh_asset(logical_path)
    }
}

pub(super) fn ensure_skydome_primitive(
    prims: &mut PrimitiveRegistry,
    logical_path: &str,
) -> Option<PrimitiveId> {
    let logical_path = logical_path.trim();
    if logical_path.is_empty() {
        newengine_ulog_api::ulog::info!(
            "game-ready: skydome skipped reason='no authored .ytyp sky mesh resolved' policy='.ytyp owns SkyDome description; no procedural fallback'"
        );
        return None;
    }
    if prims.is_registered(SKYDOME_PRIMITIVE_ID) {
        return Some(SKYDOME_PRIMITIVE_ID);
    }

    match load_skydome_mesh_asset(logical_path) {
        Ok(mesh) => {
            let vertex_count = mesh.vertices.len();
            let index_count = mesh.indices.len();
            prims.register_mesh(
                SKYDOME_PRIMITIVE_ID,
                format!("Imported/SkyDome ({logical_path})"),
                mesh,
            );
            newengine_ulog_api::ulog::info!(
                "game-ready: skydome imported through AssetManager path='{}' vertices={} indices={} policy='.ytyp -> .ydd/.ne3d -> engine.assets'",
                logical_path,
                vertex_count,
                index_count
            );
            Some(SKYDOME_PRIMITIVE_ID)
        }
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "game-ready: skydome mesh import failed path='{}' err='{}'; authored dome disabled policy='no procedural fallback; fix .ytyp/.ydd/.ne3d asset graph'",
                logical_path,
                e
            );
            None
        }
    }
}
