#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_assets::AssetServiceClient;
use newengine_materials::{MaterialDescriptor, MaterialFlags, MaterialTextureBindings};
use newengine_model_domain_api::{ModelCollisionKind, ModelCollisionRef, ModelConstructionManifest};
use newengine_math::Vec3;
use newengine_plugin_host::default_host_api;
use newengine_primitives::{PrimitiveMesh, PrimitiveVertex};
use serde::{Deserialize, Serialize};

/// Requested model asset hierarchy. All paths are AssetManager logical paths.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelAssetRequest {
    pub model: String,
    pub manifest: Option<String>,
    pub skeleton: Option<String>,
    pub texture_dictionary: Option<String>,
    pub collisions: Vec<ModelCollisionRef>,
    pub target_height: f32,
    pub eye_height_ratio: f32,
}

impl Default for ModelAssetRequest {
    fn default() -> Self {
        Self {
            model: String::new(),
            manifest: None,
            skeleton: None,
            texture_dictionary: None,
            collisions: Vec::new(),
            target_height: 1.8,
            eye_height_ratio: 0.91,
        }
    }
}

impl ModelAssetRequest {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            manifest: None,
            skeleton: None,
            texture_dictionary: None,
            collisions: Vec::new(),
            target_height: 1.8,
            eye_height_ratio: 0.91,
        }
    }

    pub fn with_manifest(mut self, manifest: impl Into<String>) -> Self {
        self.manifest = Some(manifest.into());
        self
    }

    pub fn with_skeleton(mut self, skeleton: impl Into<String>) -> Self {
        self.skeleton = Some(skeleton.into());
        self
    }

    pub fn with_texture_dictionary(mut self, dictionary: impl Into<String>) -> Self {
        self.texture_dictionary = Some(dictionary.into());
        self
    }

    pub fn with_collision(mut self, collision: ModelCollisionRef) -> Self {
        self.collisions.push(collision);
        self
    }

    pub fn with_human_scale(mut self, target_height: f32, eye_height_ratio: f32) -> Self {
        self.target_height = target_height;
        self.eye_height_ratio = eye_height_ratio;
        self
    }
}

/// Complete resolved runtime bundle returned by the adapter.
#[derive(Clone, Debug)]
pub struct ModelAssetBundle {
    pub source: String,
    pub parts: Vec<ModelMeshPart>,
    pub skeleton: Option<ModelSkeletonMetadata>,
    pub texture_dictionary: Option<String>,
    pub collisions: Vec<ModelCollisionRef>,
}

/// One mesh/material slot from a model.
#[derive(Clone, Debug)]
pub struct ModelMeshPart {
    pub material_slot: String,
    pub mesh: PrimitiveMesh,
    pub material: ModelMaterialBinding,
}

/// Renderer-neutral material binding for one model material slot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelMaterialBinding {
    pub slot: String,
    pub descriptor: MaterialDescriptor,
    pub textures: MaterialTextureBindings,
    pub fallback_color: [f32; 4],
}

