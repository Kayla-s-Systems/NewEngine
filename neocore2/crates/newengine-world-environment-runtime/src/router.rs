use abi_stable::std_types::RResult;
use newengine_console_api::COMMAND_DESCRIPTOR_CONTRACT_ID;
use newengine_plugin_api::Blob;

use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, JsonServiceRouter,
};
use newengine_world_environment_api::{
    EnvironmentFrameRequest, EnvironmentPreviewTimeRequest, EnvironmentRestoreRequest,
    EnvironmentSampleAtPositionRequest, EnvironmentServiceInfo, EnvironmentSnapshotRequest,
    ENGINE_WORLD_ENVIRONMENT_SERVICE_ID, WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID,
    WORLD_ENVIRONMENT_SERVICE_METHOD_FRAME_JSON_V1, WORLD_ENVIRONMENT_SERVICE_METHOD_INFO,
    WORLD_ENVIRONMENT_SERVICE_METHOD_INSPECT_CELL_TEXT_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_INSPECT_TEXT_V1, WORLD_ENVIRONMENT_SERVICE_METHOD_INVOKE,
    WORLD_ENVIRONMENT_SERVICE_METHOD_OBJECTS_TEXT_V1,
    WORLD_ENVIRONMENT_SERVICE_METHOD_PREVIEW_TIME_JSON_V1,
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
    let base_description = engine_gateway_provider_service_description(
        service_id,
        WORLD_ENVIRONMENT_GATEWAY_OWNER,
        WORLD_ENVIRONMENT_BACKEND_CAPABILITY_ID,
        info.methods.clone(),
    )
    .protocol(info.protocol.clone())
    .features(info.features.clone())
    .gateway(ENGINE_WORLD_ENVIRONMENT_SERVICE_ID)
    .notes("Environment is world meaning; renderer consumes resolved packets only.");
    let mut description = serde_json::to_value(base_description).unwrap_or_default();
    if let Some(object) = description.as_object_mut() {
        object.insert(
            "console".to_owned(),
            serde_json::json!({
                "contract": COMMAND_DESCRIPTOR_CONTRACT_ID,
                "commands": [
                    {
                        "name": "env.inspect",
                        "help": "Inspect the current causal atmospheric/weather state",
                        "usage": "env.inspect",
                        "kind": "service_call",
                        "service_id": ENGINE_WORLD_ENVIRONMENT_SERVICE_ID,
                        "method": WORLD_ENVIRONMENT_SERVICE_METHOD_INSPECT_TEXT_V1,
                        "payload": "empty",
                        "flags": { "developer": true, "read_only": true, "remote_allowed": true },
                        "owner": "world.environment"
                    },
                    {
                        "name": "env.cell",
                        "help": "Inspect one resident mesoscale atmospheric cell",
                        "usage": "env.cell <x> <z>",
                        "kind": "service_call",
                        "service_id": ENGINE_WORLD_ENVIRONMENT_SERVICE_ID,
                        "method": WORLD_ENVIRONMENT_SERVICE_METHOD_INSPECT_CELL_TEXT_V1,
                        "payload": "raw",
                        "args": [
                            { "name": "x", "value_type": "i32", "required": true, "description": "World atmosphere cell X" },
                            { "name": "z", "value_type": "i32", "required": true, "description": "World atmosphere cell Z" }
                        ],
                        "flags": { "developer": true, "read_only": true, "remote_allowed": true },
                        "owner": "world.environment"
                    },
                    {
                        "name": "env.objects",
                        "help": "List physical mesoscale weather objects and their owning cells",
                        "usage": "env.objects",
                        "kind": "service_call",
                        "service_id": ENGINE_WORLD_ENVIRONMENT_SERVICE_ID,
                        "method": WORLD_ENVIRONMENT_SERVICE_METHOD_OBJECTS_TEXT_V1,
                        "payload": "empty",
                        "flags": { "developer": true, "read_only": true, "remote_allowed": true },
                        "owner": "world.environment"
                    }
                ]
            }),
        );
    }

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
    .blob(
        WORLD_ENVIRONMENT_SERVICE_METHOD_INSPECT_TEXT_V1,
        |state, _payload| RResult::ROk(Blob::from(state.inspect_text_v1().into_bytes())),
    )
    .blob(
        WORLD_ENVIRONMENT_SERVICE_METHOD_INSPECT_CELL_TEXT_V1,
        |state, payload| match state.inspect_cell_text_v1(payload.as_slice()) {
            Ok(text) => RResult::ROk(Blob::from(text.into_bytes())),
            Err(error) => RResult::RErr(error.into()),
        },
    )
    .blob(
        WORLD_ENVIRONMENT_SERVICE_METHOD_OBJECTS_TEXT_V1,
        |state, _payload| RResult::ROk(Blob::from(state.objects_text_v1().into_bytes())),
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

#[cfg(test)]
mod console_descriptor_tests {
    use super::*;

    #[test]
    fn environment_description_advertises_discoverable_inspect_command() {
        let service = environment_gateway_service(
            newengine_world_environment_api::WORLD_ENVIRONMENT_DEFAULT_SERVICE_ID,
            "environment.default",
            "engine.world.environment.default",
            false,
        );
        let description = service.describe().to_string();
        let value: serde_json::Value =
            serde_json::from_str(&description).expect("description json");
        let commands = value["console"]["commands"]
            .as_array()
            .expect("console commands");
        let names = commands
            .iter()
            .filter_map(|command| command["name"].as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            names,
            std::collections::BTreeSet::from(["env.cell", "env.inspect", "env.objects"])
        );
        for command in commands {
            assert_eq!(
                command["service_id"],
                newengine_world_environment_api::ENGINE_WORLD_ENVIRONMENT_SERVICE_ID
            );
            assert_eq!(command["flags"]["read_only"], true);
        }
    }
}
