#![forbid(unsafe_op_in_unsafe_fn)]

//! Static first-party runtime units.
//!
//! Product profiles select stable `EngineRuntimeUnitSpec` ids. Concrete gateway
//! registration and lifecycle module construction live in this catalog.

use std::sync::Arc;

use newengine_core::{Engine, EngineError, EngineResult, Module, ModuleCtx, StartupConfig};
use newengine_service_api::{EngineRuntimeUnitKind, EngineRuntimeUnitSpec, RuntimeUnitRequirementSpec};

pub type StaticRuntimeUnitFactory =
    fn(&mut Engine<()>, &StartupConfig) -> EngineResult<Option<Box<dyn Module<()>>>>;

#[derive(Clone, Copy)]
pub struct StaticRuntimeUnitRegistration {
    pub spec: EngineRuntimeUnitSpec,
    pub factory: StaticRuntimeUnitFactory,
}

const PROVIDER_TAGS: &[&str] = &[
    "engine.runtime-unit",
    "static",
    "first-party",
    "provider-route",
];
const MODULE_TAGS: &[&str] = &[
    "engine.runtime-unit",
    "static",
    "first-party",
    "lifecycle-module",
];

pub const SCENE_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.scene",
    1,
    EngineRuntimeUnitKind::Provider,
    &[newengine_scene_io::SCENE_BACKEND_CAPABILITY_ID],
    &[newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID],
    PROVIDER_TAGS,
);
pub const WORLD_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.world",
    1,
    EngineRuntimeUnitKind::Provider,
    &[newengine_world_api::WORLD_BACKEND_CAPABILITY_ID],
    &[newengine_scene_io::SCENE_BACKEND_CAPABILITY_ID],
    PROVIDER_TAGS,
);
pub const WORLD_ENVIRONMENT_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.world-environment",
    1,
    EngineRuntimeUnitKind::Provider,
    &[newengine_world_environment_api::WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID],
    &[newengine_world_api::WORLD_BACKEND_CAPABILITY_ID],
    PROVIDER_TAGS,
);
pub const ECS_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.ecs",
    1,
    EngineRuntimeUnitKind::Provider,
    &[newengine_ecs_api::ECS_BACKEND_CAPABILITY_ID],
    &[newengine_scene_io::SCENE_BACKEND_CAPABILITY_ID],
    PROVIDER_TAGS,
);
pub const ENTITY_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.entity",
    1,
    EngineRuntimeUnitKind::Provider,
    &[newengine_entity_api::ENTITY_BACKEND_CAPABILITY_ID],
    &[newengine_ecs_api::ECS_BACKEND_CAPABILITY_ID],
    PROVIDER_TAGS,
);
pub const TIME_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.time",
    1,
    EngineRuntimeUnitKind::Provider,
    &[newengine_time_api::TIME_BACKEND_CAPABILITY_ID],
    &[],
    PROVIDER_TAGS,
);
pub const SCHEMA_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.schema",
    1,
    EngineRuntimeUnitKind::Provider,
    &[newengine_schema_api::SCHEMA_BACKEND_CAPABILITY_ID],
    &[],
    PROVIDER_TAGS,
);
pub const SCRIPTING_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.scripting",
    1,
    EngineRuntimeUnitKind::Provider,
    &[newengine_scripting_api::SCRIPTING_BACKEND_CAPABILITY_ID],
    &[],
    PROVIDER_TAGS,
);
pub const GAMEPLAY_FOUNDATION_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.gameplay-foundation",
    1,
    EngineRuntimeUnitKind::Provider,
    &[
        newengine_tags_api::TAGS_REGISTRY_CAPABILITY_ID,
        newengine_tasks_api::TASKS_BACKEND_CAPABILITY_ID,
        newengine_animation_api::ANIMATION_BACKEND_CAPABILITY_ID,
        newengine_navigation_api::NAVIGATION_BACKEND_CAPABILITY_ID,
        newengine_ai_api::AI_BACKEND_CAPABILITY_ID,
    ],
    &[],
    PROVIDER_TAGS,
);
pub const ASSET_TYPES_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.asset-types",
    1,
    EngineRuntimeUnitKind::Provider,
    &[newengine_assets_api::ASSET_TYPES_BACKEND_CAPABILITY_ID],
    &[newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID],
    PROVIDER_TAGS,
);
pub const ASSET_DOCUMENTS_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.asset-documents",
    1,
    EngineRuntimeUnitKind::Provider,
    &[
        newengine_assets_api::ASSETS_INSPECT_BACKEND_CAPABILITY_ID,
        newengine_assets_api::ASSETS_EDIT_BACKEND_CAPABILITY_ID,
    ],
    &[newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID],
    PROVIDER_TAGS,
);
pub const DEFINITIONS_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.definitions",
    1,
    EngineRuntimeUnitKind::Provider,
    &[newengine_assets_api::DEFINITIONS_BACKEND_CAPABILITY_ID],
    &[newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID],
    PROVIDER_TAGS,
);
pub const ASSETS_UI_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.assets-ui",
    1,
    EngineRuntimeUnitKind::Provider,
    &[newengine_assets_api::ASSETS_UI_BACKEND_CAPABILITY_ID],
    &[newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID],
    PROVIDER_TAGS,
);
pub const MATERIALS_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.materials",
    1,
    EngineRuntimeUnitKind::Provider,
    &[newengine_materials::MATERIALS_BACKEND_CAPABILITY_ID],
    &[newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID],
    PROVIDER_TAGS,
);
pub const MODELS_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.models",
    1,
    EngineRuntimeUnitKind::Provider,
    &[newengine_model_domain_api::MODEL_BACKEND_CAPABILITY_ID],
    &[
        newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID,
        newengine_materials::MATERIALS_BACKEND_CAPABILITY_ID,
    ],
    PROVIDER_TAGS,
);
pub const ASSET_GRAPH_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.asset-graph",
    1,
    EngineRuntimeUnitKind::Provider,
    &[newengine_model_domain_api::ASSET_GRAPH_BACKEND_CAPABILITY_ID],
    &[
        newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID,
        newengine_model_domain_api::MODEL_BACKEND_CAPABILITY_ID,
        newengine_assets_api::DEFINITIONS_BACKEND_CAPABILITY_ID,
    ],
    PROVIDER_TAGS,
);

