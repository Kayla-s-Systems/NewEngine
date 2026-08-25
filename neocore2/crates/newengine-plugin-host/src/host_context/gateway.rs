mod capability;
mod policy;
mod registry;
mod routes;
mod slots;

pub use capability::resolve_service_for_backend_capability;
pub use policy::{
    clear_engine_gateway_selection_policies, install_engine_gateway_selection_policy,
    EngineGatewaySelectionPolicy,
};
pub(crate) use registry::active_engine_gateways;
pub use routes::{
    active_engine_gateway_route, engine_gateway_has_capability, list_engine_gateway_routes,
    register_engine_gateway_provider_route, register_null_engine_gateway_provider_route,
    register_null_engine_gateway_provider_route_with_abi, resolve_service_for_engine_gateway,
};
pub use slots::{
    declare_engine_capability_requirement, declare_engine_capability_slot,
    declare_engine_composition, list_engine_capability_slots,
    validate_required_engine_capability_slots,
};

#[cfg(test)]
mod gateway_diagnostic_tests {
    use super::*;

    #[test]
    fn gateway_resolution_diagnostics_emit_only_when_resolution_changes() {
        let gateway = "engine.test.diagnostic-dedupe";
        assert!(registry::should_emit_gateway_resolution(
            gateway,
            "selected:one"
        ));
        assert!(!registry::should_emit_gateway_resolution(
            gateway,
            "selected:one"
        ));
        assert!(registry::should_emit_gateway_resolution(
            gateway,
            "selected:two"
        ));
        assert!(!registry::should_emit_gateway_resolution(
            gateway,
            "selected:two"
        ));
        assert!(registry::should_emit_gateway_resolution(gateway, "missing"));
        assert!(!registry::should_emit_gateway_resolution(
            gateway, "missing"
        ));
    }

    #[test]
    fn optional_capability_slot_may_remain_empty() {
        let gateway = "engine.kernel-slot-test.optional";
        declare_engine_capability_slot(
            gateway,
            "kernel-slot-test.optional",
            false,
            "newengine-plugin-host.test",
        )
        .expect("declare optional capability slot");

        let slot = list_engine_capability_slots()
            .into_iter()
            .find(|slot| slot.gateway_id == gateway)
            .expect("declared capability slot");
        assert_eq!(slot.state, "empty");
        assert!(!slot.required);
        assert_eq!(
            slot.requirement_level,
            newengine_service_api::CapabilityRequirementLevel::Optional
        );
        assert!(slot.provider_service_id.is_none());
    }
}
