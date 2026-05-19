#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime material mapping for imported model material sources.

use newengine_materials::{MaterialDescriptor, MaterialFlags, MaterialTextureBindings};
use newengine_model_domain_api::ModelMaterialBinding;
use newengine_model_import_obj::{normalize_logical_path, ModelMaterialSource};

pub fn material_binding(
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

pub fn runtime_texture_ref(path: &str, texture_dictionary: Option<&str>) -> Option<String> {
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

pub fn fallback_slot_color(material_slot: &str) -> [f32; 4] {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_dictionary_selector_is_derived() {
        let selector = runtime_texture_ref("player/abigail/textures/hair_diff_000_a_uni.dds", Some("player/abigail/textures/abigail.neytd"));
        assert_eq!(selector.as_deref(), Some("player/abigail/textures/abigail.neytd@hair_diff_000_a_uni"));
    }
}
