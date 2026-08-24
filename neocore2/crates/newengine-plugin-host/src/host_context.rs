#![forbid(unsafe_op_in_unsafe_fn)]

mod events;
mod gateway;
mod lifecycle;
mod resources;
mod services;
mod state;
mod validation;

pub use events::{emit_plugin_event, publish_event, subscribe_event_sink};
pub use gateway::{
    active_engine_gateway_route, clear_engine_gateway_selection_policies,
    declare_engine_capability_slot, declare_engine_composition, engine_gateway_has_capability,
    install_engine_gateway_selection_policy, list_engine_capability_slots,
    list_engine_gateway_routes, register_engine_gateway_provider_route,
    register_null_engine_gateway_provider_route,
    register_null_engine_gateway_provider_route_with_abi, resolve_service_for_backend_capability,
    resolve_service_for_engine_gateway, validate_required_engine_capability_slots,
    EngineGatewaySelectionPolicy,
};
pub use lifecycle::{shutdown_services_by_owner, unregister_by_owner};
pub use resources::{
    list_external_runtime_descriptors, list_external_runtime_plugins,
    register_external_runtime_plugin,
};
pub use services::{describe_service, has_service, list_services};
pub use state::{
    bump_services_generation, init_host_context, services_generation, EngineCapabilitySlotSnapshot,
    EngineGatewayRouteSnapshot, ExternalRuntimePluginSnapshot,
};
pub(crate) use state::{ctx, current_plugin_id, with_current_plugin_id, ServiceEntry};
pub(crate) use validation::{plugin_declares_provided_service, register_plugin_descriptor};
