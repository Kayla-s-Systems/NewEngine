#![forbid(unsafe_op_in_unsafe_fn)]

//! OBJ/MTL parsing for model construction.
//!
//! This crate is pure import logic: callers provide the OBJ text and a callback
//! for resolving MTL files through whichever asset service/provider is active.

use std::collections::BTreeMap;

use newengine_math::Vec3;
use newengine_primitives::{PrimitiveMesh, PrimitiveVertex};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelMaterialSource {
    pub kd: [f32; 3],
    pub alpha: f32,
    pub ns: f32,
    pub base_color_texture: Option<String>,
    pub normal_texture: Option<String>,
}

impl Default for ModelMaterialSource {
    #[inline]
    fn default() -> Self {
        Self { kd: [0.82, 0.78, 0.72], alpha: 1.0, ns: 32.0, base_color_texture: None, normal_texture: None }
    }
}

#[derive(Clone, Debug)]
pub struct ObjPart {
    pub material_slot: String,
    pub mesh: PrimitiveMesh,
}

#[derive(Clone, Debug)]
pub struct ObjDecodeResult {
    pub parts: Vec<ObjPart>,
    pub materials: BTreeMap<String, ModelMaterialSource>,
    pub mtllibs: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
struct ObjCorner {
    pos: usize,
    uv: Option<usize>,
    nrm: Option<usize>,
}

#[derive(Clone, Debug, Default)]
struct ObjPartBuilder {
    vertices: Vec<PrimitiveVertex>,
    indices: Vec<u32>,
}

pub fn decode_obj_with_mtl_loader<F>(
    logical_path: &str,
    obj_text: &str,
    target_height: f32,
    mut read_mtl: F,
) -> Result<ObjDecodeResult, String>
where
    F: FnMut(&str) -> Option<String>,
{
    let logical_path = normalize_logical_path(logical_path, false)?;

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut mtllibs: Vec<String> = Vec::new();
    let mut current_material = "default".to_owned();
    let mut groups: BTreeMap<String, ObjPartBuilder> = BTreeMap::new();

    for raw_line in obj_text.lines() {
        let line = raw_line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        let mut words = line.split_whitespace();
        let Some(tag) = words.next() else { continue; };
        match tag {
            "v" => {
                let x = words.next().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                let y = words.next().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                let z = words.next().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                positions.push([x, y, z]);
            }
            "vn" => {
                let x = words.next().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                let y = words.next().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.0);
                let z = words.next().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                normals.push([x, y, z]);
            }
            "vt" => {
                let u = words.next().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                let v = words.next().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
                uvs.push([u, 1.0 - v]);
            }
            "mtllib" => {
                for mtl in words {
                    mtllibs.push(mtl.to_owned());
                }
            }
            "usemtl" => {
                current_material = words.next().unwrap_or("default").trim().to_owned();
            }
            "f" => {
                let corners = words
                    .filter_map(|token| parse_face_corner(token, positions.len(), uvs.len(), normals.len()))
                    .collect::<Vec<_>>();
                if corners.len() < 3 {
                    continue;
                }
                let part = groups.entry(current_material.clone()).or_default();
                for i in 1..corners.len() - 1 {
                    push_triangle(part, [corners[0], corners[i], corners[i + 1]], &positions, &uvs, &normals);
                }
            }
            _ => {}
        }
    }

    let mut parts = groups
        .into_iter()
        .filter_map(|(material_slot, builder)| mesh_from_builder(builder).map(|mesh| ObjPart { material_slot, mesh }))
        .collect::<Vec<_>>();

    if parts.is_empty() {
        return Err(format!("model OBJ has no renderable faces path='{logical_path}'"));
    }

    normalize_parts(&mut parts, target_height);
    let materials = load_mtl_map(&logical_path, &mtllibs, &mut read_mtl);
    Ok(ObjDecodeResult { parts, materials, mtllibs })
}

