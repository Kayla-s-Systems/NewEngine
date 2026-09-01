#![forbid(unsafe_op_in_unsafe_fn)]

//! Declarative first-party runtime-unit distribution catalog.
//!
//! This crate intentionally owns **no factories and no runtime behavior**. Every
//! materializer is exported by the implementation crate that owns that capability.
//! The catalog only composes those registrations into the standard game distribution
//! and declares product-level runtime-unit roots.

pub use newengine_runtime_unit_api::{
    RuntimeUnitRegistration as StaticRuntimeUnitRegistration, RuntimeUnitRequirementSpec,
};

/// Static runtime-unit inventory shipped by the standard game distribution.
///
/// Ordering is declarative inventory order only; dependency ordering is resolved by
/// the generic Host composition solver from each registration's `requires/provides`.
pub const STATIC_RUNTIME_UNIT_REGISTRATIONS: &[StaticRuntimeUnitRegistration] = &[
    newengine_render_runtime_adapter::RUNTIME_UNIT_REGISTRATION,
    newengine_physics_runtime_adapter::RUNTIME_UNIT_REGISTRATION,
    newengine_scene_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_world_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_world_environment_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_ecs_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_entity_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_time_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_schema_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_scripting_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_tags_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_tasks_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_animation_foundation_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_navigation_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_ai_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_assets::ASSET_DOCUMENTS_RUNTIME_UNIT_REGISTRATION,
    newengine_definitions_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_assets_ui_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_material_runtime::RUNTIME_UNIT_REGISTRATION,
    newengine_model_runtime::MODELS_RUNTIME_UNIT_REGISTRATION,
    newengine_model_runtime::ASSET_GRAPH_RUNTIME_UNIT_REGISTRATION,
    newengine_audio_runtime::AUDIO_NATIVE_RUNTIME_UNIT_REGISTRATION,
    newengine_audio_world_runtime::AUDIO_SCENE_RUNTIME_UNIT_REGISTRATION,
    newengine_audio_world_runtime::AUDIO_AMBIENCE_RUNTIME_UNIT_REGISTRATION,
    newengine_audio_world_runtime::AUDIO_ORCHESTRATION_RUNTIME_UNIT_REGISTRATION,
];

/// Descriptor-only view retained for composition diagnostics and profile tests.
pub const STANDARD_GAME_RUNTIME_UNITS: &[newengine_runtime_unit_api::EngineRuntimeUnitSpec] = &[
    newengine_render_runtime_adapter::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_physics_runtime_adapter::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_scene_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_world_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_world_environment_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_ecs_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_entity_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_time_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_schema_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_scripting_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_tags_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_tasks_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_animation_foundation_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_navigation_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_ai_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_assets::ASSET_DOCUMENTS_RUNTIME_UNIT_REGISTRATION.spec,
    newengine_definitions_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_assets_ui_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_material_runtime::RUNTIME_UNIT_REGISTRATION.spec,
    newengine_model_runtime::MODELS_RUNTIME_UNIT_REGISTRATION.spec,
    newengine_model_runtime::ASSET_GRAPH_RUNTIME_UNIT_REGISTRATION.spec,
    newengine_audio_runtime::AUDIO_NATIVE_RUNTIME_UNIT_REGISTRATION.spec,
    newengine_audio_world_runtime::AUDIO_SCENE_RUNTIME_UNIT_REGISTRATION.spec,
    newengine_audio_world_runtime::AUDIO_AMBIENCE_RUNTIME_UNIT_REGISTRATION.spec,
    newengine_audio_world_runtime::AUDIO_ORCHESTRATION_RUNTIME_UNIT_REGISTRATION.spec,
];

/// Product-level capability roots for the standard game shape.
///
/// These are composition data, not implementation calls. Capability ids are read
/// from the owner registrations so the catalog cannot drift from provider metadata.
pub const STANDARD_GAME_RUNTIME_UNIT_REQUIREMENTS: &[RuntimeUnitRequirementSpec] = &[
    RuntimeUnitRequirementSpec::required(
        newengine_scene_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_world_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_world_environment_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_ecs_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_entity_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_time_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_schema_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_scripting_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_tags_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_tasks_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_animation_foundation_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_navigation_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_ai_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_assets::ASSET_DOCUMENTS_RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_assets::ASSET_DOCUMENTS_RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[1],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_definitions_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_assets_ui_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_material_runtime::RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_model_runtime::MODELS_RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_model_runtime::ASSET_GRAPH_RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_audio_runtime::AUDIO_NATIVE_RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_audio_world_runtime::AUDIO_SCENE_RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_audio_world_runtime::AUDIO_AMBIENCE_RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
    RuntimeUnitRequirementSpec::required(
        newengine_audio_world_runtime::AUDIO_ORCHESTRATION_RUNTIME_UNIT_REGISTRATION
            .spec
            .provides[0],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn standard_game_does_not_materialize_asset_types_fallback() {
        assert!(STANDARD_GAME_RUNTIME_UNITS
            .iter()
            .all(|unit| unit.id != "engine.runtime.asset-types"));
        assert!(STANDARD_GAME_RUNTIME_UNIT_REQUIREMENTS
            .iter()
            .all(|requirement| requirement.capability != "assets.types.backend"));
    }

    #[test]
    fn standard_unit_ids_are_unique() {
        let mut ids = BTreeSet::new();
        for unit in STATIC_RUNTIME_UNIT_REGISTRATIONS {
            assert!(
                ids.insert(unit.spec.id),
                "duplicate runtime unit {}",
                unit.spec.id
            );
        }
    }

    #[test]
    fn descriptor_view_matches_registration_inventory() {
        assert_eq!(
            STANDARD_GAME_RUNTIME_UNITS.len(),
            STATIC_RUNTIME_UNIT_REGISTRATIONS.len()
        );
        for (descriptor, registration) in STANDARD_GAME_RUNTIME_UNITS
            .iter()
            .zip(STATIC_RUNTIME_UNIT_REGISTRATIONS)
        {
            assert_eq!(*descriptor, registration.spec);
        }
    }

    #[test]
    fn gameplay_foundation_is_composed_from_single_capability_units() {
        for unit in [
            newengine_tags_runtime::RUNTIME_UNIT_REGISTRATION,
            newengine_tasks_runtime::RUNTIME_UNIT_REGISTRATION,
            newengine_animation_foundation_runtime::RUNTIME_UNIT_REGISTRATION,
            newengine_navigation_runtime::RUNTIME_UNIT_REGISTRATION,
            newengine_ai_runtime::RUNTIME_UNIT_REGISTRATION,
        ] {
            assert_eq!(
                unit.spec.provides.len(),
                1,
                "gameplay leaf runtime unit {} must provide exactly one capability",
                unit.spec.id
            );
        }
    }

    #[test]
    fn provides_and_requires_have_real_dependency_semantics() {
        let scene = newengine_scene_runtime::RUNTIME_UNIT_REGISTRATION.spec;
        let world = newengine_world_runtime::RUNTIME_UNIT_REGISTRATION.spec;
        assert!(scene.provides.contains(&"scene.backend"));
        assert!(!scene.requires.contains(&"scene.backend"));
        assert!(world.requires.contains(&"scene.backend"));
    }
}
