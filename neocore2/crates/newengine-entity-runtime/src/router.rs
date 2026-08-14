use std::sync::Arc;

use newengine_entity_api::{
    EntityServiceInfo, ENGINE_ENTITY_SERVICE_ID, ENTITY_BACKEND_CAPABILITY_ID,
    ENTITY_SERVICE_METHOD_ARCHETYPES_JSON_V1, ENTITY_SERVICE_METHOD_DESPAWN_JSON_V1,
    ENTITY_SERVICE_METHOD_EXISTS_JSON_V1, ENTITY_SERVICE_METHOD_INVOKE,
    ENTITY_SERVICE_METHOD_LIST_JSON_V1,
    ENTITY_SERVICE_METHOD_REGISTER_ARCHETYPE_DEFINITION_JSON_V1, ENTITY_SERVICE_METHOD_SHUTDOWN_V1,
    ENTITY_SERVICE_METHOD_SPAWN_JSON_V1,
    ENTITY_SERVICE_METHOD_UNREGISTER_ARCHETYPE_DEFINITION_JSON_V1,
};
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, JsonServiceRouter,
};

use crate::service::{EngineEntityGatewayService, ENTITY_GATEWAY_OWNER};

pub fn entity_gateway_service(
    scene: Arc<newengine_scene_runtime::SceneBridge>,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let service = EngineEntityGatewayService::new(scene);
    let info = EntityServiceInfo::default();
    let description = engine_gateway_provider_service_description(
        ENGINE_ENTITY_SERVICE_ID,
        ENTITY_GATEWAY_OWNER,
        ENTITY_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .protocol(info.protocol.clone())
    .features(info.features.clone());

    let invoke_service = service.clone();
    let list_service = service.clone();
    let archetypes_service = service.clone();
    let exists_service = service.clone();
    let spawn_service = service.clone();
    let despawn_service = service.clone();
    let register_definition_service = service.clone();
    let unregister_definition_service = service;

    JsonServiceRouter::new(ENGINE_ENTITY_SERVICE_ID)
        .describe_json(&description)
        .info(EntityServiceInfo::default)
        .blob(ENTITY_SERVICE_METHOD_INVOKE, move |_unit, payload| {
            invoke_service.invoke_json(payload)
        })
        .blob(ENTITY_SERVICE_METHOD_LIST_JSON_V1, move |_unit, payload| {
            list_service.list_json_v1(payload)
        })
        .blob(
            ENTITY_SERVICE_METHOD_ARCHETYPES_JSON_V1,
            move |_unit, _payload| archetypes_service.archetypes_json_v1(),
        )
        .blob(
            ENTITY_SERVICE_METHOD_EXISTS_JSON_V1,
            move |_unit, payload| exists_service.exists_json_v1(payload),
        )
        .blob(
            ENTITY_SERVICE_METHOD_SPAWN_JSON_V1,
            move |_unit, payload| spawn_service.spawn_json_v1(payload),
        )
        .blob(
            ENTITY_SERVICE_METHOD_DESPAWN_JSON_V1,
            move |_unit, payload| despawn_service.despawn_json_v1(payload),
        )
        .blob(
            ENTITY_SERVICE_METHOD_REGISTER_ARCHETYPE_DEFINITION_JSON_V1,
            move |_unit, payload| {
                register_definition_service.register_archetype_definition_json_v1(payload)
            },
        )
        .blob(
            ENTITY_SERVICE_METHOD_UNREGISTER_ARCHETYPE_DEFINITION_JSON_V1,
            move |_unit, payload| {
                unregister_definition_service.unregister_archetype_definition_json_v1(payload)
            },
        )
        .blob(ENTITY_SERVICE_METHOD_SHUTDOWN_V1, |_unit, _payload| {
            ok_empty_blob()
        })
        .into_service_v1()
}