pub fn parse_mtl_text(base_dir: &str, text: &str) -> BTreeMap<String, ModelMaterialSource> {
    let mut out = BTreeMap::new();
    let mut current: Option<(String, ModelMaterialSource)> = None;
    for raw_line in text.lines() {
        let line = raw_line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        let mut words = line.split_whitespace();
        let Some(tag) = words.next() else { continue; };
        let rest = words.collect::<Vec<_>>();
        match tag {
            "newmtl" => {
                if let Some((name, mat)) = current.take() {
                    out.insert(name, mat);
                }
                current = Some((rest.first().copied().unwrap_or("default").to_owned(), ModelMaterialSource::default()));
            }
            "Kd" => {
                if let Some((_, mat)) = &mut current {
                    mat.kd = [
                        rest.get(0).and_then(|v| v.parse::<f32>().ok()).unwrap_or(mat.kd[0]),
                        rest.get(1).and_then(|v| v.parse::<f32>().ok()).unwrap_or(mat.kd[1]),
                        rest.get(2).and_then(|v| v.parse::<f32>().ok()).unwrap_or(mat.kd[2]),
                    ];
                }
            }
            "d" => {
                if let Some((_, mat)) = &mut current {
                    mat.alpha = rest.first().and_then(|v| v.parse::<f32>().ok()).unwrap_or(mat.alpha);
                }
            }
            "Ns" => {
                if let Some((_, mat)) = &mut current {
                    mat.ns = rest.first().and_then(|v| v.parse::<f32>().ok()).unwrap_or(mat.ns);
                }
            }
            "map_Kd" => {
                if let Some((_, mat)) = &mut current {
                    mat.base_color_texture = mtl_texture_path(base_dir, &rest);
                }
            }
            "norm" | "map_Bump" | "bump" => {
                if let Some((_, mat)) = &mut current {
                    mat.normal_texture = mtl_texture_path(base_dir, &rest);
                }
            }
            _ => {}
        }
    }
    if let Some((name, mat)) = current.take() {
        out.insert(name, mat);
    }
    out
}

fn load_mtl_map<F>(obj_path: &str, mtllibs: &[String], read_mtl: &mut F) -> BTreeMap<String, ModelMaterialSource>
where
    F: FnMut(&str) -> Option<String>,
{
    let base = logical_dir(obj_path);
    let mut out = BTreeMap::new();

    for rel in mtllibs {
        let Ok(path) = join_logical_path(base, rel) else {
            newengine_ulog_api::ulog::warn!("model import obj: MTL rejected relative='{}' base='{}'", rel, base);
            continue;
        };
        let Some(text) = read_mtl(&path) else {
            newengine_ulog_api::ulog::warn!("model import obj: MTL unavailable path='{}'", path);
            continue;
        };
        out.extend(parse_mtl_text(base, &text));
    }

    out
}

pub fn normalize_logical_path(raw: &str, allow_selector: bool) -> Result<String, String> {
    let trimmed = raw.trim().replace('\\', "/");
    if trimmed.is_empty() {
        return Err("empty asset path".to_owned());
    }
    if !allow_selector && trimmed.contains('@') {
        return Err(format!("texture selector is not allowed for asset path '{raw}'"));
    }

    let (path, selector) = if allow_selector {
        match trimmed.split_once('@') {
            Some((path, selector)) => (path, Some(selector)),
            None => (trimmed.as_str(), None),
        }
    } else {
        (trimmed.as_str(), None)
    };

    if path.is_empty() || path.starts_with('/') || path.contains(':') {
        return Err(format!("invalid logical asset path '{raw}'"));
    }

    let mut parts = Vec::new();
    for part in path.split('/') {
        let part = part.trim();
        if part.is_empty() || part == "." || part == ".." {
            return Err(format!("invalid logical asset path '{raw}'"));
        }
        parts.push(part);
    }

    let normalized = parts.join("/");
    if let Some(selector) = selector {
        let selector = selector.trim();
        if selector.is_empty() || selector.contains('@') || selector.contains('/') || selector.contains('\\') || selector.contains(':') {
            return Err(format!("invalid logical texture selector '{raw}'"));
        }
        Ok(format!("{}@{}", normalized, selector))
    } else {
        Ok(normalized)
    }
}

#[inline]
pub fn logical_dir(logical_path: &str) -> &str {
    logical_path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("")
}

pub fn join_logical_path(base_dir: &str, relative: &str) -> Result<String, String> {
    let rel = normalize_logical_path(relative, true)?;
    if base_dir.trim().is_empty() {
        Ok(rel)
    } else {
        Ok(format!("{}/{}", base_dir.trim_end_matches('/'), rel))
    }
}

fn mtl_texture_path(base_dir: &str, tokens: &[&str]) -> Option<String> {
    let raw = tokens
        .iter()
        .rev()
        .find(|token| !token.trim().is_empty() && !token.starts_with('-'))?;
    join_logical_path(base_dir, raw).ok()
}

fn parse_obj_index(raw: &str, len: usize) -> Option<usize> {
    let idx = raw.trim().parse::<isize>().ok()?;
    if idx > 0 {
        let zero_based = (idx as usize).checked_sub(1)?;
        (zero_based < len).then_some(zero_based)
    } else if idx < 0 {
        let resolved = len as isize + idx;
        (resolved >= 0 && (resolved as usize) < len).then_some(resolved as usize)
    } else {
        None
    }
}