/// Parsed, normalized material data before it is converted to runtime materials.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelMaterialSource {
    pub kd: [f32; 3],
    pub alpha: f32,
    pub ns: f32,
    pub base_color_texture: Option<String>,
    pub normal_texture: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelSkeletonJointMetadata {
    pub name: String,
    pub parent: Option<String>,
    pub position_ls: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelSkeletonAnchors {
    pub root: String,
    pub hips: String,
    pub head: String,
    pub left_hand: String,
    pub right_hand: String,
    pub left_foot: String,
    pub right_foot: String,
    pub eye: String,
    pub eye_height: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelSkeletonMetadata {
    pub source: String,
    pub source_format: String,
    pub container_magic: String,
    pub byte_len: usize,
    pub content_hash: String,
    pub decode_status: String,
    pub joints: Vec<ModelSkeletonJointMetadata>,
    pub anchors: ModelSkeletonAnchors,
}

#[derive(Clone)]
pub struct ModelAssetAdapter {
    client: AssetServiceClient,
}

impl Default for ModelAssetAdapter {
    fn default() -> Self { Self::new() }
}

impl ModelAssetAdapter {
    #[inline]
    pub fn new() -> Self {
        Self { client: AssetServiceClient::new(default_host_api()) }
    }

    #[inline]
    pub fn with_client(client: AssetServiceClient) -> Self {
        Self { client }
    }

    pub fn load_bundle(&self, request: &ModelAssetRequest) -> Result<ModelAssetBundle, String> {
        let request = self.resolve_request(request)?;
        let target_height = request.target_height.clamp(0.25, 3.0);
        let source = normalize_logical_path(&request.model, false)?;
        let texture_dictionary = request
            .texture_dictionary
            .as_deref()
            .map(|path| normalize_logical_path(path, false))
            .transpose()?
            .filter(|path| path.ends_with(".neytd"));

        let skeleton = match request.skeleton.as_deref() {
            Some(path) => Some(self.load_skeleton_metadata(path, target_height, request.eye_height_ratio)?),
            None => None,
        };

        let (obj_parts, materials) = self.decode_obj(&source, target_height)?;
        let mut parts = Vec::with_capacity(obj_parts.len());
        for part in obj_parts {
            let material = self.material_binding(
                &part.material_slot,
                materials.get(&part.material_slot),
                texture_dictionary.as_deref(),
            );
            parts.push(ModelMeshPart { material_slot: part.material_slot, mesh: part.mesh, material });
        }

        let collisions = if request.collisions.is_empty() {
            default_collisions_for_model(&skeleton, target_height)
        } else {
            request.collisions.clone()
        };

        Ok(ModelAssetBundle { source, parts, skeleton, texture_dictionary, collisions })
    }

    pub fn load_manifest(&self, logical_path: &str) -> Result<ModelConstructionManifest, String> {
        let source = normalize_logical_path(logical_path, false)?;
        let text = self.read_text(&source)?;
        serde_json::from_str::<ModelConstructionManifest>(&text)
            .map_err(|e| format!("model manifest parse failed path='{source}' err='{e}'"))
    }

    pub fn resolve_request(&self, request: &ModelAssetRequest) -> Result<ModelAssetRequest, String> {
        let Some(manifest_path) = request.manifest.as_deref() else { return Ok(request.clone()); };
        let manifest = self.load_manifest(manifest_path)?;
        let mut resolved = request.clone();
        if resolved.model.trim().is_empty() {
            resolved.model = manifest.model;
        }
        if resolved.skeleton.is_none() {
            resolved.skeleton = manifest.skeleton.map(|it| it.source);
        }
        if resolved.texture_dictionary.is_none() {
            resolved.texture_dictionary = manifest.material_set.texture_dictionary;
        }
        if resolved.collisions.is_empty() {
            resolved.collisions = manifest.collisions;
        }
        if (resolved.target_height - ModelAssetRequest::default().target_height).abs() < f32::EPSILON {
            resolved.target_height = manifest.target_height;
        }
        if (resolved.eye_height_ratio - ModelAssetRequest::default().eye_height_ratio).abs() < f32::EPSILON {
            resolved.eye_height_ratio = manifest.eye_height_ratio;
        }
        Ok(resolved)
    }

    pub fn load_skeleton_metadata(
        &self,
        logical_path: &str,
        target_height: f32,
        eye_height_ratio: f32,
    ) -> Result<ModelSkeletonMetadata, String> {
        let source = normalize_logical_path(logical_path, false)?;
        let bytes = self.read_bytes(&source)?;
        decode_ymt_skeleton_metadata(&source, &bytes, target_height, eye_height_ratio)
    }

    fn read_bytes(&self, logical_path: &str) -> Result<Vec<u8>, String> {
        let path = normalize_logical_path(logical_path, false)?;
        self.client
            .raw_bytes_v1(&path)
            .map_err(|e| format!("asset.raw_bytes_v1 failed path='{path}' err='{e}'"))
    }

    fn read_text(&self, logical_path: &str) -> Result<String, String> {
        let path = normalize_logical_path(logical_path, false)?;
        let bytes = self.read_bytes(&path)?;
        String::from_utf8(bytes).map_err(|e| format!("asset text is not UTF-8 path='{path}' err='{e}'"))
    }

    fn decode_obj(
        &self,
        logical_path: &str,
        target_height: f32,
    ) -> Result<(Vec<ObjPart>, std::collections::BTreeMap<String, ModelMaterialSource>), String> {
        let logical_path = normalize_logical_path(logical_path, false)?;
        let obj_text = self.read_text(&logical_path)?;

        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut uvs: Vec<[f32; 2]> = Vec::new();
        let mut mtllibs: Vec<String> = Vec::new();
        let mut current_material = "default".to_owned();
        let mut groups: std::collections::BTreeMap<String, ObjPartBuilder> = std::collections::BTreeMap::new();

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
        let materials = self.load_mtl_map(&logical_path, &mtllibs);
        Ok((parts, materials))
    }

    fn load_mtl_map(&self, obj_path: &str, mtllibs: &[String]) -> std::collections::BTreeMap<String, ModelMaterialSource> {
        let base = logical_dir(obj_path);
        let mut out = std::collections::BTreeMap::new();

        for rel in mtllibs {
            let Ok(path) = join_logical_path(base, rel) else {
                log::warn!("model adapter: MTL rejected relative='{}' base='{}'", rel, base);
                continue;
            };
            let Ok(text) = self.read_text(&path) else {
                log::warn!("model adapter: MTL unavailable through AssetManager path='{}'", path);
                continue;
            };
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
                            mat.base_color_texture = mtl_texture_path(base, &rest);
                        }
                    }
                    "norm" | "map_Bump" | "bump" => {
                        if let Some((_, mat)) = &mut current {
                            mat.normal_texture = mtl_texture_path(base, &rest);
                        }
                    }
                    _ => {}
                }
            }
            if let Some((name, mat)) = current.take() {
                out.insert(name, mat);
            }
        }

        out
    }

    fn material_binding(
        &self,
        material_slot: &str,
        parsed: Option<&ModelMaterialSource>,
        texture_dictionary: Option<&str>,
    ) -> ModelMaterialBinding {
        let mut color = parsed
            .map(|mat| {
                let authored_white = mat.kd.iter().all(|v| *v >= 0.92);
                if authored_white && mat.base_color_texture.is_some() {
                    fallback_slot_color(material_slot)
                } else {
                    [mat.kd[0], mat.kd[1], mat.kd[2], mat.alpha]
                }
            })
            .unwrap_or_else(|| fallback_slot_color(material_slot));
        for c in &mut color {
            *c = c.clamp(0.0, 1.0);
        }

        let roughness = parsed
            .map(|mat| (1.0 - (mat.ns / 512.0).clamp(0.0, 0.9)).clamp(0.28, 0.92))
            .unwrap_or(0.78);
        let alpha_flag = if color[3] < 0.99 { MaterialFlags::ALPHA_BLEND } else { MaterialFlags::NONE };
        let flags = MaterialFlags::DOUBLE_SIDED
            .union(MaterialFlags::CAST_SHADOWS)
            .union(MaterialFlags::RECEIVE_SHADOWS)
            .union(alpha_flag);

        let descriptor = MaterialDescriptor { base_color: color, roughness, flags, ..MaterialDescriptor::default() };
        let mut textures = MaterialTextureBindings::default();
        if let Some(texture) = parsed
            .and_then(|mat| mat.base_color_texture.as_deref())
            .and_then(|path| runtime_texture_ref(path, texture_dictionary))
        {
            textures.base_color_texture = Some(texture);
        }
        if let Some(texture) = parsed
            .and_then(|mat| mat.normal_texture.as_deref())
            .and_then(|path| runtime_texture_ref(path, texture_dictionary))
        {
            textures.normal_texture = Some(texture);
        }

        ModelMaterialBinding {
            slot: material_slot.to_owned(),
            descriptor,
            textures: textures.sanitized(),
            fallback_color: color,
        }
    }
}

