#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RResult;
use newengine_plugin_api::ServiceV1Dyn;
use newengine_service_api::EngineServiceKind;

pub struct EngineOwnedGatewayDecl {
    pub gateway: &'static str,
    pub service_kind: EngineServiceKind,
    pub provider_service: &'static str,
    pub capability: &'static str,
    pub priority: i32,
    pub owner: &'static str,
    pub service: ServiceV1Dyn<'static>,
}

/// Dynamic variant for gateway providers whose domain is declared by data, not
/// by the historical `EngineServiceKind` convenience enum. Prefer this for new
/// third-level domains and profile/plugin-owned feature surfaces.
pub struct EngineOwnedGatewayDeclDynamic {
    pub gateway: &'static str,
    pub service_kind: &'static str,
    pub provider_service: &'static str,
    pub capability: &'static str,
    pub priority: i32,
    pub owner: &'static str,
    pub service: ServiceV1Dyn<'static>,
}

pub fn register_engine_owned_gateway_service(decl: EngineOwnedGatewayDecl) -> Result<(), String> {
    let service_id = decl.service.id().to_string();
    if service_id != decl.provider_service {
        return Err(format!(
            "engine-owned gateway service id mismatch: declared='{}' actual='{}'",
            decl.provider_service, service_id
        ));
    }

    match newengine_plugin_host::host_register_service_impl(decl.service) {
        RResult::ROk(()) => {}
        RResult::RErr(e) => return Err(e.to_string()),
    }

    newengine_plugin_host::register_engine_owned_gateway(
        decl.gateway,
        decl.service_kind,
        decl.provider_service,
        decl.capability,
        decl.priority,
        decl.owner,
    )
}

pub fn register_engine_owned_gateway_service_dynamic(decl: EngineOwnedGatewayDeclDynamic) -> Result<(), String> {
    let service_id = decl.service.id().to_string();
    if service_id != decl.provider_service {
        return Err(format!(
            "engine-owned gateway service id mismatch: declared='{}' actual='{}'",
            decl.provider_service, service_id
        ));
    }

    match newengine_plugin_host::host_register_service_impl(decl.service) {
        RResult::ROk(()) => {}
        RResult::RErr(e) => return Err(e.to_string()),
    }

    newengine_plugin_host::register_engine_owned_gateway(
        decl.gateway,
        decl.service_kind,
        decl.provider_service,
        decl.capability,
        decl.priority,
        decl.owner,
    )
}

pub fn register_engine_owned_gateway_service_best_effort(decl: EngineOwnedGatewayDecl) -> bool {
    let gateway = decl.gateway;
    let capability = decl.capability;
    let owner = decl.owner;
    match register_engine_owned_gateway_service(decl) {
        Ok(()) => {
            log::info!(
                "engine-owned gateway registered gateway='{}' capability='{}' owner='{}'",
                gateway,
                capability,
                owner
            );
            true
        }
        Err(e) => {
            log::warn!(
                "engine-owned gateway registration skipped gateway='{}' capability='{}' owner='{}' err='{}'",
                gateway,
                capability,
                owner,
                e
            );
            false
        }
    }
}

pub fn register_engine_owned_gateway_service_dynamic_best_effort(decl: EngineOwnedGatewayDeclDynamic) -> bool {
    let gateway = decl.gateway;
    let capability = decl.capability;
    let owner = decl.owner;
    match register_engine_owned_gateway_service_dynamic(decl) {
        Ok(()) => {
            log::info!(
                "engine-owned dynamic gateway registered gateway='{}' capability='{}' owner='{}'",
                gateway,
                capability,
                owner
            );
            true
        }
        Err(e) => {
            log::warn!(
                "engine-owned dynamic gateway registration skipped gateway='{}' capability='{}' owner='{}' err='{}'",
                gateway,
                capability,
                owner,
                e
            );
            false
        }
    }
}