/// Device-backed audio remains a late lifecycle unit. Device creation is deliberately
/// not performed during provider discovery/freeze.
pub const AUDIO_NATIVE_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.audio-native",
    1,
    EngineRuntimeUnitKind::Module,
    &["engine.runtime.audio-native"],
    &[newengine_assets_api::ASSET_BACKEND_CAPABILITY_ID],
    MODULE_TAGS,
);
pub const AUDIO_SCENE_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.audio-scene",
    1,
    EngineRuntimeUnitKind::Module,
    &["engine.runtime.audio-scene"],
    &[
        newengine_scene_io::SCENE_BACKEND_CAPABILITY_ID,
        newengine_audio_api::AUDIO_BACKEND_CAPABILITY_ID,
    ],
    MODULE_TAGS,
);
pub const AUDIO_AMBIENCE_RUNTIME_UNIT_SPEC: EngineRuntimeUnitSpec = EngineRuntimeUnitSpec::new(
    "engine.runtime.audio-ambience",
    1,
    EngineRuntimeUnitKind::Module,
    &["engine.runtime.audio-ambience"],
    &[
        newengine_scene_io::SCENE_BACKEND_CAPABILITY_ID,
        newengine_audio_api::AUDIO_BACKEND_CAPABILITY_ID,
    ],
    MODULE_TAGS,
);

fn scene_bridge(
    engine: &mut Engine<()>,
) -> EngineResult<Arc<newengine_scene_runtime::SceneBridge>> {
    engine
        .resources_mut()
        .get::<Arc<newengine_scene_runtime::SceneBridge>>()
        .cloned()
        .ok_or_else(|| {
            EngineError::Other(
                "runtime-unit requires instance Arc<SceneBridge> resource before materialization"
                    .to_owned(),
            )
        })
}

fn asset_client() -> newengine_assets::AssetServiceClient {
    newengine_assets::AssetServiceClient::new(newengine_plugin_host::default_host_api())
}

