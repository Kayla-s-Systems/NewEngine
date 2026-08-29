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

pub(super) fn decode_ydd_mesh(
    dictionary_path: &str,
    selector: Option<&str>,
    payload: &[u8],
) -> Result<PrimitiveMesh, String> {
    let document = newengine_asset_format_nef8::ydd_binary::decode_ydd_binary_body(payload)
        .map_err(|error| {
            format!("binary YDD decode failed path='{dictionary_path}' err='{error}'")
        })?;
    let (_entry, source_mesh) = document.select_mesh(selector, true).map_err(|error| {
        format!("binary YDD selection failed path='{dictionary_path}' err='{error}'")
    })?;
    let vertices = source_mesh
        .vertices
        .iter()
        .map(|vertex| PrimitiveVertex {
            pos: vertex.position,
            nrm: vertex.normal,
            uv: vertex.uv0,
        })
        .collect::<Vec<_>>();
    let min = Vec3::new(
        source_mesh.bounds_min[0],
        source_mesh.bounds_min[1],
        source_mesh.bounds_min[2],
    );
    let max = Vec3::new(
        source_mesh.bounds_max[0],
        source_mesh.bounds_max[1],
        source_mesh.bounds_max[2],
    );
    let bounds_center = (min + max) * 0.5;
    let bounds_radius = vertices
        .iter()
        .map(|vertex| {
            let point = Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]);
            (point - bounds_center).length()
        })
        .fold(0.001_f32, f32::max);
    Ok(PrimitiveMesh {
        vertices,
        indices: source_mesh.indices.clone(),
        bounds_center,
        bounds_radius,
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
