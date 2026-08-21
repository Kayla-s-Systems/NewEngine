use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{CanonicalProjection, ToolRuntimeConformanceSpec};

pub const CANONICAL_ASSET_DTO_SCHEMA: &str = "northstar.tool_runtime.canonical_dto.v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanonicalAssetDtoV1 {
    pub schema: String,
    pub format: String,
    pub entries: Vec<CanonicalAssetEntryV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanonicalAssetEntryV1 {
    pub name: String,
    pub stable_hash: u64,
    pub kind: String,
    pub entry_ref: String,
    pub dependencies: Vec<String>,
    pub mesh_count: Option<u32>,
    pub material_slots: Vec<String>,
    pub bounds_min: Option<[f32; 3]>,
    pub bounds_max: Option<[f32; 3]>,
    pub properties_ref: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub mip_count: Option<u32>,
    pub pixel_format: Option<String>,
    pub color_space: Option<String>,
    pub shader: Option<String>,
    pub blend: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DtoParityReport {
    pub spec_id: String,
    pub projection: String,
    pub canonical: CanonicalAssetDtoV1,
}

fn base_entry(
    name: String,
    stable_hash: u64,
    kind: String,
    entry_ref: String,
) -> CanonicalAssetEntryV1 {
    CanonicalAssetEntryV1 {
        name,
        stable_hash,
        kind,
        entry_ref,
        dependencies: Vec::new(),
        mesh_count: None,
        material_slots: Vec::new(),
        bounds_min: None,
        bounds_max: None,
        properties_ref: None,
        width: None,
        height: None,
        mip_count: None,
        pixel_format: None,
        color_space: None,
        shader: None,
        blend: None,
    }
}

fn normalize_path(value: &str) -> String {
    let mut out = value.trim().replace('\\', "/");
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    out.trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned()
}

fn normalize_optional_ref(value: Option<String>) -> Option<String> {
    value
        .map(|value| normalize_path(&value))
        .filter(|value| !value.is_empty())
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn normalize_strings<I>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    values
        .into_iter()
        .map(|value| normalize_path(&value))
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalize_slots<I>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn finish(format: &str, mut entries: Vec<CanonicalAssetEntryV1>) -> CanonicalAssetDtoV1 {
    entries.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then_with(|| a.stable_hash.cmp(&b.stable_hash))
            .then_with(|| a.entry_ref.cmp(&b.entry_ref))
    });
    CanonicalAssetDtoV1 {
        schema: CANONICAL_ASSET_DTO_SCHEMA.to_owned(),
        format: format.to_owned(),
        entries,
    }
}