fn scene_factory(
    engine: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    let scene = scene_bridge(engine)?;
    let mounts = engine
        .resources_mut()
        .get::<newengine_scene_runtime::SceneGatewayAssetMounts>()
        .copied();
    newengine_scene_runtime::register_scene_gateway_best_effort(scene, mounts);
    Ok(None)
}
fn world_factory(
    engine: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    newengine_world_runtime::register_world_gateway_best_effort(scene_bridge(engine)?);
    Ok(None)
}
fn world_environment_factory(
    _: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    newengine_world_environment_runtime::register_world_environment_gateway_best_effort();
    Ok(None)
}
fn ecs_factory(
    engine: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    newengine_ecs_runtime::register_ecs_gateway_best_effort(scene_bridge(engine)?);
    Ok(None)
}
fn entity_factory(
    engine: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    newengine_entity_runtime::register_entity_gateway_best_effort(scene_bridge(engine)?);
    Ok(None)
}
fn time_factory(
    _: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    let _ = newengine_time_runtime::register_time_gateway_best_effort();
    Ok(None)
}
fn schema_factory(
    _: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    let _ = newengine_schema_runtime::register_schema_gateway_best_effort();
    Ok(None)
}
fn scripting_factory(
    _: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    let _ = newengine_scripting_runtime::register_scripting_gateway_best_effort();
    Ok(None)
}
fn gameplay_foundation_factory(
    _: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    let _ = newengine_gameplay_runtime::register_gameplay_foundation_gateways_best_effort();
    Ok(None)
}
fn asset_types_factory(
    _: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    let _ = newengine_assets::register_asset_types_gateway_best_effort();
    let host = newengine_plugin_host::default_host_api();
    let registered = newengine_asset_format_nef8::descriptors()
        .into_iter()
        .filter(|descriptor| {
            newengine_assets::register_asset_type_descriptor_best_effort(&host, descriptor.clone())
        })
        .count();
    newengine_ulog_api::ulog::info!(
        "runtime-unit asset-types: registered {} provider-owned first-party formats",
        registered
    );
    Ok(None)
}
fn asset_documents_factory(
    _: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    let _ = newengine_assets::register_asset_document_gateways_best_effort(
        newengine_plugin_host::default_host_api(),
    );
    Ok(None)
}
fn definitions_factory(
    _: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    let _ = newengine_definitions_runtime::register_definitions_gateway_best_effort(asset_client());
    Ok(None)
}
fn assets_ui_factory(
    _: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    let _ = newengine_assets_ui_runtime::register_assets_ui_gateway_best_effort(asset_client());
    Ok(None)
}
fn materials_factory(
    _: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    let host = newengine_plugin_host::default_host_api();
    let _ = newengine_material_runtime::register_materials_gateway_best_effort_with_host(
        Some(host),
        asset_client(),
    );
    Ok(None)
}
fn models_factory(
    _: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    let host = newengine_plugin_host::default_host_api();
    let _ =
        newengine_model_runtime::register_model_gateway_best_effort_with_host(host, asset_client());
    Ok(None)
}
fn asset_graph_factory(
    _: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    let host = newengine_plugin_host::default_host_api();
    let _ = newengine_model_runtime::register_asset_graph_gateway_best_effort(host, asset_client());
    Ok(None)
}

struct NativeAudioProviderBootstrapModule;
impl Module<()> for NativeAudioProviderBootstrapModule {
    fn id(&self) -> &'static str {
        "engine.runtime.audio-native"
    }

    fn init(&mut self, _ctx: &mut ModuleCtx<'_, ()>) -> EngineResult<()> {
        if !newengine_plugin_host::has_service(newengine_audio_runtime::NATIVE_AUDIO_SERVICE_ID) {
            let _ =
                newengine_audio_runtime::register_native_audio_provider_best_effort(asset_client());
        }
        Ok(())
    }
}
fn audio_native_factory(
    _: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    Ok(Some(Box::new(NativeAudioProviderBootstrapModule)))
}
fn audio_scene_factory(
    engine: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    Ok(Some(Box::new(
        newengine_engine_runtime::AudioSceneRuntimeModule::new(scene_bridge(engine)?),
    )))
}
fn audio_ambience_factory(
    engine: &mut Engine<()>,
    _: &StartupConfig,
) -> EngineResult<Option<Box<dyn Module<()>>>> {
    Ok(Some(Box::new(
        newengine_engine_runtime::AudioAmbienceRuntimeModule::new(scene_bridge(engine)?),
    )))
}

