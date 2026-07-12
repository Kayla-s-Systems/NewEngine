use std::sync::Arc;

use newengine_service_kit::{
    register_engine_gateway_provider_service_dynamic, EngineGatewayProviderDeclDynamic,
};
use newengine_world_api::{ENGINE_WORLD_SERVICE_ID, WORLD_BACKEND_CAPABILITY_ID, WORLD_SERVICE_ID};

use crate::{
    router::world_gateway_service,
    service::{WORLD_FOUNDATION_PROVIDER_ROUTE, WORLD_GATEWAY_OWNER},
};

pub fn register_world_gateway_best_effort(scene: Arc<newengine_scene_runtime::SceneBridge>) {
    if newengine_plugin_host::has_service(ENGINE_WORLD_SERVICE_ID) {
        newengine_ulog_api::ulog::debug!(
            "engine.world gateway registration skipped; service already available"
        );
        return;
    }

    let service = world_gateway_service(scene);
    match register_engine_gateway_provider_service_dynamic(EngineGatewayProviderDeclDynamic {
        gateway: ENGINE_WORLD_SERVICE_ID,
        service_kind: "world",
        provider_service: WORLD_SERVICE_ID,
        provider_route: WORLD_FOUNDATION_PROVIDER_ROUTE,
        capability: WORLD_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: WORLD_GATEWAY_OWNER,
        service,
    }) {
        Ok(()) => newengine_ulog_api::ulog::info!(
            "engine.world gateway registered source=engine-runtime service='{}' provider_service='{}' capability='{}' owner='{}' semantics='living runtime world; scene remains authored structure'",
            ENGINE_WORLD_SERVICE_ID,
            WORLD_SERVICE_ID,
            WORLD_BACKEND_CAPABILITY_ID,
            WORLD_GATEWAY_OWNER
        ),
        Err(error) => newengine_ulog_api::ulog::error!(
            "engine.world gateway registration failed id='{}' err='{}'",
            ENGINE_WORLD_SERVICE_ID,
            error
        ),
    }
}