fn ytyp_asset_manager(logical_path: &str, bytes: &[u8]) -> Result<CanonicalAssetDtoV1, String> {
    let manifest: newengine_assets_api::AssetFileManifest = serde_json::from_slice(bytes)
        .map_err(|error| format!("YTYP AssetManager DTO decode failed: {error}"))?;
    let logical_path = normalize_path(logical_path);
    let entries = manifest
        .entries
        .into_iter()
        .map(|entry| {
            let stable_hash = u64::from_str_radix(entry.stable_id.trim(), 16).map_err(|error| {
                format!(
                    "YTYP AssetManager entry '{}' has invalid stable_id='{}': {error}",
                    entry.name, entry.stable_id
                )
            })?;
            let dependencies = normalize_strings(
                entry
                    .dependencies
                    .into_iter()
                    .map(|dependency| dependency.reference),
            );
            let entry_ref = if entry.entry_ref.trim().is_empty() {
                newengine_assets_api::entry_ref(&logical_path, &entry.name)
            } else {
                entry.entry_ref.clone()
            };
            let mut out = base_entry(
                entry.name.trim().to_owned(),
                stable_hash,
                entry.asset_kind.trim().to_owned(),
                normalize_path(&entry_ref),
            );
            out.dependencies = dependencies;
            Ok(out)
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(finish("ytyp", entries))
}

fn ytyp_runtime(logical_path: &str, bytes: &[u8]) -> Result<CanonicalAssetDtoV1, String> {
    let entries: Vec<newengine_definitions_runtime::DefinitionEntryV1> =
        serde_json::from_slice(bytes)
            .map_err(|error| format!("YTYP runtime DTO decode failed: {error}"))?;
    let logical_path = normalize_path(logical_path);
    let entries = entries
        .into_iter()
        .map(|entry| {
            let refs = entry.refs;
            let dependencies = normalize_strings(
                refs.drawable_refs
                    .into_iter()
                    .chain(refs.material_refs)
                    .chain(refs.texture_refs)
                    .chain(refs.uv_layout_refs)
                    .chain(refs.physics_refs)
                    .chain(refs.collision_refs)
                    .chain(refs.ai_refs)
                    .chain(refs.streaming_refs)
                    .chain(refs.editor_refs)
                    .chain(refs.other_refs),
            );
            let entry_ref = if entry.identity.definition_ref.trim().is_empty() {
                newengine_assets_api::entry_ref(&logical_path, &entry.identity.name)
            } else {
                entry.identity.definition_ref.clone()
            };
            let mut out = base_entry(
                entry.identity.name.trim().to_owned(),
                entry.stable_hash,
                entry.kind.trim().to_owned(),
                normalize_path(&entry_ref),
            );
            out.dependencies = dependencies;
            out
        })
        .collect();
    Ok(finish("ytyp", entries))
}

fn ydd_asset_manager(logical_path: &str, bytes: &[u8]) -> Result<CanonicalAssetDtoV1, String> {
    let manifest: newengine_model_domain_api::DrawableDictionaryManifest =
        serde_json::from_slice(bytes)
            .map_err(|error| format!("YDD AssetManager DTO decode failed: {error}"))?;
    let logical_path = normalize_path(logical_path);
    let entries = manifest
        .entries
        .into_iter()
        .map(|entry| {
            let mut out = base_entry(
                entry.name.trim().to_owned(),
                entry.name_hash,
                newengine_model_domain_api::DRAWABLE_DICTIONARY_ASSET_KIND.to_owned(),
                newengine_assets_api::entry_ref(&logical_path, &entry.name),
            );
            out.dependencies = normalize_strings(
                entry
                    .dependency_refs
                    .into_iter()
                    .chain(entry.skeleton_refs)
                    .chain(entry.collision_refs)
                    .chain(entry.lods),
            );
            out.mesh_count = Some(entry.mesh_count);
            out.material_slots =
                normalize_slots(entry.material_slots.into_iter().map(|slot| slot.slot_name));
            out.bounds_min = Some(entry.bounds_min);
            out.bounds_max = Some(entry.bounds_max);
            out.properties_ref = normalize_optional_ref(entry.properties_ref);
            out
        })
        .collect();
    Ok(finish("ydd", entries))
}

fn ydd_runtime(logical_path: &str, bytes: &[u8]) -> Result<CanonicalAssetDtoV1, String> {
    let document: newengine_asset_format_nef8::ydd_binary::YddBinaryDocument =
        serde_json::from_slice(bytes)
            .map_err(|error| format!("YDD runtime DTO decode failed: {error}"))?;
    let logical_path = normalize_path(logical_path);
    let entries = document
        .entries
        .into_iter()
        .map(|entry| {
            let mut out = base_entry(
                entry.name.trim().to_owned(),
                newengine_assets_api::stable_hash_from_text(&entry.name),
                newengine_model_domain_api::DRAWABLE_DICTIONARY_ASSET_KIND.to_owned(),
                newengine_assets_api::entry_ref(&logical_path, &entry.name),
            );
            out.mesh_count = Some(entry.meshes.len() as u32);
            out.material_slots =
                normalize_slots(entry.meshes.iter().map(|mesh| mesh.material_slot()));
            out.bounds_min = Some(entry.bounds_min);
            out.bounds_max = Some(entry.bounds_max);
            out.properties_ref = normalize_optional_ref(entry.properties_ref);
            out
        })
        .collect();
    Ok(finish("ydd", entries))
}

fn ytd_asset_manager(logical_path: &str, bytes: &[u8]) -> Result<CanonicalAssetDtoV1, String> {
    let manifest: newengine_model_domain_api::TextureDictionaryManifest =
        serde_json::from_slice(bytes)
            .map_err(|error| format!("YTD AssetManager DTO decode failed: {error}"))?;
    let logical_path = normalize_path(logical_path);
    let entries = manifest
        .entries
        .into_iter()
        .map(|entry| {
            let mut out = base_entry(
                entry.name.trim().to_owned(),
                entry.name_hash,
                newengine_model_domain_api::TEXTURE_DICTIONARY_ASSET_KIND.to_owned(),
                newengine_assets_api::entry_ref(&logical_path, &entry.name),
            );
            out.width = Some(entry.width);
            out.height = Some(entry.height);
            out.mip_count = Some(entry.mip_count);
            out.pixel_format = normalize_optional_text(Some(entry.pixel_format));
            out.color_space = normalize_optional_text(Some(entry.color_space));
            out
        })
        .collect();
    Ok(finish("ytd", entries))
}

fn ytd_runtime(logical_path: &str, bytes: &[u8]) -> Result<CanonicalAssetDtoV1, String> {
    let manifest: newengine_texture_container::TextureDictionaryManifest =
        serde_json::from_slice(bytes)
            .map_err(|error| format!("YTD runtime DTO decode failed: {error}"))?;
    let logical_path = normalize_path(logical_path);
    let entries = manifest
        .entries
        .into_iter()
        .map(|entry| {
            let mut out = base_entry(
                entry.name.trim().to_owned(),
                entry.name_hash,
                newengine_model_domain_api::TEXTURE_DICTIONARY_ASSET_KIND.to_owned(),
                newengine_assets_api::entry_ref(&logical_path, &entry.name),
            );
            out.width = Some(entry.width);
            out.height = Some(entry.height);
            out.mip_count = Some(entry.mip_count);
            out.pixel_format = normalize_optional_text(Some(entry.format));
            out.color_space = normalize_optional_text(Some(entry.color_space));
            out
        })
        .collect();
    Ok(finish("ytd", entries))
}

fn nemat_asset_manager(logical_path: &str, bytes: &[u8]) -> Result<CanonicalAssetDtoV1, String> {
    let manifest: newengine_assets_api::AssetFileManifest = serde_json::from_slice(bytes)
        .map_err(|error| format!("NEMAT AssetManager DTO decode failed: {error}"))?;
    let logical_path = normalize_path(logical_path);
    let entries = manifest
        .entries
        .into_iter()
        .map(|entry| {
            let stable_hash = u64::from_str_radix(entry.stable_id.trim(), 16).map_err(|error| {
                format!(
                    "NEMAT AssetManager entry '{}' has invalid stable_id='{}': {error}",
                    entry.name, entry.stable_id
                )
            })?;
            let mut out = base_entry(
                entry.name.trim().to_owned(),
                stable_hash,
                "newengine.asset.material".to_owned(),
                newengine_assets_api::entry_ref(&logical_path, &entry.name),
            );
            out.dependencies =
                normalize_strings(entry.dependencies.into_iter().map(|d| d.reference));
            out.shader = normalize_optional_text(entry.metadata.get("shader").cloned());
            out.blend = normalize_optional_text(entry.metadata.get("blend").cloned());
            Ok(out)
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(finish("nemat", entries))
}

fn nemat_runtime(logical_path: &str, bytes: &[u8]) -> Result<CanonicalAssetDtoV1, String> {
    let library: newengine_materials::AuthoredMaterialLibrary = serde_json::from_slice(bytes)
        .map_err(|error| format!("NEMAT runtime DTO decode failed: {error}"))?;
    let logical_path = normalize_path(logical_path);
    let entries = library
        .materials
        .into_iter()
        .map(|material| {
            let mut dependencies = material.textures.values().cloned().collect::<Vec<_>>();
            dependencies.extend(material.params.values().filter_map(|value| match value {
                newengine_materials::MaterialParamValue::TextureRef(reference) => {
                    Some(reference.clone())
                }
                _ => None,
            }));
            let mut out = base_entry(
                material.name.trim().to_owned(),
                newengine_assets_api::stable_hash_from_text(&material.name),
                "newengine.asset.material".to_owned(),
                newengine_assets_api::entry_ref(&logical_path, &material.name),
            );
            out.dependencies = normalize_strings(dependencies);
            out.shader = normalize_optional_text(Some(material.shader));
            out.blend = normalize_optional_text(Some(material.surface.blend));
            out
        })
        .collect();
    Ok(finish("nemat", entries))
}

fn neui_asset_manager(logical_path: &str, bytes: &[u8]) -> Result<CanonicalAssetDtoV1, String> {
    let manifest: newengine_assets_api::AssetFileManifest = serde_json::from_slice(bytes)
        .map_err(|error| format!("NEUI AssetManager DTO decode failed: {error}"))?;
    let logical_path = normalize_path(logical_path);
    let entries = manifest
        .entries
        .into_iter()
        .map(|entry| {
            let stable_hash = u64::from_str_radix(entry.stable_id.trim(), 16).map_err(|error| {
                format!(
                    "NEUI AssetManager entry '{}' has invalid stable_id='{}': {error}",
                    entry.name, entry.stable_id
                )
            })?;
            Ok(base_entry(
                entry.name.trim().to_owned(),
                stable_hash,
                entry.asset_kind.trim().to_owned(),
                newengine_assets_api::entry_ref(&logical_path, &entry.name),
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(finish("neui", entries))
}

fn neui_runtime(logical_path: &str, bytes: &[u8]) -> Result<CanonicalAssetDtoV1, String> {
    let root: newengine_ui_api::UiNodeRequest = serde_json::from_slice(bytes)
        .map_err(|error| format!("NEUI runtime DTO decode failed: {error}"))?;
    let logical_path = normalize_path(logical_path);
    let source_ref = root
        .source_span
        .as_ref()
        .map(|span| span.source_ref.as_str())
        .unwrap_or_default();
    let selector = source_ref
        .rsplit_once('@')
        .map(|(_, selector)| selector.trim())
        .filter(|selector| !selector.is_empty())
        .unwrap_or("surface");
    let kind = if root.component_id.eq_ignore_ascii_case("surface")
        || root.role.eq_ignore_ascii_case("surface")
    {
        "ui_surface"
    } else {
        "ui_entry"
    };
    let entry = base_entry(
        selector.to_owned(),
        newengine_assets_api::stable_hash_from_text(selector),
        kind.to_owned(),
        newengine_assets_api::entry_ref(&logical_path, selector),
    );
    Ok(finish("neui", vec![entry]))
}

pub fn project_asset_manager_native_dto(
    projection: CanonicalProjection,
    logical_path: &str,
    bytes: &[u8],
) -> Result<CanonicalAssetDtoV1, String> {
    match projection {
        CanonicalProjection::YtypDefinitionEntriesV1 => ytyp_asset_manager(logical_path, bytes),
        CanonicalProjection::YddDrawableDictionaryV1 => ydd_asset_manager(logical_path, bytes),
        CanonicalProjection::YtdTextureDictionaryV1 => ytd_asset_manager(logical_path, bytes),
        CanonicalProjection::NematMaterialLibraryV1 => nemat_asset_manager(logical_path, bytes),
        CanonicalProjection::NeuiSelectorSurfaceV1 => neui_asset_manager(logical_path, bytes),
    }
}

pub fn project_runtime_native_dto(
    projection: CanonicalProjection,
    logical_path: &str,
    bytes: &[u8],
) -> Result<CanonicalAssetDtoV1, String> {
    match projection {
        CanonicalProjection::YtypDefinitionEntriesV1 => ytyp_runtime(logical_path, bytes),
        CanonicalProjection::YddDrawableDictionaryV1 => ydd_runtime(logical_path, bytes),
        CanonicalProjection::YtdTextureDictionaryV1 => ytd_runtime(logical_path, bytes),
        CanonicalProjection::NematMaterialLibraryV1 => nemat_runtime(logical_path, bytes),
        CanonicalProjection::NeuiSelectorSurfaceV1 => neui_runtime(logical_path, bytes),
    }
}

pub fn validate_native_dto_parity(
    spec: &ToolRuntimeConformanceSpec,
    logical_path: &str,
    asset_manager_json: &[u8],
    runtime_json: &[u8],
) -> Result<DtoParityReport, Vec<String>> {
    let Some(projection) = spec.canonical_projection else {
        return Err(vec![format!(
            "tool/runtime '{}' does not declare canonical DTO projection",
            spec.id
        )]);
    };
    let asset_manager =
        project_asset_manager_native_dto(projection, logical_path, asset_manager_json)
            .map_err(|error| vec![error])?;
    let runtime = project_runtime_native_dto(projection, logical_path, runtime_json)
        .map_err(|error| vec![error])?;
    if asset_manager != runtime {
        let left = serde_json::to_string_pretty(&asset_manager)
            .unwrap_or_else(|_| format!("{asset_manager:?}"));
        let right =
            serde_json::to_string_pretty(&runtime).unwrap_or_else(|_| format!("{runtime:?}"));
        return Err(vec![format!(
            "canonical DTO mismatch spec='{}' projection='{}'\nAssetManager:\n{}\nRuntime:\n{}",
            spec.id,
            projection.as_str(),
            left,
            right
        )]);
    }
    Ok(DtoParityReport {
        spec_id: spec.id.to_owned(),
        projection: projection.as_str().to_owned(),
        canonical: asset_manager,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ytyp_projection_accepts_equivalent_native_dtos() {
        let stable_hash = newengine_assets_api::stable_hash_from_text("fixture");
        let asset_manager = serde_json::json!({
            "source": "Content/fixture.ytyp",
            "entries": [{
                "name": "fixture",
                "stable_id": format!("{stable_hash:016x}"),
                "asset_kind": "archetype_definition",
                "entry_ref": "Content/fixture.ytyp@fixture",
                "dependencies": [{"reference":"models/a.ydd@a","kind":"drawable","role":"drawable","domain":"engine.model","required":true}]
            }]
        });
        let runtime = serde_json::json!([{
            "identity": {"name":"fixture","source":"Content/fixture.ytyp","definition_ref":"Content/fixture.ytyp@fixture"},
            "kind":"archetype_definition",
            "stable_hash":stable_hash,
            "refs":{"drawable_refs":["models/a.ydd@a"]}
        }]);
        let spec = crate::tool_runtime_conformance_spec("ytyp").unwrap();
        validate_native_dto_parity(
            spec,
            "Content/fixture.ytyp",
            &serde_json::to_vec(&asset_manager).unwrap(),
            &serde_json::to_vec(&runtime).unwrap(),
        )
        .expect("YTYP parity");
    }

    #[test]
    fn ydd_projection_detects_mesh_count_drift() {
        let hash = newengine_assets_api::stable_hash_from_text("drawable");
        let asset_manager = serde_json::json!({
            "entries":[{
                "name":"drawable","name_hash":hash,"mesh_count":1,
                "material_slots":[{"slot_name":"mesh","required":true}],
                "bounds_min":[0.0,0.0,0.0],"bounds_max":[1.0,1.0,0.0]
            }]
        });
        let runtime = serde_json::json!({"entries":[{
            "name":"drawable","source_path":"fixture.obj","bounds_min":[0.0,0.0,0.0],"bounds_max":[1.0,1.0,0.0],
            "meshes":[]
        }]});
        let spec = crate::tool_runtime_conformance_spec("ydd").unwrap();
        let errors = validate_native_dto_parity(
            spec,
            "Content/fixture.ydd",
            &serde_json::to_vec(&asset_manager).unwrap(),
            &serde_json::to_vec(&runtime).unwrap(),
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|error| error.contains("canonical DTO mismatch")));
    }

    #[test]
    fn neui_projection_ignores_retained_tree_details() {
        let hash = newengine_assets_api::stable_hash_from_text("surface");
        let asset_manager = serde_json::json!({"entries":[{
            "name":"surface","stable_id":format!("{hash:016x}"),"asset_kind":"ui_surface"
        }]});
        let runtime = serde_json::json!({
            "id":"runtime.surface","kind":"surface","component_id":"surface","role":"surface",
            "source_span":{"source_ref":"Content/fixture.neui@surface","line":1,"column":1},
            "children":[{"id":"child","kind":"text","component_id":"text","role":"text"}]
        });
        let spec = crate::tool_runtime_conformance_spec("neui").unwrap();
        validate_native_dto_parity(
            spec,
            "Content/fixture.neui",
            &serde_json::to_vec(&asset_manager).unwrap(),
            &serde_json::to_vec(&runtime).unwrap(),
        )
        .expect("NEUI selector/runtime parity");
    }
}
