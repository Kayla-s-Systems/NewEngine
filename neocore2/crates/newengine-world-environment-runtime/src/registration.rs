use newengine_service_kit::{
    register_engine_gateway_provider_service_dynamic_best_effort,
    register_null_engine_gateway_provider_service_dynamic_best_effort,
    EngineGatewayProviderDeclDynamic, NullEngineGatewayProviderDeclDynamic,
};
use newengine_world_environment_api::{
    ENGINE_WORLD_ENVIRONMENT_SERVICE_ID, WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID,
    WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID, WORLD_ENVIRONMENT_NULL_SERVICE_ID,
    WORLD_ENVIRONMENT_REQUIRED_METHODS_V1,
};

use crate::{
    constants::{
        WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE, WORLD_ENVIRONMENT_GATEWAY_OWNER,
        WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE,
    },
    router::environment_gateway_service,
};

/// Registers visible default and null provider routes for `engine.world.environment`.
pub fn register_world_environment_gateway_best_effort() {
    if !newengine_plugin_host::has_service(WORLD_ENVIRONMENT_NULL_SERVICE_ID) {
        let null_service = environment_gateway_service(
            WORLD_ENVIRONMENT_NULL_SERVICE_ID,
            "environment.null",
            WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE,
            true,
        );
        register_null_engine_gateway_provider_service_dynamic_best_effort(
            NullEngineGatewayProviderDeclDynamic {
                gateway: ENGINE_WORLD_ENVIRONMENT_SERVICE_ID,
                service_kind: "world.environment",
                provider_service: WORLD_ENVIRONMENT_NULL_SERVICE_ID,
                provider_route: WORLD_ENVIRONMENT_NULL_PROVIDER_ROUTE,
                capability: WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID,
                owner: WORLD_ENVIRONMENT_GATEWAY_OWNER,
                service: null_service,
            },
        );
    }

    if !newengine_plugin_host::has_service(WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID) {
        let default_service = environment_gateway_service(
            WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID,
            "environment.default",
            WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
            false,
        );
        register_engine_gateway_provider_service_dynamic_best_effort(
            EngineGatewayProviderDeclDynamic {
                gateway: ENGINE_WORLD_ENVIRONMENT_SERVICE_ID,
                service_kind: "world.environment",
                provider_service: WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID,
                provider_route: WORLD_ENVIRONMENT_DEFAULT_PROVIDER_ROUTE,
                capability: WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID,
                priority: 0,
                owner: WORLD_ENVIRONMENT_GATEWAY_OWNER,
                service: default_service,
            },
        );
    }

    newengine_ulog_api::ulog::info!(
        "engine.world.environment gateway baseline routes ready methods={} default_service='{}' null_service='{}'",
        WORLD_ENVIRONMENT_REQUIRED_METHODS_V1.len(),
        WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID,
        WORLD_ENVIRONMENT_NULL_SERVICE_ID
    );
}