fn parse_face_corner(token: &str, pos_len: usize, uv_len: usize, nrm_len: usize) -> Option<ObjCorner> {
    let mut it = token.split('/');
    let pos = parse_obj_index(it.next()?, pos_len)?;
    let uv = it.next().and_then(|v| if v.trim().is_empty() { None } else { parse_obj_index(v, uv_len) });
    let nrm = it.next().and_then(|v| if v.trim().is_empty() { None } else { parse_obj_index(v, nrm_len) });
    Some(ObjCorner { pos, uv, nrm })
}

#[inline]
fn vertex(corner: ObjCorner, positions: &[[f32; 3]], uvs: &[[f32; 2]], normals: &[[f32; 3]], fallback_normal: [f32; 3]) -> PrimitiveVertex {
    PrimitiveVertex {
        pos: positions[corner.pos],
        nrm: corner.nrm.and_then(|ix| normals.get(ix).copied()).unwrap_or(fallback_normal),
        uv: corner.uv.and_then(|ix| uvs.get(ix).copied()).unwrap_or([0.0, 0.0]),
    }
}

fn push_triangle(part: &mut ObjPartBuilder, tri: [ObjCorner; 3], positions: &[[f32; 3]], uvs: &[[f32; 2]], normals: &[[f32; 3]]) {
    let a = Vec3::new(positions[tri[0].pos][0], positions[tri[0].pos][1], positions[tri[0].pos][2]);
    let b = Vec3::new(positions[tri[1].pos][0], positions[tri[1].pos][1], positions[tri[1].pos][2]);
    let c = Vec3::new(positions[tri[2].pos][0], positions[tri[2].pos][1], positions[tri[2].pos][2]);
    let n = (b - a).cross(c - a).normalize_or_zero();
    let fallback = if n.length_squared() > 0.0 { [n.x, n.y, n.z] } else { [0.0, 1.0, 0.0] };

    for corner in tri {
        let ix = part.vertices.len() as u32;
        part.vertices.push(vertex(corner, positions, uvs, normals, fallback));
        part.indices.push(ix);
    }
}

fn mesh_from_builder(mut builder: ObjPartBuilder) -> Option<PrimitiveMesh> {
    if builder.vertices.is_empty() || builder.indices.is_empty() {
        return None;
    }

    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for v in &builder.vertices {
        let p = Vec3::new(v.pos[0], v.pos[1], v.pos[2]);
        min = min.min(p);
        max = max.max(p);
    }
    let center = (min + max) * 0.5;
    let mut radius = 0.0f32;
    for v in &builder.vertices {
        let p = Vec3::new(v.pos[0], v.pos[1], v.pos[2]);
        radius = radius.max((p - center).length());
    }

    for index in &mut builder.indices {
        *index = (*index).min(builder.vertices.len().saturating_sub(1) as u32);
    }

    Some(PrimitiveMesh { vertices: builder.vertices, indices: builder.indices, bounds_center: center, bounds_radius: radius.max(0.001) })
}

fn normalize_parts(parts: &mut [ObjPart], target_height: f32) {
    let target_height = target_height.clamp(0.25, 3.0);
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);

    for part in parts.iter() {
        for v in &part.mesh.vertices {
            let p = Vec3::new(v.pos[0], v.pos[1], v.pos[2]);
            min = min.min(p);
            max = max.max(p);
        }
    }

    let height = (max.y - min.y).abs();
    if !height.is_finite() || height <= 0.0001 {
        return;
    }

    let scale = target_height / height;
    let origin = Vec3::new((min.x + max.x) * 0.5, min.y, (min.z + max.z) * 0.5);

    for part in parts.iter_mut() {
        let mut mesh_min = Vec3::splat(f32::INFINITY);
        let mut mesh_max = Vec3::splat(f32::NEG_INFINITY);
        for v in &mut part.mesh.vertices {
            let p = (Vec3::new(v.pos[0], v.pos[1], v.pos[2]) - origin) * scale;
            v.pos = [p.x, p.y, p.z];
            mesh_min = mesh_min.min(p);
            mesh_max = mesh_max.max(p);
        }
        let center = (mesh_min + mesh_max) * 0.5;
        let mut radius = 0.0f32;
        for v in &part.mesh.vertices {
            let p = Vec3::new(v.pos[0], v.pos[1], v.pos[2]);
            radius = radius.max((p - center).length());
        }
        part.mesh.bounds_center = center;
        part.mesh.bounds_radius = radius.max(0.001);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_absolute_paths() {
        assert!(normalize_logical_path("C:/tmp/x.obj", false).is_err());
        assert!(normalize_logical_path("../x.obj", false).is_err());
    }
}
