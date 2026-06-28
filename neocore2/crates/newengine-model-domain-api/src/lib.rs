#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable domain contracts for drawable/model construction.
//!
//! Drawable/model semantics stay separate from definition metadata and dependency graph expansion.
//! Definition files are routed through `engine.assets.definitions`; graph expansion is routed through `engine.assets.graph`.
//! `.nepak` packages remain VFS delivery containers.

mod asset_chain;
mod asset_graph;
mod construction;
mod data_driven;
mod definition;
mod drawable;
mod texture_dictionary;

pub use asset_chain::*;
pub use asset_graph::*;
pub use construction::*;
pub use data_driven::*;
pub use definition::*;
pub use drawable::*;
pub use texture_dictionary::*;

pub use newengine_model_skeleton_api::{
    ModelSkeletonAnchors, ModelSkeletonJointMetadata, ModelSkeletonMetadata,
};

pub const ENGINE_ASSETS_MODELS_SERVICE_ID: &str = "engine.assets.models";
pub const ENGINE_MODEL_SKELETONS_SERVICE_ID: &str = "engine.assets.models.skeletons";
pub const ENGINE_MODEL_MATERIALS_SERVICE_ID: &str = "engine.assets.models.materials";
pub const ENGINE_MODEL_COLLISIONS_SERVICE_ID: &str = "engine.assets.models.collisions";

pub const MODEL_SERVICE_ID: &str = "model.api";
pub const MODEL_BACKEND_CAPABILITY_ID: &str = "assets.models.backend";
pub const MODEL_SKELETONS_SERVICE_ID: &str = "model.skeletons.api";
pub const MODEL_SKELETONS_BACKEND_CAPABILITY_ID: &str = "assets.models.skeletons.backend";
pub const MODEL_MATERIALS_SERVICE_ID: &str = "model.materials.api";
pub const MODEL_MATERIALS_BACKEND_CAPABILITY_ID: &str = "assets.models.materials.backend";
pub const MODEL_COLLISIONS_SERVICE_ID: &str = "model.collisions.api";
pub const MODEL_COLLISIONS_BACKEND_CAPABILITY_ID: &str = "assets.models.collisions.backend";

pub const MODEL_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const MODEL_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const MODEL_SERVICE_METHOD_SHUTDOWN_V1: &str =
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1: &str = "assemble_json_v1";
pub const MODEL_SERVICE_METHOD_VALIDATE_JSON_V1: &str = "validate_json_v1";

pub const DRAWABLE_DICTIONARY_EXTENSION: &str = "ydd";
pub const DRAWABLE_DICTIONARY_ASSET_KIND: &str = "drawable_dictionary";
pub const DRAWABLE_DICTIONARY_CONTAINER: &str = "newengine.listfile.nef8.ydd";
pub const DRAWABLE_DICTIONARY_MANIFEST_SCHEMA: &str = "newengine.drawable_dictionary.manifest.v1";
pub const MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1: &str =
    "assets.models.drawable_manifest_v1";
pub const MODEL_SERVICE_METHOD_RESOLVE_DRAWABLE_V1: &str = "assets.models.resolve_drawable_v1";

/// Texture dictionary role consumed after graph/material resolution.
pub const TEXTURE_DICTIONARY_EXTENSION: &str = "ytd";
pub const TEXTURE_DICTIONARY_ASSET_KIND: &str = "texture_dictionary";
pub const TEXTURE_DICTIONARY_CONTAINER: &str = "newengine.listfile.nef8.ytd";
pub const TEXTURE_DICTIONARY_MANIFEST_SCHEMA: &str = "newengine.texture_dictionary.manifest.v2";

pub const MATERIAL_LIBRARY_EXTENSION: &str = "nemat";
pub const MATERIAL_LIBRARY_ASSET_KIND: &str = "material_library";
pub const MATERIAL_LIBRARY_CONTAINER: &str = "newengine.listfile.nef8.nemat";

pub const ASSET_PACKAGE_EXTENSION: &str = "nepak";
pub const ASSET_PACKAGE_ASSET_KIND: &str = "asset_package";
pub const ASSET_PACKAGE_CONTAINER: &str = "newengine.asset_package.v1";

pub const CLIP_DICTIONARY_EXTENSION: &str = "ycd";
pub const CLIP_DICTIONARY_ASSET_KIND: &str = "clip_dictionary";
pub const PHYSICS_DICTIONARY_EXTENSION: &str = "ybn";
pub const PHYSICS_DICTIONARY_ASSET_KIND: &str = "physics_dictionary";

pub const OBJECT_TYPE_DEFINITIONS_EXTENSION: &str = "ytyp";
pub const OBJECT_TYPE_DEFINITIONS_ASSET_KIND: &str = "archetype_dictionary";
pub const OBJECT_TYPE_DEFINITIONS_CONTAINER: &str = "newengine.listfile.nef8.ytyp";
pub const DEFINITION_ENTRIES_SCHEMA: &str = "newengine.ytyp.definition_entries.v1";

pub const MODEL_FEATURE_DOMAINS: &[&str] = &[
    "mesh.obj",
    "material.mtl",
    "skeleton.nef8",
    "collision.default",
    "drawable.ydd",
    "material.nemat",
    "texture.ytd",
    "package.nepak",
    "drawable.resolve",
];

