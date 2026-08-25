use std::sync::Arc;

use newengine_asset_bootstrap_runtime::ProfileMountSpec;
use newengine_core::{Engine, EngineError, EngineResult, Module, ModuleCtx};
use newengine_project_runtime::RuntimeCompositionContext;
use newengine_scene_runtime::SceneGatewayAssetMounts;

use crate::entity_archetypes::register_game_ready_entity_archetypes_best_effort;
use crate::{GameReadyRuntimeProfile, GAME_READY_MOUNT_SPEC};

const GAME_READY_REQUIREMENTS: &[newengine_service_api::CapabilityRequirement] = &[
    newengine_service_api::CapabilityRequirement::required(
        newengine_assets_api::ASSET_BACKEND_SERVICE_SPEC.capability(),
    ),
    newengine_service_api::CapabilityRequirement::required(
        newengine_assets_api::MAPS_BACKEND_SERVICE_SPEC.capability(),
    ),
    newengine_service_api::CapabilityRequirement::required(
        newengine_assets_api::TEXTURES_BACKEND_SERVICE_SPEC.capability(),
    ),
    newengine_service_api::CapabilityRequirement::required(
        newengine_materials::MATERIALS_BACKEND_SERVICE_SPEC.capability(),
    ),
    newengine_service_api::CapabilityRequirement::required(
        newengine_render_api::RENDER_BACKEND_SERVICE_SPEC.capability(),
    ),
    newengine_service_api::CapabilityRequirement::required(
        newengine_physics_api::PHYSICS_BACKEND_SERVICE_SPEC.capability(),
    ),
    newengine_service_api::CapabilityRequirement::required(
        newengine_input_api::INPUT_BACKEND_SERVICE_SPEC.capability(),
    ),
    newengine_service_api::CapabilityRequirement::required(
        newengine_scene_io::SCENE_BACKEND_SERVICE_SPEC.capability(),
    ),
    newengine_service_api::CapabilityRequirement::required(
        newengine_world_api::WORLD_BACKEND_SERVICE_SPEC.capability(),
    ),
    newengine_service_api::CapabilityRequirement::required(
        newengine_ui_api::UI_BACKEND_SERVICE_SPEC.capability(),
    ),
    newengine_service_api::CapabilityRequirement::optional(
        newengine_audio_api::AUDIO_BACKEND_SERVICE_SPEC.capability(),
    ),
    newengine_service_api::CapabilityRequirement::optional(
        newengine_ui_api::UI_NOTIFY_BACKEND_SERVICE_SPEC.capability(),
    ),
    newengine_service_api::CapabilityRequirement::optional(
        newengine_time_api::TIME_BACKEND_SERVICE_SPEC.capability(),
    ),
    newengine_service_api::CapabilityRequirement::optional(
        newengine_schema_api::SCHEMA_BACKEND_SERVICE_SPEC.capability(),
    ),
    newengine_service_api::CapabilityRequirement::optional(
        newengine_scripting_api::SCRIPTING_BACKEND_SERVICE_SPEC.capability(),
    ),
];

pub const GAME_READY_COMPOSITION_SPEC: newengine_service_api::EngineCompositionSpec =
    newengine_service_api::EngineCompositionSpec::new(
        "newengine.composition.game-ready",
        GAME_READY_REQUIREMENTS,
    );