pub const STATIC_RUNTIME_UNIT_REGISTRATIONS: &[StaticRuntimeUnitRegistration] = &[
    StaticRuntimeUnitRegistration {
        spec: SCENE_RUNTIME_UNIT_SPEC,
        factory: scene_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: WORLD_RUNTIME_UNIT_SPEC,
        factory: world_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: WORLD_ENVIRONMENT_RUNTIME_UNIT_SPEC,
        factory: world_environment_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: ECS_RUNTIME_UNIT_SPEC,
        factory: ecs_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: ENTITY_RUNTIME_UNIT_SPEC,
        factory: entity_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: TIME_RUNTIME_UNIT_SPEC,
        factory: time_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: SCHEMA_RUNTIME_UNIT_SPEC,
        factory: schema_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: SCRIPTING_RUNTIME_UNIT_SPEC,
        factory: scripting_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: GAMEPLAY_FOUNDATION_RUNTIME_UNIT_SPEC,
        factory: gameplay_foundation_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: ASSET_TYPES_RUNTIME_UNIT_SPEC,
        factory: asset_types_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: ASSET_DOCUMENTS_RUNTIME_UNIT_SPEC,
        factory: asset_documents_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: DEFINITIONS_RUNTIME_UNIT_SPEC,
        factory: definitions_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: ASSETS_UI_RUNTIME_UNIT_SPEC,
        factory: assets_ui_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: MATERIALS_RUNTIME_UNIT_SPEC,
        factory: materials_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: MODELS_RUNTIME_UNIT_SPEC,
        factory: models_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: ASSET_GRAPH_RUNTIME_UNIT_SPEC,
        factory: asset_graph_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: AUDIO_NATIVE_RUNTIME_UNIT_SPEC,
        factory: audio_native_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: AUDIO_SCENE_RUNTIME_UNIT_SPEC,
        factory: audio_scene_factory,
    },
    StaticRuntimeUnitRegistration {
        spec: AUDIO_AMBIENCE_RUNTIME_UNIT_SPEC,
        factory: audio_ambience_factory,
    },
];

pub const STANDARD_GAME_RUNTIME_UNITS: &[EngineRuntimeUnitSpec] = &[
    SCENE_RUNTIME_UNIT_SPEC,
    WORLD_RUNTIME_UNIT_SPEC,
    WORLD_ENVIRONMENT_RUNTIME_UNIT_SPEC,
    ECS_RUNTIME_UNIT_SPEC,
    ENTITY_RUNTIME_UNIT_SPEC,
    TIME_RUNTIME_UNIT_SPEC,
    SCHEMA_RUNTIME_UNIT_SPEC,
    SCRIPTING_RUNTIME_UNIT_SPEC,
    GAMEPLAY_FOUNDATION_RUNTIME_UNIT_SPEC,
    ASSET_TYPES_RUNTIME_UNIT_SPEC,
    ASSET_DOCUMENTS_RUNTIME_UNIT_SPEC,
    DEFINITIONS_RUNTIME_UNIT_SPEC,
    ASSETS_UI_RUNTIME_UNIT_SPEC,
    MATERIALS_RUNTIME_UNIT_SPEC,
    MODELS_RUNTIME_UNIT_SPEC,
    ASSET_GRAPH_RUNTIME_UNIT_SPEC,
    AUDIO_NATIVE_RUNTIME_UNIT_SPEC,
    AUDIO_SCENE_RUNTIME_UNIT_SPEC,
    AUDIO_AMBIENCE_RUNTIME_UNIT_SPEC,
];

/// Runtime-unit-only capability roots for the standard game shape. These drive
/// unit selection without leaking into provider/gateway composition requirements.
pub const STANDARD_GAME_RUNTIME_UNIT_REQUIREMENTS: &[RuntimeUnitRequirementSpec] = &[
    RuntimeUnitRequirementSpec::required(newengine_scene_io::SCENE_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_world_api::WORLD_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_world_environment_api::WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_ecs_api::ECS_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_entity_api::ENTITY_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_time_api::TIME_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_schema_api::SCHEMA_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_scripting_api::SCRIPTING_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_tags_api::TAGS_REGISTRY_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_tasks_api::TASKS_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_animation_api::ANIMATION_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_navigation_api::NAVIGATION_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_ai_api::AI_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_assets_api::ASSET_TYPES_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_assets_api::ASSETS_INSPECT_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_assets_api::ASSETS_EDIT_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_assets_api::DEFINITIONS_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_assets_api::ASSETS_UI_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_materials::MATERIALS_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_model_domain_api::MODEL_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required(newengine_model_domain_api::ASSET_GRAPH_BACKEND_CAPABILITY_ID),
    RuntimeUnitRequirementSpec::required("engine.runtime.audio-native"),
    RuntimeUnitRequirementSpec::required("engine.runtime.audio-scene"),
    RuntimeUnitRequirementSpec::required("engine.runtime.audio-ambience"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

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
    fn provides_and_requires_have_real_dependency_semantics() {
        assert!(SCENE_RUNTIME_UNIT_SPEC
            .provides
            .contains(&newengine_scene_io::SCENE_BACKEND_CAPABILITY_ID));
        assert!(!SCENE_RUNTIME_UNIT_SPEC
            .requires
            .contains(&newengine_scene_io::SCENE_BACKEND_CAPABILITY_ID));
        assert!(WORLD_RUNTIME_UNIT_SPEC
            .requires
            .contains(&newengine_scene_io::SCENE_BACKEND_CAPABILITY_ID));
    }
}
