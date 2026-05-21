#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable domain contracts for model construction.
//!
//! The crate owns one canonical model chain:
//!
//! ```text
//! .ytyp Definition Entries -> .ydd drawable dictionary -> .ytd texture dictionary role
//! ```
//!
//! NewEngine runtime texture packets may come from `.neytd`, but archetype metadata
//! speaks in source/domain roles so the core does not special-case concrete
//! renderer/AssetManager implementation details.

mod construction;
mod definition;
mod drawable;

pub use construction::*;
pub use definition::*;
pub use drawable::*;

pub use newengine_model_skeleton_api::{
    ModelSkeletonAnchors, ModelSkeletonJointMetadata, ModelSkeletonMetadata,
};

pub const ENGINE_MODEL_SERVICE_ID: &str = "engine.model";
pub const ENGINE_MODEL_SKELETONS_SERVICE_ID: &str = "engine.model.skeletons";
pub const ENGINE_MODEL_MATERIALS_SERVICE_ID: &str = "engine.model.materials";
pub const ENGINE_MODEL_COLLISIONS_SERVICE_ID: &str = "engine.model.collisions";

pub const MODEL_SERVICE_ID: &str = "model.api";
pub const MODEL_BACKEND_CAPABILITY_ID: &str = "model.backend";
pub const MODEL_SKELETONS_SERVICE_ID: &str = "model.skeletons.api";
pub const MODEL_SKELETONS_BACKEND_CAPABILITY_ID: &str = "model.skeletons.backend";
pub const MODEL_MATERIALS_SERVICE_ID: &str = "model.materials.api";
pub const MODEL_MATERIALS_BACKEND_CAPABILITY_ID: &str = "model.materials.backend";
pub const MODEL_COLLISIONS_SERVICE_ID: &str = "model.collisions.api";
pub const MODEL_COLLISIONS_BACKEND_CAPABILITY_ID: &str = "model.collisions.backend";

pub const MODEL_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const MODEL_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const MODEL_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1: &str = "assemble_json_v1";
pub const MODEL_SERVICE_METHOD_VALIDATE_JSON_V1: &str = "validate_json_v1";

pub const DRAWABLE_DICTIONARY_EXTENSION: &str = "ydd";
pub const DRAWABLE_DICTIONARY_ASSET_KIND: &str = "drawable_dictionary";
pub const DRAWABLE_DICTIONARY_MAGIC: [u8; 4] = *b"RSC7";
pub const DRAWABLE_DICTIONARY_MANIFEST_SCHEMA: &str = "newengine.drawable_dictionary.manifest.v1";
pub const MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1: &str = "model.drawable_dictionary_manifest_json_v1";

/// Source/domain texture dictionary role referenced by YTYP archetypes.
pub const TEXTURE_DICTIONARY_EXTENSION: &str = "ytd";
pub const TEXTURE_DICTIONARY_ASSET_KIND: &str = "texture_dictionary";

/// Runtime-ready NewEngine texture dictionary implementation.
pub const NEWENGINE_TEXTURE_DICTIONARY_EXTENSION: &str = "neytd";
pub const NEWENGINE_TEXTURE_DICTIONARY_CONTAINER: &str = "newengine.texture_dictionary.neytd.v2";

pub const CLIP_DICTIONARY_EXTENSION: &str = "ycd";
pub const CLIP_DICTIONARY_ASSET_KIND: &str = "clip_dictionary";
pub const PHYSICS_DICTIONARY_EXTENSION: &str = "ybn";
pub const PHYSICS_DICTIONARY_ASSET_KIND: &str = "physics_dictionary";

pub const OBJECT_TYPE_DEFINITIONS_EXTENSION: &str = "ytyp";
pub const OBJECT_TYPE_DEFINITIONS_ASSET_KIND: &str = "object_type_definitions";
pub const OBJECT_TYPE_DEFINITIONS_CONTAINER: &str = "rockstar.map_types.ytyp.xml";
pub const DEFINITION_ENTRIES_SCHEMA: &str = "newengine.ytyp.definition_entries.v1";
pub const MODEL_SERVICE_METHOD_DEFINITION_ENTRIES_JSON_V1: &str = "model.definition_entries_json_v1";