impl GameReadyRuntimeProfile {
    pub fn initialize_composition_services(
        &self,
        engine: &mut Engine<()>,
        host_preinit: &newengine_runtime_host::HostPreInitSnapshot,
        runtime: Option<&RuntimeCompositionContext>,
    ) -> EngineResult<()> {
        newengine_ulog_api::ulog::info!(
            "composition host capabilities: logical_cores={} gpu={} preferred_gpu='{}' provider_hints={}",
            host_preinit.capabilities.cpu.logical_cores.map(|value| value.to_string()).unwrap_or_else(|| "<unknown>".to_owned()),
            host_preinit.capabilities.gpu.len(),
            host_preinit.runtime_policy.preferred_gpu_stable_id.as_deref().unwrap_or("<none>"),
            host_preinit.runtime_policy.provider_hints.len(),
        );

        let game_message_registry = newengine_game_events_runtime::GameMessageRegistry::default();
        let game_message_queue = newengine_game_events_runtime::GameMessageQueue::default();
        newengine_game_events_runtime::init_game_events_service_with_event_hub(
            game_message_registry.clone(),
            game_message_queue.clone(),
            engine.events().clone(),
        );
        engine.resources_mut().insert(game_message_registry);
        engine.resources_mut().insert(game_message_queue);

        let replication_registry =
            newengine_replication_runtime::ReplicationDescriptorRegistry::default();
        if let Some(runtime) = runtime {
            let report = newengine_replication_runtime::load_replication_definitions_from_roots(
                &runtime.runtime_root,
                &runtime.definitions,
                &replication_registry,
            )
            .map_err(EngineError::Other)?;
            if !report.files.is_empty() {
                newengine_ulog_api::ulog::info!(
                    "composition replication definitions: loaded_files={} components={} profiles={} messages={}",
                    report.files.len(),
                    report.components,
                    report.entity_profiles,
                    report.messages,
                );
            }
        }

        let network_runtime =
            newengine_network_runtime::init_network_service(replication_registry.clone());
        engine.resources_mut().insert(network_runtime);
        engine.resources_mut().insert(replication_registry);

        if runtime.is_some() {
            engine.register_module(Box::new(
                newengine_game_module_runtime::GameModuleContractModule::new(),
            ))?;
        }
        Ok(())
    }

    #[inline]
    pub fn register_engine_provider_routes_best_effort(&self) {
        register_game_ready_entity_archetypes_best_effort();
        let asset_mounts = SceneGatewayAssetMounts::from_profile(GAME_READY_MOUNT_SPEC);
        newengine_scene_runtime::register_scene_gateway_best_effort(
            Arc::clone(&self.scene),
            Some(asset_mounts),
        );
        newengine_world_runtime::register_world_gateway_best_effort(Arc::clone(&self.scene));
        newengine_world_environment_runtime::register_world_environment_gateway_best_effort();
        newengine_ecs_runtime::register_ecs_gateway_best_effort(Arc::clone(&self.scene));
        newengine_entity_runtime::register_entity_gateway_best_effort(Arc::clone(&self.scene));
        self.register_input_bindings_gateway_best_effort();
        newengine_time_runtime::register_time_gateway_best_effort();
        newengine_schema_runtime::register_schema_gateway_best_effort();
        newengine_scripting_runtime::register_scripting_gateway_best_effort();
        newengine_gameplay_runtime::register_gameplay_foundation_gateways_best_effort();
        newengine_assets::register_asset_types_gateway_best_effort();

        let host_api = newengine_plugin_host::default_host_api();
        let registered_file_types = newengine_asset_format_nef8::descriptors()
            .into_iter()
            .filter(|descriptor| {
                newengine_assets::register_asset_type_descriptor_best_effort(
                    &host_api,
                    descriptor.clone(),
                )
            })
            .count();
        newengine_ulog_api::ulog::info!(
            "asset type descriptors: registered {} provider-owned first-party formats",
            registered_file_types
        );
        let asset_document_routes_ok =
            newengine_assets::register_asset_document_gateways_best_effort(host_api.clone());
        newengine_ulog_api::ulog::info!(
            "asset document gateways: registered={} routes='engine.assets.inspect,engine.assets.edit'",
            asset_document_routes_ok
        );
        let asset_client = newengine_assets::AssetServiceClient::new(host_api.clone());
        newengine_definitions_runtime::register_definitions_gateway_best_effort(
            asset_client.clone(),
        );
        newengine_assets_ui_runtime::register_assets_ui_gateway_best_effort(asset_client.clone());
        newengine_material_runtime::register_materials_gateway_best_effort_with_host(
            Some(host_api.clone()),
            asset_client.clone(),
        );
        newengine_model_runtime::register_model_gateway_best_effort_with_host(
            host_api.clone(),
            asset_client.clone(),
        );
        newengine_model_runtime::register_asset_graph_gateway_best_effort(host_api, asset_client);
    }

