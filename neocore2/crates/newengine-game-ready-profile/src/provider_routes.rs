use std::sync::Arc;

use newengine_asset_bootstrap_runtime::ProfileMountSpec;
use newengine_core::{Engine, EngineError, EngineResult, Module, ModuleCtx};
use newengine_project_runtime::RuntimeCompositionContext;
use newengine_scene_runtime::SceneGatewayAssetMounts;

use crate::entity_archetypes::register_game_ready_entity_archetypes_best_effort;
use crate::{GameReadyRuntimeProfile, GAME_READY_MOUNT_SPEC};

const GAME_READY_CAPABILITY_SLOTS: &[newengine_service_api::EngineCapabilitySlotSpec] = &[
    newengine_service_api::EngineCapabilitySlotSpec::required("engine.assets", "assets"),
    newengine_service_api::EngineCapabilitySlotSpec::required("engine.assets.maps", "assets.maps"),
    newengine_service_api::EngineCapabilitySlotSpec::required(
        "engine.assets.textures",
        "assets.textures",
    ),
    newengine_service_api::EngineCapabilitySlotSpec::required(
        "engine.assets.materials",
        "assets.materials",
    ),
    newengine_service_api::EngineCapabilitySlotSpec::required("engine.render", "render"),
    newengine_service_api::EngineCapabilitySlotSpec::required("engine.physics", "physics"),
    newengine_service_api::EngineCapabilitySlotSpec::required("engine.input", "input"),
    newengine_service_api::EngineCapabilitySlotSpec::required("engine.scene", "scene"),
    newengine_service_api::EngineCapabilitySlotSpec::required("engine.world", "world"),
    newengine_service_api::EngineCapabilitySlotSpec::required("engine.ui", "ui"),
    newengine_service_api::EngineCapabilitySlotSpec::optional("engine.audio", "audio"),
    newengine_service_api::EngineCapabilitySlotSpec::optional("engine.ui.notify", "ui.notify"),
    newengine_service_api::EngineCapabilitySlotSpec::optional("engine.time", "time"),
    newengine_service_api::EngineCapabilitySlotSpec::optional("engine.schema", "schema"),
    newengine_service_api::EngineCapabilitySlotSpec::optional("engine.scripting", "scripting"),
];

pub const GAME_READY_COMPOSITION_SPEC: newengine_service_api::EngineCompositionSpec =
    newengine_service_api::EngineCompositionSpec::new(
        "newengine.composition.game-ready",
        GAME_READY_CAPABILITY_SLOTS,
    );

impl GameReadyRuntimeProfile {
    pub fn declare_composition_capability_slots(&self) -> EngineResult<()> {
        newengine_plugin_host::declare_engine_composition(GAME_READY_COMPOSITION_SPEC)
            .map_err(EngineError::Other)
    }

    pub fn initialize_composition_services(
        &self,
        engine: &mut Engine<()>,
        host_preinit: &newengine_runtime_host::HostPreInitSnapshot,
        runtime: Option<&RuntimeCompositionContext>,
    ) -> EngineResult<()> {
        self.declare_composition_capability_slots()?;
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
        // Safe control-plane routes can be refreshed during init. The physical audio gateway is
        // intentionally excluded here: publishing a new host service while the startup FSM is
        // executing Module::init() can re-enter the service registry and deadlock the main thread.
        self.profile.register_engine_provider_routes_best_effort();
        newengine_ulog_api::ulog::info!(
            "game-ready provider bootstrap: phase='init' routes_refreshed=true audio_provider='plugin-owned'"
        );
        Ok(())
    }
}

#[allow(dead_code)]
const _: Option<ProfileMountSpec> = None;
