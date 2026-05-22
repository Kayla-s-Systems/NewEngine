use serde::{Deserialize, Serialize};

use crate::{
    ASSET_PACKAGE_ASSET_KIND, ASSET_PACKAGE_CONTAINER, ASSET_PACKAGE_EXTENSION,
    DRAWABLE_DICTIONARY_ASSET_KIND, DRAWABLE_DICTIONARY_CONTAINER, DRAWABLE_DICTIONARY_EXTENSION,
    OBJECT_TYPE_DEFINITIONS_ASSET_KIND, OBJECT_TYPE_DEFINITIONS_CONTAINER,
    MATERIAL_LIBRARY_ASSET_KIND, MATERIAL_LIBRARY_CONTAINER, MATERIAL_LIBRARY_EXTENSION,
    OBJECT_TYPE_DEFINITIONS_EXTENSION, TEXTURE_DICTIONARY_ASSET_KIND, TEXTURE_DICTIONARY_CONTAINER,
    TEXTURE_DICTIONARY_EXTENSION,
};

/// Compile-time canonical asset-chain role.
///
/// This table is the source of truth for authored data-driven model content.
/// Runtime code, codecs and tools should query it instead of copying extension /
/// kind tuples into separate branches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelAssetChainRoleSpec {
    pub role: &'static str,
    pub extension: &'static str,
    pub asset_kind: &'static str,
    pub source_container: &'static str,
    pub codec_service: &'static str,
    pub primary_output: &'static str,
    pub runtime_ready: bool,
    pub runtime_container: Option<&'static str>,
    pub description: &'static str,
}

/// Serializable owned role used by tools and asset graph diagnostics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelAssetChainRole {
    pub role: String,
    pub extension: String,
    pub asset_kind: String,
    pub source_container: String,
    pub codec_service: String,
    pub primary_output: String,
    pub runtime_ready: bool,
    pub runtime_container: Option<String>,
    pub description: String,
}

impl Default for ModelAssetChainRole {
    fn default() -> Self {
        Self {
            role: String::new(),
            extension: String::new(),
            asset_kind: String::new(),
            source_container: String::new(),
            codec_service: String::new(),
            primary_output: String::new(),
            runtime_ready: false,
            runtime_container: None,
            description: String::new(),
        }
    }
}

impl From<&ModelAssetChainRoleSpec> for ModelAssetChainRole {
    fn from(role: &ModelAssetChainRoleSpec) -> Self {
        Self {
            role: role.role.to_owned(),
            extension: role.extension.to_owned(),
            asset_kind: role.asset_kind.to_owned(),
            source_container: role.source_container.to_owned(),
            codec_service: role.codec_service.to_owned(),
            primary_output: role.primary_output.to_owned(),
            runtime_ready: role.runtime_ready,
            runtime_container: role.runtime_container.map(ToOwned::to_owned),
            description: role.description.to_owned(),
        }
    }
}

pub const ROLE_DEFINITION_ENTRIES: &str = "definition_entries";
pub const ROLE_DRAWABLE_DICTIONARY: &str = "drawable_dictionary";
pub const ROLE_MATERIAL_LIBRARY: &str = "material_library";
pub const ROLE_TEXTURE_DICTIONARY: &str = "texture_dictionary";
pub const ROLE_ASSET_PACKAGE: &str = "asset_package";

/// Public authored chain: `.ytyp -> .ydd -> .nemat -> .ytd`.
///
/// `.neytd` is intentionally not present here and is not kept as a public alias.
pub const MODEL_ASSET_CHAIN_ROLES: &[ModelAssetChainRoleSpec] = &[
    ModelAssetChainRoleSpec {
        role: ROLE_DEFINITION_ENTRIES,
        extension: OBJECT_TYPE_DEFINITIONS_EXTENSION,
        asset_kind: OBJECT_TYPE_DEFINITIONS_ASSET_KIND,
        source_container: OBJECT_TYPE_DEFINITIONS_CONTAINER,
        codec_service: "asset.codec.listfile.ytyp",
        primary_output: "definitions.manifest_json_v1",
        runtime_ready: true,
        runtime_container: None,
        description: "NEF8 ListFile Definition Entries / archetype metadata. Declares drawable, texture, physics, bounds and LOD metadata.",
    },
    ModelAssetChainRoleSpec {
        role: ROLE_DRAWABLE_DICTIONARY,
        extension: DRAWABLE_DICTIONARY_EXTENSION,
        asset_kind: DRAWABLE_DICTIONARY_ASSET_KIND,
        source_container: DRAWABLE_DICTIONARY_CONTAINER,
        codec_service: "asset.codec.listfile.ydd",
        primary_output: "model.drawable_dictionary_manifest_json",
        runtime_ready: true,
        runtime_container: None,
        description: "NEF8 ListFile drawable dictionary source/native model container referenced by Definition Entries.",
    },
    ModelAssetChainRoleSpec {
        role: ROLE_MATERIAL_LIBRARY,
        extension: MATERIAL_LIBRARY_EXTENSION,
        asset_kind: MATERIAL_LIBRARY_ASSET_KIND,
        source_container: MATERIAL_LIBRARY_CONTAINER,
        codec_service: "asset.codec.listfile.nemat",
        primary_output: "material.manifest_json",
        runtime_ready: true,
        runtime_container: None,
        description: "NEF8 ListFile material library. Drawable material slots reference .nemat@entry selectors.",
    },
    ModelAssetChainRoleSpec {
        role: ROLE_TEXTURE_DICTIONARY,
        extension: TEXTURE_DICTIONARY_EXTENSION,
        asset_kind: TEXTURE_DICTIONARY_ASSET_KIND,
        source_container: TEXTURE_DICTIONARY_CONTAINER,
        codec_service: "asset.codec.listfile.ytd",
        primary_output: "texture_dictionary.manifest_json",
        runtime_ready: true,
        runtime_container: None,
        description: "NEF8 ListFile primary authored texture dictionary. YTYP and materials reference .ytd@entry selectors.",
    },
];

