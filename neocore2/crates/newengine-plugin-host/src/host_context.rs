#![forbid(unsafe_op_in_unsafe_fn)]

mod contracts;
mod events;
mod gateway;
mod lifecycle;
mod resources;
mod services;
mod state;
mod validation;

pub use contracts::{
    list_runtime_contracts, runtime_contract, runtime_contract_by_advertised_id,
    RuntimeContractAuthority, RuntimeContractEntry, RuntimeContractSpec,
};
pub use events::{emit_plugin_event, publish_event, subscribe_event_sink};
pub use gateway::{
    active_engine_gateway_route, clear_engine_gateway_selection_policies,
    declare_engine_capability_requirement, declare_engine_capability_slot,
    declare_engine_composition, engine_composition_allows_system_tags,
    engine_composition_explanation, engine_composition_has_forbidden_system_tags,
    engine_composition_snapshot_v1, engine_composition_snapshot_v1_json,
    engine_gateway_has_capability, explain_engine_gateway_composition,
    install_engine_gateway_selection_policy, list_engine_capability_slots,
    list_engine_gateway_routes, register_engine_gateway_provider_route,
    register_null_engine_gateway_provider_route,
    register_null_engine_gateway_provider_route_with_abi,
    register_null_engine_gateway_provider_route_with_abi_and_tags,
    register_null_engine_gateway_provider_route_with_tags, resolve_service_for_backend_capability,
    resolve_service_for_engine_gateway, validate_required_engine_capability_slots,
    EngineGatewaySelectionPolicy,
};
pub(crate) use gateway::{composition_planning_snapshot, CompositionPlanningSnapshot};
pub use lifecycle::ProviderRegistrationTransaction;

pub(crate) use lifecycle::{
    begin_provider_transaction, commit_provider_transaction, quiesce_provider_publication,
    restore_provider_publication, rollback_provider_transaction,
    shutdown_provider_publication_services, snapshot_provider_publication,
    stage_event_sink_registration, stage_gateway_route_registration, stage_service_registration,
    validate_provider_transaction, wait_for_provider_publication_quiescence,
};
pub use lifecycle::{shutdown_services_by_owner, unregister_by_owner};
pub use resources::{
    list_external_runtime_descriptors, list_external_runtime_plugins,
    register_external_runtime_plugin,
};
pub use services::{describe_service, has_service, list_services};
pub use state::{
    activate_host_context, bump_services_generation, create_host_context,
    create_host_context_with_environment_snapshot, current_host_context, init_host_context,
    services_generation, with_host_context, with_host_module_callback,
    EngineCapabilitySlotSnapshot, EngineGatewayRouteSnapshot, ExternalRuntimePluginSnapshot,
    HostContextHandle,
};
pub(crate) use state::{
    ctx, current_host_context_identity, current_plugin_id, environment_snapshot_utf8,
    environment_var, environment_var_os, reject_topology_mutation_from_host_callback,
    with_current_plugin_id, ServiceCallLease, ServiceEntry, ServiceLifecycle,
};
pub(crate) use validation::{
    capability_provider_candidates, missing_typed_descriptor_requirements,
    plugin_declares_provided_service, register_plugin_descriptor,
};
