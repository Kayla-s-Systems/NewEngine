use std::sync::Arc;

use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, JsonServiceRouter,
};
use newengine_world_api::{
    WorldServiceInfo, ENGINE_WORLD_SERVICE_ID, WORLD_BACKEND_CAPABILITY_ID, WORLD_SERVICE_ID,
    WORLD_SERVICE_METHOD_ACTIVE_CELLS_JSON_V1, WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1,
    WORLD_SERVICE_METHOD_BOOT_JSON_V1, WORLD_SERVICE_METHOD_INFO, WORLD_SERVICE_METHOD_INVOKE,
    WORLD_SERVICE_METHOD_LOAD_SNAPSHOT_JSON_V1, WORLD_SERVICE_METHOD_PARTITION_JSON_V1,
    WORLD_SERVICE_METHOD_RESTORE_SNAPSHOT_JSON_V1, WORLD_SERVICE_METHOD_SAVE_SNAPSHOT_JSON_V1,
    WORLD_SERVICE_METHOD_SHUTDOWN_V1, WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1,
    WORLD_SERVICE_METHOD_STATE_JSON_V1, WORLD_SERVICE_METHOD_STREAMING_CELLS_JSON_V1,
};

use crate::service::{EngineWorldGatewayService, WORLD_GATEWAY_OWNER};

pub fn world_gateway_service(
    scene: Arc<newengine_scene_runtime::SceneBridge>,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let service = EngineWorldGatewayService::new(scene);
    let info = WorldServiceInfo::default();
    let description = engine_gateway_provider_service_description(
        WORLD_SERVICE_ID,
        WORLD_GATEWAY_OWNER,
        WORLD_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .protocol(info.protocol.clone())
    .features(info.features.clone())
    .gateway(ENGINE_WORLD_SERVICE_ID)
    .notes("Scene = authored structure; World = living runtime world.");

    let info_service = service.clone();
    let invoke_service = service.clone();
    let boot_service = service.clone();
    let state_service = service.clone();
    let cells_service = service.clone();
    let partition_service = service.clone();
    let streaming_service = service.clone();
    let snapshot_service = service.clone();
    let restore_service = service.clone();
    let apply_service = service.clone();
    let save_snapshot_service = service.clone();
    let load_snapshot_service = service;

    JsonServiceRouter::new(WORLD_SERVICE_ID)
        .describe_json(&description)
        .get_json(WORLD_SERVICE_METHOD_INFO, move |_| info_service.info_json())
        .blob(WORLD_SERVICE_METHOD_INVOKE, move |_unit, payload| {
            invoke_service.invoke_json(payload)
        })
        .blob(WORLD_SERVICE_METHOD_BOOT_JSON_V1, move |_unit, payload| {
            boot_service.boot_json_v1(payload)
        })
        .blob(WORLD_SERVICE_METHOD_STATE_JSON_V1, move |_unit, payload| {
            state_service.state_json_v1(payload)
        })
        .blob(
            WORLD_SERVICE_METHOD_ACTIVE_CELLS_JSON_V1,
            move |_unit, payload| cells_service.active_cells_json_v1(payload),
        )
        .blob(
            WORLD_SERVICE_METHOD_STREAMING_CELLS_JSON_V1,
            move |_unit, payload| streaming_service.streaming_cells_json_v1(payload),
        )
        .blob(
            WORLD_SERVICE_METHOD_PARTITION_JSON_V1,
            move |_unit, _payload| partition_service.partition_json_v1(),
        )
        .blob(
            WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1,
            move |_unit, payload| snapshot_service.snapshot_json_v1(payload),
        )
        .blob(
            WORLD_SERVICE_METHOD_RESTORE_SNAPSHOT_JSON_V1,
            move |_unit, payload| restore_service.restore_snapshot_json_v1(payload),
        )
        .blob(
            WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1,
            move |_unit, payload| apply_service.apply_stage_json_v1(payload),
        )
        .blob(
            WORLD_SERVICE_METHOD_SAVE_SNAPSHOT_JSON_V1,
            move |_unit, payload| save_snapshot_service.save_snapshot_json_v1(payload),
        )
        .blob(
            WORLD_SERVICE_METHOD_LOAD_SNAPSHOT_JSON_V1,
            move |_unit, payload| load_snapshot_service.load_snapshot_json_v1(payload),
        )
        .blob(WORLD_SERVICE_METHOD_SHUTDOWN_V1, |_unit, _payload| {
            ok_empty_blob()
        })
        .into_service_v1()
}
