use newengine_service_kit::{
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl,
};
use newengine_time_api::{ENGINE_TIME_SERVICE_ID, TIME_BACKEND_CAPABILITY_ID, TIME_SERVICE_ID};

use crate::{
    constants::{OWNER, PROVIDER_ROUTE},
    router::service,
};

pub fn register_time_gateway_best_effort() -> bool {
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_TIME_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Time,
        provider_service: TIME_SERVICE_ID,
        provider_route: PROVIDER_ROUTE,
        capability: TIME_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: OWNER,
        service: service(),
    })
}
