pub(crate) struct HostContext {
    pub(crate) instance_id: u64,
    pub(crate) services: Mutex<NeHashMap<String, ServiceEntry>>,
    pub(crate) services_generation: AtomicU64,
    pub(crate) event_sinks: Mutex<Arc<[EventSinkEntry]>>,
    pub(crate) plugin_descriptors: Mutex<NeHashMap<String, PluginDescriptor>>,
    pub(crate) plugin_descriptors_v2:
        Mutex<NeHashMap<String, newengine_plugin_api::PluginDescriptorV2>>,
    pub(crate) plugin_origins:
        Mutex<NeHashMap<String, crate::service_gateway::GatewayProviderOrigin>>,
    pub(crate) runtime_contract_catalog: Mutex<RuntimeContractCatalog>,
    pub(crate) external_runtime_plugins: Mutex<NeHashMap<String, ExternalRuntimePluginEntry>>,
    pub(crate) gateway_provider_routes: Mutex<NeHashMap<String, GatewayProviderRouteEntry>>,
    pub(crate) capability_slots: Mutex<NeHashMap<String, EngineCapabilitySlotEntry>>,
    pub(crate) composition_policy: Mutex<DeclaredCompositionPolicy>,
    pub(crate) gateway_selection_policies:
        Mutex<NeHashMap<String, crate::host_context::gateway::EngineGatewaySelectionPolicy>>,
    pub(crate) gateway_registry_cache: Mutex<Option<GatewayRegistryCache>>,
    pub(crate) frozen_composition_plan: RwLock<Option<Arc<newengine_service_api::CompositionPlan>>>,
    pub(crate) provider_transaction: Mutex<Option<ProviderTransactionState>>,
    pub(crate) plugin_config_store:
        Mutex<Option<Arc<crate::plugin_config_service::PluginConfigStore>>>,
    pub(crate) host_job_seq: AtomicU64,
    pub(crate) active_host_jobs: Mutex<NeHashMap<String, crate::diagnostics::PluginHostJobRecord>>,
    pub(crate) plugin_root_observers: RwLock<crate::root_observers::PluginRootObserverState>,
    pub(crate) invalid_gateway_route_warnings: Mutex<NeHashSet<String>>,
    pub(crate) gateway_resolution_diagnostics: Mutex<NeHashMap<String, String>>,
    pub(crate) warned_retired_capabilities: AtomicBool,
    /// Per-Engine snapshot of process/bootstrap environment. Runtime policy must
    /// read this snapshot instead of observing later process-global mutations.
    pub(crate) environment: RwLock<Arc<NeHashMap<OsString, OsString>>>,
}
