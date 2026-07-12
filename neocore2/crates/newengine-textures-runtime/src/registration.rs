use newengine_assets::AssetServiceClient;
use newengine_assets_api::{
    ENGINE_ASSETS_TEXTURES_SERVICE_ID, TEXTURES_BACKEND_CAPABILITY_ID, TEXTURES_SERVICE_ID,
};
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl,
};

use crate::{
    router::textures_gateway_service,
    service::{TEXTURES_GATEWAY_OWNER, TEXTURES_PROVIDER_ROUTE},
};

pub fn register_textures_gateway_best_effort(client: AssetServiceClient) -> bool {
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
        service_kind: EngineServiceKind::Textures,
        provider_service: TEXTURES_SERVICE_ID,
        provider_route: TEXTURES_PROVIDER_ROUTE,
        capability: TEXTURES_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: TEXTURES_GATEWAY_OWNER,
        service: textures_gateway_service(client),
    })
}
