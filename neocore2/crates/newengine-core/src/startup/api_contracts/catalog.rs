#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_service_api::{RuntimeServiceContractSpec, RuntimeServiceRequirementSpec};

const INPUT_REQUIRED_METHODS: &[&str] = &[
    "state_json",
    "text_take_json",
    "ime_commit_take_json",
];

const INPUT_RUNTIME_CONTRACT_SPEC: RuntimeServiceContractSpec = RuntimeServiceContractSpec::new(
    "engine.input",
    "newengine.input service >= 0.3.x",
    INPUT_REQUIRED_METHODS,
);

const INPUT_RUNTIME_REQUIREMENT_SPEC: RuntimeServiceRequirementSpec =
    RuntimeServiceRequirementSpec::new(
        INPUT_RUNTIME_CONTRACT_SPEC,
        Some("input.backend"),
        Some("NEWENGINE_REQUIRE_INPUT_BACKEND"),
    );

const LOG_REQUIRED_METHODS: &[&str] = &[
    "write_json",
    "flush",
];

const LOG_RUNTIME_CONTRACT_SPEC: RuntimeServiceContractSpec = RuntimeServiceContractSpec::new(
    crate::plugin_forward_logger::ENGINE_LOG_SERVICE_ID,
    "newengine.log-sink/v1",
    LOG_REQUIRED_METHODS,
);

const LOG_RUNTIME_REQUIREMENT_SPEC: RuntimeServiceRequirementSpec =
    RuntimeServiceRequirementSpec::new(
        LOG_RUNTIME_CONTRACT_SPEC,
        Some("log.backend"),
        Some("NEWENGINE_REQUIRE_LOG_BACKEND"),
    );

const PLATFORM_REQUIRED_METHODS: &[&str] = &[
    newengine_platform_api::PLATFORM_WINDOW_SERVICE_METHOD_SNAPSHOT_JSON_V1,
];

const PLATFORM_RUNTIME_CONTRACT_SPEC: RuntimeServiceContractSpec = RuntimeServiceContractSpec::new(
    newengine_platform_api::PLATFORM_WINDOW_SERVICE_ID,
    "newengine.platform-api >= 0.1.x",
    PLATFORM_REQUIRED_METHODS,
);

const PLATFORM_RUNTIME_REQUIREMENT_SPEC: RuntimeServiceRequirementSpec =
    RuntimeServiceRequirementSpec::new(
        PLATFORM_RUNTIME_CONTRACT_SPEC,
        None,
        Some("NEWENGINE_REQUIRE_PLATFORM_WINDOW_SERVICE"),
    );

/// Declarative startup validation catalog.
///
/// Adding a new accepted engine service family should add a data spec here (or,
/// preferably, re-export one from the corresponding `*-api` crate). The
/// validator does not contain per-domain `if/else` dispatch.
pub(crate) const RUNTIME_SERVICE_REQUIREMENTS: &[RuntimeServiceRequirementSpec] = &[
    newengine_assets_api::ASSET_RUNTIME_REQUIREMENT_SPEC,
    newengine_render_api::RENDER_RUNTIME_REQUIREMENT_SPEC,
    newengine_camera_api::CAMERA_RUNTIME_REQUIREMENT_SPEC,
    newengine_ui_api::UI_RUNTIME_REQUIREMENT_SPEC,
    newengine_scene_io::SCENE_RUNTIME_REQUIREMENT_SPEC,
    INPUT_RUNTIME_REQUIREMENT_SPEC,
    newengine_physics_api::PHYSICS_RUNTIME_REQUIREMENT_SPEC,
    LOG_RUNTIME_REQUIREMENT_SPEC,
    PLATFORM_RUNTIME_REQUIREMENT_SPEC,
];

/// Data-only diagnostics label for startup API table.
pub(crate) fn runtime_service_user(service_id: &str) -> &'static str {
    const USERS: &[(&str, &str)] = &[
        (newengine_assets_api::ENGINE_ASSET_SERVICE_ID, "asset clients / VFS / material texture load"),
        (newengine_render_api::ENGINE_RENDER_SERVICE_ID, "runtime-host render adapter / RuntimeRenderController"),
        (newengine_camera_api::ENGINE_CAMERA_SERVICE_ID, "CameraGatewayBridge / render view extraction"),
        (newengine_ui_api::ENGINE_UI_SERVICE_ID, "platform UI bridge / overlays / HUD"),
        (newengine_scene_io::ENGINE_SCENE_SERVICE_ID, "SceneBridge / world streaming / scene asset load-save"),
        ("engine.input", "platform_input::poll_input_frame / UI input projection"),
        (newengine_physics_api::ENGINE_PHYSICS_SERVICE_ID, "PhysicsSyncModule / gameplay ECS sync"),
        (crate::plugin_forward_logger::ENGINE_LOG_SERVICE_ID, "plugin_forward_logger / host log backend"),
        (newengine_platform_api::PLATFORM_WINDOW_SERVICE_ID, "platform runtime / native window surface"),
    ];

    USERS
        .iter()
        .find_map(|(id, user)| (*id == service_id).then_some(*user))
        .unwrap_or("<unclassified runtime consumer>")
}

pub(crate) fn runtime_service_requirement_duplicates() -> Vec<&'static str> {
    let mut ids = RUNTIME_SERVICE_REQUIREMENTS
        .iter()
        .map(|requirement| requirement.contract.service_id)
        .collect::<Vec<_>>();
    ids.sort_unstable();

    let mut duplicates = Vec::new();
    for pair in ids.windows(2) {
        if pair[0] == pair[1] && !duplicates.iter().any(|id| *id == pair[0]) {
            duplicates.push(pair[0]);
        }
    }
    duplicates
}
