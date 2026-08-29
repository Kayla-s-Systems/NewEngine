use std::sync::Arc;

use newengine_entity_api::{ENGINE_ENTITY_SERVICE_ID, ENTITY_BACKEND_CAPABILITY_ID};
use newengine_service_kit::{register_engine_gateway_provider_service, EngineGatewayProviderDecl};

use crate::{router::entity_gateway_service, service::ENTITY_GATEWAY_OWNER};

pub fn register_entity_gateway_best_effort(scene: Arc<newengine_scene_runtime::SceneBridge>) {
    if newengine_plugin_host::has_service(ENGINE_ENTITY_SERVICE_ID) {
        newengine_ulog_api::ulog::debug!(
            "engine.entity gateway registration skipped; service already available"
        );
        return;
    }

    let service = entity_gateway_service(scene);
    match register_engine_gateway_provider_service(EngineGatewayProviderDecl {
        gateway: ENGINE_ENTITY_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Entity,
        provider_service: ENGINE_ENTITY_SERVICE_ID,
        provider_route: "engine.entity.foundation",
        capability: ENTITY_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: ENTITY_GATEWAY_OWNER,
        service,
    }) {
        Ok(()) => newengine_ulog_api::ulog::info!(
            "engine.entity gateway registered source=engine-runtime service='{}' capability='{}' owner='{}'",
            ENGINE_ENTITY_SERVICE_ID,
            ENTITY_BACKEND_CAPABILITY_ID,
            ENTITY_GATEWAY_OWNER
        ),
        Err(error) => newengine_ulog_api::ulog::error!(
            "engine.entity gateway registration failed id='{}' err='{}'",
            ENGINE_ENTITY_SERVICE_ID,
            error
        ),
    }
}