pub const MODEL_FEATURE_DOMAINS: &[&str] = &[
    "mesh.obj",
    "material.mtl",
    "skeleton.rsc7",
    "collision.default",
    "drawable.ydd",
    "texture.ytd",
    "definition_entries.ytyp",
];

pub const MODEL_SERVICE_METHODS: &[&str] = &[
    MODEL_SERVICE_METHOD_INFO,
    MODEL_SERVICE_METHOD_INVOKE,
    MODEL_SERVICE_METHOD_SHUTDOWN_V1,
    MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1,
    MODEL_SERVICE_METHOD_VALIDATE_JSON_V1,
    MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1,
    MODEL_SERVICE_METHOD_DEFINITION_ENTRIES_JSON_V1,
];

pub const MODEL_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "model",
        ENGINE_MODEL_SERVICE_ID,
        MODEL_SERVICE_ID,
        MODEL_BACKEND_CAPABILITY_ID,
    );

pub const MODEL_SKELETONS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "model.skeletons",
        ENGINE_MODEL_SKELETONS_SERVICE_ID,
        MODEL_SKELETONS_SERVICE_ID,
        MODEL_SKELETONS_BACKEND_CAPABILITY_ID,
    );

pub const MODEL_MATERIALS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "model.materials",
        ENGINE_MODEL_MATERIALS_SERVICE_ID,
        MODEL_MATERIALS_SERVICE_ID,
        MODEL_MATERIALS_BACKEND_CAPABILITY_ID,
    );

pub const MODEL_COLLISIONS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "model.collisions",
        ENGINE_MODEL_COLLISIONS_SERVICE_ID,
        MODEL_COLLISIONS_SERVICE_ID,
        MODEL_COLLISIONS_BACKEND_CAPABILITY_ID,
    );

pub const MODEL_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_MODEL_SERVICE_ID,
        "newengine.model-domain-api >= 0.1.x",
        newengine_service_api::JSON_CONTROL_SERVICE_METHODS_V1,
    );

pub const MODEL_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        MODEL_RUNTIME_CONTRACT_SPEC,
        Some(MODEL_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_MODEL_BACKEND"),
    );

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_are_gateway_first() {
        assert_eq!(MODEL_BACKEND_SERVICE_SPEC.engine_gateway_id, ENGINE_MODEL_SERVICE_ID);
        assert_eq!(MODEL_SKELETONS_BACKEND_SERVICE_SPEC.domain, "model.skeletons");
        assert_eq!(MODEL_MATERIALS_BACKEND_SERVICE_SPEC.domain, "model.materials");
        assert_eq!(MODEL_COLLISIONS_BACKEND_SERVICE_SPEC.domain, "model.collisions");
        assert!(MODEL_FEATURE_DOMAINS.contains(&"definition_entries.ytyp"));
        assert!(MODEL_FEATURE_DOMAINS.contains(&"texture.ytd"));
    }

    #[test]
    fn definition_asset_chain_uses_ytyp_ydd_ytd_roles() {
        let mut entry = DefinitionEntry {
            name: "prop_box".to_owned(),
            asset_name: "prop_box_drawable".to_owned(),
            asset_type: "ASSET_TYPE_DRAWABLE".to_owned(),
            dictionaries: DefinitionDictionaries { texture: Some("prop_box_textures".to_owned()), ..Default::default() },
            ..Default::default()
        };
        refresh_definition_asset_chain("metadata/props.ytyp", &mut entry);
        assert_eq!(entry.asset_chain.definition_type.as_ref().unwrap().extension, "ytyp");
        assert_eq!(entry.asset_chain.drawable_dictionary.as_ref().unwrap().extension, "ydd");
        assert_eq!(entry.asset_chain.texture_dictionary.as_ref().unwrap().extension, "ytd");
    }
}
