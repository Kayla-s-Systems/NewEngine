#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RResult;
use newengine_plugin_api::ServiceV1Dyn;
use newengine_service_api::EngineServiceKind;

pub struct EngineGatewayProviderDecl {
    /// Public engine API gateway, for example `engine.camera` or `engine.assets.textures`.
    pub gateway: &'static str,
    pub service_kind: EngineServiceKind,
    pub provider_service: &'static str,
    /// Provider implementation route identity. This is metadata, not the public API.
    /// Built-in providers must declare it the same way external/plugin providers do.
    pub provider_route: &'static str,
    pub capability: &'static str,
    pub priority: i32,
    pub owner: &'static str,
    pub service: ServiceV1Dyn<'static>,
}

/// Dynamic variant for gateway providers whose domain is declared by data, not
/// by the historical `EngineServiceKind` convenience enum. Prefer this for new
/// third-level domains and profile/plugin-owned feature surfaces.
pub struct EngineGatewayProviderDeclDynamic {
    /// Public engine API gateway, for example `engine.jobs` or a data-declared child gateway.
    pub gateway: &'static str,
    pub service_kind: &'static str,
    pub provider_service: &'static str,
    /// Provider implementation route identity. This is metadata, not the public API.
    pub provider_route: &'static str,
    pub capability: &'static str,
    pub priority: i32,
    pub owner: &'static str,
    pub service: ServiceV1Dyn<'static>,
}

pub struct NullEngineGatewayProviderDeclDynamic {
    pub gateway: &'static str,
    pub service_kind: &'static str,
    pub provider_service: &'static str,
    pub provider_route: &'static str,
    pub capability: &'static str,
    pub owner: &'static str,
    pub service: ServiceV1Dyn<'static>,
}

pub fn register_engine_gateway_provider_service(
    decl: EngineGatewayProviderDecl,
) -> Result<(), String> {
    let service_id = decl.service.id().to_string();
    if service_id != decl.provider_service {
        return Err(format!(
            "engine-runtime route service id mismatch: declared='{}' actual='{}'",
            decl.provider_service, service_id
        ));
    }

    match newengine_plugin_host::host_register_service_impl(decl.service) {
        RResult::ROk(()) => {}
        RResult::RErr(e) => return Err(e.to_string()),
    }

    newengine_plugin_host::register_engine_gateway_provider_route(
        decl.gateway,
        decl.service_kind,
        decl.provider_service,
        decl.provider_route,
        decl.capability,
        decl.priority,
        decl.owner,
    )
}

pub fn register_engine_gateway_provider_service_dynamic(
    decl: EngineGatewayProviderDeclDynamic,
) -> Result<(), String> {
    let service_id = decl.service.id().to_string();
    if service_id != decl.provider_service {
        return Err(format!(
            "engine-runtime route service id mismatch: declared='{}' actual='{}'",
            decl.provider_service, service_id
        ));
    }

    match newengine_plugin_host::host_register_service_impl(decl.service) {
        RResult::ROk(()) => {}
        RResult::RErr(e) => return Err(e.to_string()),
    }

    newengine_plugin_host::register_engine_gateway_provider_route(
        decl.gateway,
        decl.service_kind,
        decl.provider_service,
        decl.provider_route,
        decl.capability,
        decl.priority,
        decl.owner,
    )
}

pub fn register_null_engine_gateway_provider_service_dynamic(
    decl: NullEngineGatewayProviderDeclDynamic,
) -> Result<(), String> {
    let service_id = decl.service.id().to_string();
    if service_id != decl.provider_service {
        return Err(format!(
            "null-provider route service id mismatch: declared='{}' actual='{}'",
            decl.provider_service, service_id
        ));
    }

    match newengine_plugin_host::host_register_service_impl(decl.service) {
        RResult::ROk(()) => {}
        RResult::RErr(e) => return Err(e.to_string()),
    }

    newengine_plugin_host::register_null_engine_gateway_provider_route(
        decl.gateway,
        decl.service_kind,
        decl.provider_service,
        decl.provider_route,
        decl.capability,
        decl.owner,
    )
}

pub fn register_engine_gateway_provider_service_best_effort(
    decl: EngineGatewayProviderDecl,
) -> bool {
    let gateway = decl.gateway;
    let capability = decl.capability;
    let provider_route = decl.provider_route;
    let owner = decl.owner;
    match register_engine_gateway_provider_service(decl) {
        Ok(()) => {
            newengine_ulog_api::ulog::info!(
                "engine-runtime route registered gateway='{}' provider_route='{}' capability='{}' owner='{}'",
                gateway,
                provider_route,
                capability,
                owner
            );
            true
        }
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "engine-runtime route registration skipped gateway='{}' provider_route='{}' capability='{}' owner='{}' err='{}'",
                gateway,
                provider_route,
                capability,
                owner,
                e
            );
            false
        }
    }
}

pub fn register_engine_gateway_provider_service_dynamic_best_effort(
    decl: EngineGatewayProviderDeclDynamic,
) -> bool {
    let gateway = decl.gateway;
    let capability = decl.capability;
    let provider_route = decl.provider_route;
    let owner = decl.owner;
    match register_engine_gateway_provider_service_dynamic(decl) {
        Ok(()) => {
            newengine_ulog_api::ulog::info!(
                "engine-runtime route registered gateway='{}' provider_route='{}' capability='{}' owner='{}'",
                gateway,
                provider_route,
                capability,
                owner
            );
            true
        }
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "engine-runtime route registration skipped gateway='{}' provider_route='{}' capability='{}' owner='{}' err='{}'",
                gateway,
                provider_route,
                capability,
                owner,
                e
            );
            false
        }
    }
}

pub fn register_null_engine_gateway_provider_service_dynamic_best_effort(
    decl: NullEngineGatewayProviderDeclDynamic,
) -> bool {
    let gateway = decl.gateway;
    let capability = decl.capability;
    let provider_route = decl.provider_route;
    let owner = decl.owner;
    match register_null_engine_gateway_provider_service_dynamic(decl) {
        Ok(()) => {
            newengine_ulog_api::ulog::info!(
                "null-provider route registered gateway='{}' provider_route='{}' capability='{}' owner='{}'",
                gateway,
                provider_route,
                capability,
                owner
            );
            true
        }
        Err(e) => {
            newengine_ulog_api::ulog::warn!(
                "null-provider route registration skipped gateway='{}' provider_route='{}' capability='{}' owner='{}' err='{}'",
                gateway,
                provider_route,
                capability,
                owner,
                e
            );
            false
        }
    }
}