/// Container roles used to ship authored asset chains.
pub const MODEL_ASSET_PACKAGE_ROLES: &[ModelAssetChainRoleSpec] = &[
    ModelAssetChainRoleSpec {
        role: ROLE_ASSET_PACKAGE,
        extension: ASSET_PACKAGE_EXTENSION,
        asset_kind: ASSET_PACKAGE_ASSET_KIND,
        source_container: ASSET_PACKAGE_CONTAINER,
        codec_service: "asset.codec.nepak",
        primary_output: "container.manifest_json",
        runtime_ready: true,
        runtime_container: None,
        description: "Package container for delivering .ytyp/.ydd/.nemat/.ytd and related assets as one VFS layer.",
    },
];

/// No legacy asset roles are part of the target model chain.
///
/// The project is still early, so old compatibility roles are intentionally not
/// kept as aliases in the public contract.
pub const MODEL_LEGACY_ASSET_ROLES: &[ModelAssetChainRoleSpec] = &[];

#[inline]
pub fn model_asset_chain_roles() -> Vec<ModelAssetChainRole> {
    MODEL_ASSET_CHAIN_ROLES.iter().map(ModelAssetChainRole::from).collect()
}

#[inline]
pub fn model_asset_package_roles() -> Vec<ModelAssetChainRole> {
    MODEL_ASSET_PACKAGE_ROLES.iter().map(ModelAssetChainRole::from).collect()
}

#[inline]
pub fn model_legacy_asset_roles() -> Vec<ModelAssetChainRole> {
    MODEL_LEGACY_ASSET_ROLES.iter().map(ModelAssetChainRole::from).collect()
}

#[inline]
pub fn model_asset_chain_role_by_extension(extension: &str) -> Option<&'static ModelAssetChainRoleSpec> {
    let ext = extension.trim().trim_start_matches('.');
    MODEL_ASSET_CHAIN_ROLES
        .iter()
        .chain(MODEL_ASSET_PACKAGE_ROLES.iter())
        .find(|role| role.extension.eq_ignore_ascii_case(ext))
}

#[inline]
pub fn model_asset_chain_role_by_kind(asset_kind: &str) -> Option<&'static ModelAssetChainRoleSpec> {
    let kind = asset_kind.trim();
    MODEL_ASSET_CHAIN_ROLES
        .iter()
        .chain(MODEL_ASSET_PACKAGE_ROLES.iter())
        .find(|role| role.asset_kind.eq_ignore_ascii_case(kind))
}

#[inline]
pub fn model_legacy_asset_role_by_extension(extension: &str) -> Option<&'static ModelAssetChainRoleSpec> {
    let ext = extension.trim().trim_start_matches('.');
    MODEL_LEGACY_ASSET_ROLES.iter().find(|role| role.extension.eq_ignore_ascii_case(ext))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelAssetChainManifest {
    pub schema: String,
    pub roles: Vec<ModelAssetChainRole>,
    pub package_roles: Vec<ModelAssetChainRole>,
    pub legacy_roles: Vec<ModelAssetChainRole>,
    pub authored_chain: Vec<String>,
    pub notes: Vec<String>,
}

impl Default for ModelAssetChainManifest {
    fn default() -> Self {
        Self {
            schema: "newengine.model.asset_chain.v2".to_owned(),
            roles: model_asset_chain_roles(),
            package_roles: model_asset_package_roles(),
            legacy_roles: Vec::new(),
            authored_chain: vec!["ytyp".to_owned(), "ydd".to_owned(), "nemat".to_owned(), "ytd".to_owned()],
            notes: vec![
                "ListFile implementers keep their extensions (.ytyp/.ydd/.ytd/.nemat) but share NEF8 as top-level magic.".to_owned(),
                ".nepak is a separate package/VFS delivery container, not a NEF8 ListFile.".to_owned(),
                "Data-driven runtime should consume dependency graphs resolved from engine.definitions and engine.asset_graph.".to_owned(),
            ],
        }
    }
}