impl Default for ModelMaterialSource {
    #[inline]
    fn default() -> Self {
        Self { kd: [0.82, 0.78, 0.72], alpha: 1.0, ns: 32.0, base_color_texture: None, normal_texture: None }
    }
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

#[derive(Clone, Debug)]
struct ObjPart {
    material_slot: String,
    mesh: PrimitiveMesh,
}


fn default_collisions_for_model(skeleton: &Option<ModelSkeletonMetadata>, target_height: f32) -> Vec<ModelCollisionRef> {
    let height = target_height.clamp(0.25, 3.0);
    let eye = skeleton.as_ref().map(|it| it.anchors.eye_height).unwrap_or(height * 0.91);
    let half_height = (eye * 0.48).clamp(0.28, height * 0.48);
    let radius = (height * 0.18).clamp(0.14, 0.42);
    vec![ModelCollisionRef {
        name: "humanoid.body".to_owned(),
        kind: ModelCollisionKind::Capsule,
        anchor: Some("hips".to_owned()),
        radius,
        half_height,
        half_extents: [radius, half_height, radius],
        mesh: None,
    }]
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
fn logical_dir(logical_path: &str) -> &str {
    logical_path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("")
}

fn join_logical_path(base_dir: &str, relative: &str) -> Result<String, String> {
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

fn runtime_texture_ref(path: &str, texture_dictionary: Option<&str>) -> Option<String> {
    let normalized = normalize_logical_path(path, true).ok()?;
    if normalized.contains(".neytd@") {
        return Some(normalized);
    }

    let (_, file) = normalized.rsplit_once('/').unwrap_or(("", normalized.as_str()));
    let stem = file.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(file).trim();
    if stem.is_empty() {
        return None;
    }

    if let Some(dictionary) = texture_dictionary {
        return Some(format!("{}@{}", dictionary, stem));
    }

    let (base, _) = normalized.rsplit_once('/').unwrap_or(("", normalized.as_str()));
    let fallback_dict = if base.is_empty() { "textures.neytd".to_owned() } else { format!("{}/textures.neytd", base) };
    Some(format!("{}@{}", fallback_dict, stem))
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

fn fallback_slot_color(material_slot: &str) -> [f32; 4] {
    let slot = material_slot.to_ascii_lowercase();
    if slot.contains("hair") {
        [0.16, 0.10, 0.08, 1.0]
    } else if slot.contains("skin") || slot.contains("head") || slot.contains("hand") {
        [0.76, 0.58, 0.48, 1.0]
    } else if slot.contains("lowr") {
        [0.16, 0.15, 0.14, 1.0]
    } else if slot.contains("uppr") {
        [0.42, 0.30, 0.23, 1.0]
    } else {
        [0.70, 0.66, 0.60, 1.0]
    }
}

fn decode_ymt_skeleton_metadata(
    source: &str,
    bytes: &[u8],
    target_height: f32,
    eye_height_ratio: f32,
) -> Result<ModelSkeletonMetadata, String> {
    if bytes.len() < 16 {
        return Err(format!("model skeleton ymt is too small source='{source}' bytes={}", bytes.len()));
    }

    let magic = std::str::from_utf8(&bytes[0..4]).unwrap_or("????").to_owned();
    if magic != "RSC7" {
        return Err(format!("unsupported model skeleton container source='{source}' magic='{magic}'"));
    }

    let target_height = target_height.clamp(0.25, 3.0);
    let eye_height = (target_height * eye_height_ratio.clamp(0.55, 0.98)).clamp(0.05, target_height);
    let hash = blake3::hash(bytes).to_hex().to_string();

    Ok(ModelSkeletonMetadata {
        source: source.to_owned(),
        source_format: "rockstar.ymt/rsc7".to_owned(),
        container_magic: magic,
        byte_len: bytes.len(),
        content_hash: format!("blake3:{hash}"),
        decode_status: "rsc7-container-detected; native skeleton payload kept opaque; humanoid anchors derived for runtime camera/attachment use".to_owned(),
        joints: humanoid_anchor_skeleton(target_height, eye_height),
        anchors: ModelSkeletonAnchors {
            root: "root".to_owned(),
            hips: "pelvis".to_owned(),
            head: "head".to_owned(),
            left_hand: "hand_l".to_owned(),
            right_hand: "hand_r".to_owned(),
            left_foot: "foot_l".to_owned(),
            right_foot: "foot_r".to_owned(),
            eye: "eye_center".to_owned(),
            eye_height,
        },
    })
}

fn joint(name: &str, parent: Option<&str>, x: f32, y: f32, z: f32) -> ModelSkeletonJointMetadata {
    ModelSkeletonJointMetadata { name: name.to_owned(), parent: parent.map(str::to_owned), position_ls: [x, y, z] }
}

fn humanoid_anchor_skeleton(height: f32, eye_height: f32) -> Vec<ModelSkeletonJointMetadata> {
    let h = height.max(0.25);
    vec![
        joint("root", None, 0.0, 0.0, 0.0),
        joint("pelvis", Some("root"), 0.0, h * 0.52, 0.0),
        joint("spine_01", Some("pelvis"), 0.0, h * 0.62, 0.0),
        joint("spine_02", Some("spine_01"), 0.0, h * 0.72, 0.0),
        joint("neck", Some("spine_02"), 0.0, h * 0.84, 0.0),
        joint("head", Some("neck"), 0.0, h * 0.92, 0.0),
        joint("eye_center", Some("head"), 0.0, eye_height, 0.06),
        joint("clavicle_l", Some("spine_02"), -0.12 * h, h * 0.78, 0.0),
        joint("upperarm_l", Some("clavicle_l"), -0.22 * h, h * 0.74, 0.0),
        joint("lowerarm_l", Some("upperarm_l"), -0.32 * h, h * 0.62, 0.0),
        joint("hand_l", Some("lowerarm_l"), -0.38 * h, h * 0.52, 0.0),
        joint("clavicle_r", Some("spine_02"), 0.12 * h, h * 0.78, 0.0),
        joint("upperarm_r", Some("clavicle_r"), 0.22 * h, h * 0.74, 0.0),
        joint("lowerarm_r", Some("upperarm_r"), 0.32 * h, h * 0.62, 0.0),
        joint("hand_r", Some("lowerarm_r"), 0.38 * h, h * 0.52, 0.0),
        joint("thigh_l", Some("pelvis"), -0.09 * h, h * 0.42, 0.0),
        joint("calf_l", Some("thigh_l"), -0.09 * h, h * 0.22, 0.0),
        joint("foot_l", Some("calf_l"), -0.09 * h, h * 0.03, 0.10 * h),
        joint("thigh_r", Some("pelvis"), 0.09 * h, h * 0.42, 0.0),
        joint("calf_r", Some("thigh_r"), 0.09 * h, h * 0.22, 0.0),
        joint("foot_r", Some("calf_r"), 0.09 * h, h * 0.03, 0.10 * h),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_dictionary_selector_is_derived() {
        let selector = runtime_texture_ref("player/abigail/textures/hair_diff_000_a_uni.dds", Some("player/abigail/textures/abigail.neytd"));
        assert_eq!(selector.as_deref(), Some("player/abigail/textures/abigail.neytd@hair_diff_000_a_uni"));
    }

    #[test]
    fn rejects_absolute_paths() {
        assert!(normalize_logical_path("C:/tmp/x.obj", false).is_err());
        assert!(normalize_logical_path("../x.obj", false).is_err());
    }
}
