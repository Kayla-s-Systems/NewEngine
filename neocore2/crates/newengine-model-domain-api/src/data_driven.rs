use serde::{Deserialize, Serialize};

use crate::{
    DefinitionAssetRef, DefinitionEntriesManifest, DefinitionEntry, DRAWABLE_DICTIONARY_ASSET_KIND,
    DRAWABLE_DICTIONARY_EXTENSION, ROLE_DEFINITION_ENTRIES, ROLE_DRAWABLE_DICTIONARY,
    ROLE_MATERIAL_LIBRARY, ROLE_TEXTURE_DICTIONARY, TEXTURE_DICTIONARY_EXTENSION,
};

pub const DATA_DRIVEN_CONSTRUCTION_PLAN_SCHEMA: &str =
    "newengine.assets.definitions.data_driven_construction_plan.v1";

/// Declarative construction plan derived from YTYP Definition Entries.
///
/// This is the boundary that prevents scene/bootstrap code from manually doing
/// "create material here, attach it to model there". The engine can consume one
/// plan and let AssetManager/model/material gateways resolve concrete payloads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataDrivenConstructionPlan {
    pub schema: String,
    pub source: String,
    pub source_manifest_schema: String,
    pub objects: Vec<DataDrivenObjectConstruction>,
    pub packages: Vec<DataDrivenPackageHint>,
    pub warnings: Vec<String>,
}

