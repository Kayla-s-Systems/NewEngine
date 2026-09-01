use std::sync::Arc;

use newengine_core::{Engine, EngineError, EngineResult};
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
    )
    .with_runtime_unit_requirements(
        newengine_runtime_units::STANDARD_GAME_RUNTIME_UNIT_REQUIREMENTS,
    );

impl GameReadyRuntimeProfile {
    pub fn initialize_composition_services(
        &self,
        engine: &mut Engine<()>,
        host_preinit: &newengine_runtime_host::HostPreInitSnapshot,
        runtime: Option<&RuntimeCompositionContext>,
    ) -> EngineResult<()> {
        // Instance-local resources consumed by static runtime-unit factories. They are
        // installed before host materialization so scene/world/ecs/entity units share the
        // exact same live SceneBridge owned by this runtime profile.
        engine
            .resources_mut()
            .insert::<Arc<newengine_scene_runtime::SceneBridge>>(Arc::clone(&self.scene));
        engine
            .resources_mut()
            .insert(newengine_audio_world_runtime::AudioWorldScene::new(
                self.scene.scene(),
            ));
        newengine_audio_world_runtime::register_audio_gateway_best_effort();
        engine
            .resources_mut()
            .insert(SceneGatewayAssetMounts::from_profile(GAME_READY_MOUNT_SPEC));
        if let Some(provider) = self.game_data_provider.clone() {
            engine
                .resources_mut()
                .insert(crate::runtime_units::GameReadyGameDataProviderOverride(
                    provider,
                ));
        }

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
        // CameraGatewayBridge is created with the profile, before the launcher creates the
        // instance-owned runtime HostContext. Re-publish it here so the authoritative
        // engine.camera route lives in the same registry as the rest of GameReady composition.
        let _ = self.scene.publish_camera_gateway_best_effort();

        // Product-owned registrations only. Generic engine domains are materialized by
        // EngineCompositionSpec.runtime_units through the host runtime-unit catalog.
        register_game_ready_entity_archetypes_best_effort();
    }

    #[inline]
    pub fn bootstrap_content_best_effort(&self) {
        // Game scenes are assembled by GameReadySceneBootstrapModule during engine.start().
    }
}

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
    fn game_ready_selects_distribution_units_without_mirroring_the_catalog() {
        assert!(
            GAME_READY_COMPOSITION_SPEC.runtime_units.is_empty(),
            "GameReady must not duplicate distribution-owned unit descriptors"
        );
        assert_eq!(
            GAME_READY_COMPOSITION_SPEC.runtime_unit_requirements,
            newengine_runtime_units::STANDARD_GAME_RUNTIME_UNIT_REQUIREMENTS,
        );
        let distribution_provides = newengine_runtime_units::STANDARD_GAME_RUNTIME_UNITS
            .iter()
            .flat_map(|unit| unit.provides.iter().copied())
            .collect::<std::collections::BTreeSet<_>>();
        for requirement in GAME_READY_COMPOSITION_SPEC.runtime_unit_requirements {
            assert!(
                distribution_provides.contains(requirement.capability),
                "runtime-unit root has no distribution inventory candidate: {}",
                requirement.capability
            );
        }
    }

    #[test]
    fn game_ready_profile_does_not_install_game_module_composition_roles_directly() {
        let source = include_str!("profile.rs");
        for forbidden in [
            "GameReadyRenderFeaturePack",
            "GameReadyWorldRuntimeProvider",
            "GameReadySceneBootstrapModule",
            "GameReadyWorldSceneBootstrapProvider",
            "game_ready_game_input_profile",
            "set_scene_bootstrap_provider",
        ] {
            assert!(
                !source.contains(forbidden),
                "GameReadyRuntimeProfile regained direct composition knowledge: {forbidden}"
            );
        }
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

    #[test]
    fn gameready_runtime_publishes_authoritative_camera_gateway_in_active_host_context() {
        let _bootstrap_host = newengine_plugin_host::create_host_context();
        let profile = GameReadyRuntimeProfile::new();

        let _runtime_host = newengine_plugin_host::create_host_context();
        assert!(
            newengine_plugin_host::active_engine_gateway_route("engine.camera").is_none(),
            "fresh runtime host context must begin without inherited camera routes"
        );

        profile.register_engine_provider_routes_best_effort();

        let camera = newengine_plugin_host::active_engine_gateway_route("engine.camera")
            .expect("GameReady runtime must publish engine.camera");
        assert_eq!(camera.provider_service_id, "engine.camera");
        assert_eq!(
            camera.provider_route_id.as_deref(),
            Some("engine.camera.stargazer")
        );
        assert_eq!(
            camera.provider_owner_id,
            "newengine-engine-runtime.camera-gateway"
        );
        assert_eq!(camera.backend_capability_id, "camera.backend");
    }
}
