use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, JsonServiceRouter,
};
use newengine_world_environment_api::{
    EnvironmentFrameRequest, EnvironmentPreviewTimeRequest, EnvironmentRestoreRequest,
    EnvironmentSampleAtPositionRequest, EnvironmentServiceInfo, EnvironmentSnapshotRequest,
    ENGINE_WORLD_ENVIRONMENT_SERVICE_ID, WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID,
    WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1, WORLD_ENVIRONMENT_SERVICE_METHOD_INFO,
    WORLD_ENVIRONMENT_SERVICE_METHOD_INVOKE, WORLD_ENVIRONMENT_SERVICE_METHOD_PREVIEW_TIME_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_RESTORE_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_SAMPLE_AT_POSITION_JSON_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_SHUTDOWN_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_SNAPSHOT_JSON_V1,
};

use crate::{constants::WORLD_ENVIRONMENT_GATEWAY_OWNER, provider_state::EnvironmentProviderState};

pub(crate) fn environment_gateway_service(
    service_id: &'static str,
    provider: &'static str,
    provider_route: &'static str,
    degraded: bool,
) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let info = if degraded {
        EnvironmentServiceInfo::null_provider(provider)
    } else {
        EnvironmentServiceInfo::default_provider(provider)
    };
    let description = engine_gateway_provider_service_description(
        service_id,
        WORLD_ENVIRONMENT_GATEWAY_OWNER,
        WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .protocol(info.protocol.clone())
    .features(info.features.clone())
    .gateway(ENGINE_WORLD_ENVIRONMENT_SERVICE_ID)
    .notes("Environment is world meaning; renderer consumes resolved packets only.");

    JsonServiceRouter::with_state(
        service_id,
        EnvironmentProviderState::new(provider, provider_route, degraded),
    )
    .describe_json(&description)
    .get_json(WORLD_ENVIRONMENT_SERVICE_METHOD_INFO, |state| state.info())
    .blob(WORLD_ENVIRONMENT_SERVICE_METHOD_INVOKE, |state, payload| {
        state.invoke_json(payload)
    })
    .post_json(
        WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1,
        |state, request: EnvironmentFrameRequest| state.frame_json_v1(request),
    )
    .post_json(
        WORLD_ENVIRONMENT_SERVICE_METHOD_SAMPLE_AT_POSITION_JSON_V1,
        |state, request: EnvironmentSampleAtPositionRequest| {
            state.sample_at_position_json_v1(request)
        },
    )
    .post_json(
        WORLD_ENVIRONMENT_SERVICE_METHOD_SNAPSHOT_JSON_V1,
        |state, request: EnvironmentSnapshotRequest| state.snapshot_json_v1(request),
    )
    .post_json(
        WORLD_ENVIRONMENT_SERVICE_METHOD_RESTORE_JSON_V1,
        |state, request: EnvironmentRestoreRequest| state.restore_json_v1(request),
    )
    .post_json(
        WORLD_ENVIRONMENT_SERVICE_METHOD_PREVIEW_TIME_JSON_V1,
        |state, request: EnvironmentPreviewTimeRequest| state.preview_time_json_v1(request),
    )
    .blob(
        WORLD_ENVIRONMENT_SERVICE_METHOD_SHUTDOWN_V1,
        |_state, _payload| ok_empty_blob(),
    )
    .into_service_v1()
}