impl Default for DataDrivenConstructionPlan {
    fn default() -> Self {
        Self {
            schema: DATA_DRIVEN_CONSTRUCTION_PLAN_SCHEMA.to_owned(),
            source: String::new(),
            source_manifest_schema: String::new(),
            objects: Vec::new(),
            packages: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DataDrivenObjectConstruction {
    pub name: String,
    pub archetype_kind: String,
    pub definition: DataDrivenAssetLink,
    pub drawable: Option<DataDrivenAssetLink>,
    pub texture_dictionary: Option<DataDrivenAssetLink>,
    pub physics_dictionary: Option<DataDrivenAssetLink>,
    pub lod: DataDrivenLodPolicy,
    pub bounds: DataDrivenBoundsPolicy,
    pub material_binding: DataDrivenMaterialBindingPolicy,
    pub material_slots: Vec<DataDrivenMaterialSlotBinding>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DataDrivenAssetLink {
    pub role: String,
    pub logical_path: String,
    pub asset_kind: String,
    pub extension: String,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataDrivenLodPolicy {
    pub lod_dist: f32,
    pub hd_texture_dist: f32,
    pub flags: u32,
}

impl Default for DataDrivenLodPolicy {
    fn default() -> Self {
        Self {
            lod_dist: 0.0,
            hd_texture_dist: 0.0,
            flags: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataDrivenBoundsPolicy {
    pub bb_min: [f32; 3],
    pub bb_max: [f32; 3],
    pub bs_centre: [f32; 3],
    pub bs_radius: f32,
}

impl Default for DataDrivenBoundsPolicy {
    fn default() -> Self {
        Self {
            bb_min: [0.0; 3],
            bb_max: [0.0; 3],
            bs_centre: [0.0; 3],
            bs_radius: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataDrivenMaterialSlotBinding {
    pub slot: String,
    pub material: String,
    pub resolve_gateway: String,
}

impl Default for DataDrivenMaterialSlotBinding {
    fn default() -> Self {
        Self {
            slot: String::new(),
            material: String::new(),
            resolve_gateway: "engine.assets.materials".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataDrivenMaterialBindingPolicy {
    pub source: String,
    pub material_library_role: String,
    pub texture_dictionary_role: String,
    pub policy: String,
}

impl Default for DataDrivenMaterialBindingPolicy {
    fn default() -> Self {
        Self {
            source: "definition_entries.asset_chain".to_owned(),
            material_library_role: ROLE_MATERIAL_LIBRARY.to_owned(),
            texture_dictionary_role: ROLE_TEXTURE_DICTIONARY.to_owned(),
            policy: "bind_drawable_material_slots_to_nemat_entries_then_resolve_ytd_textures"
                .to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataDrivenPackageHint {
    pub role: String,
    pub extension: String,
    pub contains_roles: Vec<String>,
    pub description: String,
}

impl Default for DataDrivenPackageHint {
    fn default() -> Self {
        Self {
            role: "asset_package".to_owned(),
            extension: "nepak".to_owned(),
            contains_roles: vec![
                ROLE_DEFINITION_ENTRIES.to_owned(),
                ROLE_DRAWABLE_DICTIONARY.to_owned(),
                ROLE_MATERIAL_LIBRARY.to_owned(),
                ROLE_TEXTURE_DICTIONARY.to_owned(),
            ],
            description: "Package/VFS delivery container for the authored asset chain.".to_owned(),
        }
    }
}

pub fn build_data_driven_construction_plan(
    manifest: &DefinitionEntriesManifest,
) -> DataDrivenConstructionPlan {
    let mut plan = DataDrivenConstructionPlan {
        source: manifest.source.clone(),
        source_manifest_schema: manifest.schema.clone(),
        packages: vec![DataDrivenPackageHint::default()],
        ..Default::default()
    };

    for entry in &manifest.definition_entries {
        let object = build_data_driven_object(entry);
        if object.drawable.is_none() {
            plan.warnings.push(format!(
                "definition '{}' does not resolve a .{} drawable dictionary",
                entry.name, DRAWABLE_DICTIONARY_EXTENSION
            ));
        }
        if object.texture_dictionary.is_none() {
            plan.warnings.push(format!(
                "definition '{}' does not resolve a .{} texture dictionary",
                entry.name, TEXTURE_DICTIONARY_EXTENSION
            ));
        }
        plan.objects.push(object);
    }

    plan.objects.sort_by(|a, b| a.name.cmp(&b.name));
    plan.warnings.sort();
    plan.warnings.dedup();
    plan
}

fn build_data_driven_object(entry: &DefinitionEntry) -> DataDrivenObjectConstruction {
    DataDrivenObjectConstruction {
        name: object_name(entry),
        archetype_kind: entry.entry_kind.clone(),
        definition: entry
            .asset_chain
            .definition_type
            .as_ref()
            .map(|it| asset_link(it, true))
            .unwrap_or_default(),
        drawable: entry
            .asset_chain
            .drawable_dictionary
            .as_ref()
            .map(|it| asset_link(it, true))
            .or_else(|| fallback_drawable_link(entry)),
        texture_dictionary: entry
            .asset_chain
            .texture_dictionary
            .as_ref()
            .map(|it| asset_link(it, true)),
        physics_dictionary: entry
            .asset_chain
            .physics_dictionary
            .as_ref()
            .map(|it| asset_link(it, false)),
        lod: DataDrivenLodPolicy { lod_dist: entry.lod_dist, hd_texture_dist: entry.hd_texture_dist, flags: entry.flags },
        bounds: DataDrivenBoundsPolicy {
            bb_min: entry.bounds.bb_min,
            bb_max: entry.bounds.bb_max,
            bs_centre: entry.bounds.bs_centre,
            bs_radius: entry.bounds.bs_radius,
        },
        material_binding: DataDrivenMaterialBindingPolicy::default(),
        material_slots: Vec::new(),
        notes: vec!["Construction is derived from Definition Entries; runtime resolves drawable -> material slots -> texture refs -> render packet.".to_owned()],
    }
}

fn fallback_drawable_link(entry: &DefinitionEntry) -> Option<DataDrivenAssetLink> {
    let name = if !entry.asset_name.trim().is_empty() {
        &entry.asset_name
    } else {
        &entry.name
    };
    let logical_path = ensure_extension(name, DRAWABLE_DICTIONARY_EXTENSION);
    (!logical_path.is_empty()).then(|| DataDrivenAssetLink {
        role: ROLE_DRAWABLE_DICTIONARY.to_owned(),
        logical_path,
        asset_kind: DRAWABLE_DICTIONARY_ASSET_KIND.to_owned(),
        extension: DRAWABLE_DICTIONARY_EXTENSION.to_owned(),
        required: true,
    })
}

fn asset_link(reference: &DefinitionAssetRef, required: bool) -> DataDrivenAssetLink {
    DataDrivenAssetLink {
        role: reference.role.clone(),
        logical_path: if !reference.canonical_ref.trim().is_empty() {
            reference.canonical_ref.clone()
        } else {
            reference
                .logical_path_hint
                .clone()
                .unwrap_or_else(|| ensure_extension(&reference.name, &reference.extension))
        },
        asset_kind: reference.asset_kind.clone(),
        extension: reference.extension.clone(),
        required,
    }
}

fn object_name(entry: &DefinitionEntry) -> String {
    if !entry.name.trim().is_empty() {
        entry.name.clone()
    } else {
        entry.asset_name.clone()
    }
}

fn ensure_extension(value: &str, extension: &str) -> String {
    let value = value
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_owned();
    if value.is_empty() {
        return value;
    }
    let ext = extension.trim_start_matches('.');
    if value.to_ascii_lowercase().ends_with(&format!(".{ext}")) {
        value
    } else {
        format!("{value}.{ext}")
    }
}