pub const MODEL_SERVICE_METHODS: &[&str] = &[
    MODEL_SERVICE_METHOD_INFO,
    MODEL_SERVICE_METHOD_INVOKE,
    MODEL_SERVICE_METHOD_SHUTDOWN_V1,
    MODEL_SERVICE_METHOD_ASSEMBLE_JSON_V1,
    MODEL_SERVICE_METHOD_VALIDATE_JSON_V1,
    MODEL_SERVICE_METHOD_DRAWABLE_DICTIONARY_MANIFEST_JSON_V1,
    MODEL_SERVICE_METHOD_RESOLVE_DRAWABLE_V1,
];

pub const MODEL_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.models",
        ENGINE_ASSETS_MODELS_SERVICE_ID,
        MODEL_SERVICE_ID,
        MODEL_BACKEND_CAPABILITY_ID,
    );

pub const MODEL_SKELETONS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.models.skeletons",
        ENGINE_MODEL_SKELETONS_SERVICE_ID,
        MODEL_SKELETONS_SERVICE_ID,
        MODEL_SKELETONS_BACKEND_CAPABILITY_ID,
    );

pub const MODEL_MATERIALS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.models.materials",
        ENGINE_MODEL_MATERIALS_SERVICE_ID,
        MODEL_MATERIALS_SERVICE_ID,
        MODEL_MATERIALS_BACKEND_CAPABILITY_ID,
    );

pub const MODEL_COLLISIONS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "assets.models.collisions",
        ENGINE_MODEL_COLLISIONS_SERVICE_ID,
        MODEL_COLLISIONS_SERVICE_ID,
        MODEL_COLLISIONS_BACKEND_CAPABILITY_ID,
    );

pub const MODEL_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_ASSETS_MODELS_SERVICE_ID,
        "newengine.assets.models-domain-api >= 0.1.x",
        MODEL_SERVICE_METHODS,
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
        assert_eq!(
            MODEL_BACKEND_SERVICE_SPEC.engine_gateway_id,
            ENGINE_ASSETS_MODELS_SERVICE_ID
        );
        assert_eq!(
            MODEL_SKELETONS_BACKEND_SERVICE_SPEC.domain,
            "assets.models.skeletons"
        );
        assert_eq!(
            MODEL_MATERIALS_BACKEND_SERVICE_SPEC.domain,
            "assets.models.materials"
        );
        assert_eq!(
            MODEL_COLLISIONS_BACKEND_SERVICE_SPEC.domain,
            "assets.models.collisions"
        );
        assert!(!MODEL_FEATURE_DOMAINS.contains(&"definition_entries.ytyp"));
        assert!(MODEL_FEATURE_DOMAINS.contains(&"drawable.resolve"));
        assert!(MODEL_FEATURE_DOMAINS.contains(&"material.nemat"));
        assert!(MODEL_FEATURE_DOMAINS.contains(&"texture.ytd"));
        assert!(MODEL_FEATURE_DOMAINS.contains(&"package.nepak"));
        assert!(!MODEL_FEATURE_DOMAINS.contains(&"texture.noncanonical.runtime"));
        assert!(MODEL_SERVICE_METHODS.contains(&MODEL_SERVICE_METHOD_RESOLVE_DRAWABLE_V1));
    }

    #[test]
    fn definition_asset_chain_uses_ytyp_ydd_nemat_ytd_roles() {
        let mut entry = DefinitionEntry {
            name: "prop_box".to_owned(),
            asset_name: "prop_box_drawable".to_owned(),
            asset_type: "ASSET_TYPE_DRAWABLE".to_owned(),
            dictionaries: DefinitionDictionaries {
                texture: Some("prop_box_textures".to_owned()),
                ..Default::default()
            },
            ..Default::default()
        };
        refresh_definition_asset_chain("metadata/props.ytyp", &mut entry);
        assert_eq!(
            entry
                .asset_chain
                .definition_type
                .as_ref()
                .unwrap()
                .extension,
            "ytyp"
        );
        assert_eq!(
            entry
                .asset_chain
                .drawable_dictionary
                .as_ref()
                .unwrap()
                .extension,
            "ydd"
        );
        assert_eq!(
            entry
                .asset_chain
                .texture_dictionary
                .as_ref()
                .unwrap()
                .extension,
            "ytd"
        );
    }

    #[test]
    fn construction_plan_derives_material_binding_from_ytyp_chain() {
        let mut entry = DefinitionEntry {
            name: "prop_box".to_owned(),
            asset_name: "prop_box_drawable".to_owned(),
            asset_type: "ASSET_TYPE_DRAWABLE".to_owned(),
            dictionaries: DefinitionDictionaries {
                texture: Some("prop_box_textures".to_owned()),
                ..Default::default()
            },
            ..Default::default()
        };
        refresh_definition_asset_chain("metadata/props.ytyp", &mut entry);
        let manifest = DefinitionEntriesManifest {
            source: "metadata/props.ytyp".to_owned(),
            definition_entries: vec![entry],
            ..Default::default()
        };
        let plan = build_data_driven_construction_plan(&manifest);
        assert_eq!(plan.objects.len(), 1);
        assert_eq!(plan.objects[0].drawable.as_ref().unwrap().extension, "ydd");
        assert_eq!(
            plan.objects[0]
                .texture_dictionary
                .as_ref()
                .unwrap()
                .extension,
            "ytd"
        );
        assert_eq!(
            plan.objects[0].material_binding.material_library_role,
            ROLE_MATERIAL_LIBRARY
        );
        assert_eq!(
            plan.objects[0].material_binding.texture_dictionary_role,
            ROLE_TEXTURE_DICTIONARY
        );
    }
}
