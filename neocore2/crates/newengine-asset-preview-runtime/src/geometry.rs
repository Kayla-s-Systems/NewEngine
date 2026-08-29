use super::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct PreviewGeometryNormalization {
    pub(super) source_center: Vec3,
    pub(super) source_extent: Vec3,
    pub(super) scale: f32,
}

pub(super) fn normalize_preview_geometry(
    parts: &mut [newengine_model_domain_api::ModelMeshPart],
) -> Option<PreviewGeometryNormalization> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut vertex_count = 0usize;
    for part in parts.iter() {
        for vertex in &part.mesh.vertices {
            let position = Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]);
            if !position.is_finite() {
                continue;
            }
            min = min.min(position);
            max = max.max(position);
            vertex_count += 1;
        }
    }
    if vertex_count == 0 || !min.is_finite() || !max.is_finite() {
        return None;
    }
    let center = (min + max) * 0.5;
    let extent = max - min;
    let max_extent = extent.x.max(extent.y).max(extent.z).max(0.001);
    let scale = 2.2 / max_extent;

    for part in parts.iter_mut() {
        let mut part_min = Vec3::splat(f32::INFINITY);
        let mut part_max = Vec3::splat(f32::NEG_INFINITY);
        for vertex in &mut part.mesh.vertices {
            let source = Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]);
            if !source.is_finite() {
                continue;
            }
            let normalized = (source - center) * scale;
            vertex.pos = [normalized.x, normalized.y, normalized.z];
            part_min = part_min.min(normalized);
            part_max = part_max.max(normalized);
        }
        if part_min.is_finite() && part_max.is_finite() {
            let part_center = (part_min + part_max) * 0.5;
            let mut radius = 0.001_f32;
            for vertex in &part.mesh.vertices {
                let position = Vec3::new(vertex.pos[0], vertex.pos[1], vertex.pos[2]);
                if position.is_finite() {
                    radius = radius.max(position.distance(part_center));
                }
            }
            part.mesh.bounds_center = part_center;
            part.mesh.bounds_radius = radius;
        }
    }

    Some(PreviewGeometryNormalization {
        source_center: center,
        source_extent: extent,
        scale,
    })
}

pub(super) fn scene_preview_snapshot(
    asset_ref: &str,
    width: u32,
    height: u32,
) -> AssetPreviewSnapshot {
    AssetPreviewSnapshot {
        asset_ref: asset_ref.to_owned(),
        kind: AssetPreviewKind::Scene3d,
        ready: false,
        texture_ref: None,
        ui_texture_id: None,
        width,
        height,
        diagnostic: None,
    }
}

pub(super) fn texture_dimensions(
    document: &AssetDocument,
    metadata: Option<&std::collections::BTreeMap<String, String>>,
) -> (u32, u32) {
    let metadata_dimension = |keys: &[&str]| {
        metadata.and_then(|metadata| {
            keys.iter().find_map(|key| {
                metadata
                    .get(*key)
                    .and_then(|value| value.trim().parse::<u32>().ok())
            })
        })
    };
    let document_dimension = |keys: &[&str]| {
        document
            .sections
            .iter()
            .flat_map(|section| section.fields.iter())
            .find_map(|field| {
                let id = field.id.trim().to_ascii_lowercase();
                let label = field.label.trim().to_ascii_lowercase();
                if !keys.iter().any(|key| id == *key || label == *key) {
                    return None;
                }
                field.value.as_u64().map(|value| value as u32).or_else(|| {
                    field
                        .value
                        .as_str()
                        .and_then(|value| value.parse::<u32>().ok())
                })
            })
    };
    (
        metadata_dimension(&["width", "texture_width", "w"])
            .or_else(|| document_dimension(&["width", "texture_width"]))
            .unwrap_or(0),
        metadata_dimension(&["height", "texture_height", "h"])
            .or_else(|| document_dimension(&["height", "texture_height"]))
            .unwrap_or(0),
    )
}

pub(super) fn asset_extension(asset_ref: &str) -> &str {
    asset_ref
        .split('@')
        .next()
        .and_then(|path| path.rsplit_once('.').map(|(_, extension)| extension))
        .unwrap_or_default()
}

pub(super) fn material_texture_refs(binding: &ModelMaterialBinding) -> Vec<String> {
    let mut refs = [
        binding.textures.base_color_texture.as_deref(),
        binding.textures.normal_texture.as_deref(),
        binding.textures.roughness_texture.as_deref(),
        binding.textures.metallic_texture.as_deref(),
        binding.textures.occlusion_texture.as_deref(),
        binding.textures.emissive_texture.as_deref(),
    ]
    .into_iter()
    .flatten()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    refs.sort();
    refs.dedup();
    refs
}

pub(super) fn require_complete_graph(graph: &ResolvedAssetGraphV2) -> Result<(), String> {
    if graph.missing_refs.is_empty() && graph.cycle_errors.is_empty() {
        return Ok(());
    }
    Err(format!(
        "asset dependency graph is incomplete root='{}' missing={} cycles={}",
        graph.root_ref,
        graph.missing_refs.len(),
        graph.cycle_errors.len()
    ))
}