    #[inline]
    pub fn bootstrap_content_best_effort(&self) {
        // Game scenes are assembled by GameReadySceneBootstrapModule during engine.start().
    }
}

/// Startup-phase barrier for provider routes that depend on engine plugins such as
/// AssetManager already being loaded. RuntimeHost loads engine plugins before module
/// init, so this is the first safe point for definitions/audio publication.
pub(crate) struct GameReadyProviderBootstrapModule {
    profile: GameReadyRuntimeProfile,
}

impl GameReadyProviderBootstrapModule {
    #[inline]
    pub(crate) fn new(profile: GameReadyRuntimeProfile) -> Self {
        Self { profile }
    }
}

impl<E: Send + 'static> Module<E> for GameReadyProviderBootstrapModule {
    fn id(&self) -> &'static str {
        "engine.provider-routes.gameready"
    }

    fn init(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        // Core executes Module::init() inside ProviderRegistrationTransaction. Every service/route
        // published here is staged, validated, and committed as one topology epoch after init returns.
        self.profile.register_engine_provider_routes_best_effort();

        let audio_provider =
            if newengine_plugin_host::has_service(newengine_audio_runtime::NATIVE_AUDIO_SERVICE_ID)
            {
                "already-available"
            } else {
                let host_api = newengine_plugin_host::default_host_api();
                let asset_client = newengine_assets::AssetServiceClient::new(host_api);
                if newengine_audio_runtime::register_native_audio_provider_best_effort(asset_client)
                {
                    "native-staged"
                } else {
                    "fallback-only"
                }
            };

        newengine_ulog_api::ulog::info!(
            "game-ready provider bootstrap: phase='init' routes_staged=true audio_provider='{}' transaction='stage-validate-commit'",
            audio_provider
        );
        Ok(())
    }
}

#[allow(dead_code)]
const _: Option<ProfileMountSpec> = None;

#[cfg(test)]
mod composition_architecture_tests {
    use super::*;
    use newengine_service_api::RequirementStrength;

    #[test]
    fn game_ready_composition_is_the_product_backend_specification() {
        let actual = GAME_READY_REQUIREMENTS
            .iter()
            .map(|requirement| (requirement.capability.as_str(), requirement.strength))
            .collect::<Vec<_>>();
        let expected = vec![
            ("asset_manager.backend", RequirementStrength::Required),
            ("assets.maps.backend", RequirementStrength::Required),
            ("assets.textures.backend", RequirementStrength::Required),
            ("assets.materials.backend", RequirementStrength::Required),
            ("render.backend", RequirementStrength::Required),
            ("physics.backend", RequirementStrength::Required),
            ("input.backend", RequirementStrength::Required),
            ("scene.backend", RequirementStrength::Required),
            ("world.backend", RequirementStrength::Required),
            ("ui.backend", RequirementStrength::Required),
            ("audio.backend", RequirementStrength::Optional),
            ("ui.notify.backend", RequirementStrength::Optional),
            ("time.backend", RequirementStrength::Optional),
            ("schema.registry", RequirementStrength::Optional),
            ("scripting.backend", RequirementStrength::Optional),
        ];
        assert_eq!(actual, expected);
    }

    #[test]
    fn game_ready_profile_does_not_name_backend_runtime_modules() {
        let source = include_str!("profile.rs");
        let render_module = ["RenderBackend", "RuntimeModule"].concat();
        let physics_module = ["PhysicsBackend", "RuntimeModule"].concat();
        assert!(!source.contains(&render_module));
        assert!(!source.contains(&physics_module));
    }

    #[test]
    fn game_ready_manifest_has_no_backend_adapter_dependencies() {
        let manifest = include_str!("../Cargo.toml");
        let render_dependency = ["newengine-render-runtime", "-adapter ="].concat();
        let physics_dependency = ["newengine-physics-runtime", "-adapter ="].concat();
        assert!(!manifest.contains(&render_dependency));
        assert!(!manifest.contains(&physics_dependency));
        assert!(manifest.contains("features = [\"full-runtime\"]"));
    }
}
