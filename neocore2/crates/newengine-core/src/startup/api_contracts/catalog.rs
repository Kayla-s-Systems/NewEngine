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

#[derive(Clone, Copy)]
pub(crate) struct RuntimeServiceCatalogEntry {
    pub(crate) requirement: RuntimeServiceRequirementSpec,
    pub(crate) used_by: &'static str,
}

impl RuntimeServiceCatalogEntry {
    #[inline]
    pub(crate) const fn new(
        requirement: RuntimeServiceRequirementSpec,
        used_by: &'static str,
    ) -> Self {
        Self { requirement, used_by }
    }
}

/// Declarative startup validation catalog.
///
/// Adding a new accepted engine service family should add one data row here
/// (or, preferably, re-export the requirement spec from the corresponding
/// `*-api` crate). The validator does not contain per-domain `if/else` dispatch
/// or a second side table for diagnostics ownership.
pub(crate) const RUNTIME_SERVICE_CATALOG: &[RuntimeServiceCatalogEntry] = &[
    RuntimeServiceCatalogEntry::new(
        newengine_assets_api::ASSET_RUNTIME_REQUIREMENT_SPEC,
        "asset clients / VFS / material texture load",
    ),
    RuntimeServiceCatalogEntry::new(
        newengine_render_api::RENDER_RUNTIME_REQUIREMENT_SPEC,
        "runtime-host render adapter / RuntimeRenderController",
    ),
    RuntimeServiceCatalogEntry::new(
        newengine_camera_api::CAMERA_RUNTIME_REQUIREMENT_SPEC,
        "CameraGatewayBridge / render view extraction",
    ),
    RuntimeServiceCatalogEntry::new(
        newengine_ui_api::UI_RUNTIME_REQUIREMENT_SPEC,
        "platform UI bridge / overlays / HUD",
    ),
    RuntimeServiceCatalogEntry::new(
        newengine_scene_io::SCENE_RUNTIME_REQUIREMENT_SPEC,
        "SceneBridge / world streaming / scene asset load-save",
    ),
    RuntimeServiceCatalogEntry::new(
        newengine_ecs_api::ECS_RUNTIME_REQUIREMENT_SPEC,
        "ECS gateway / world summary-snapshot-command service",
    ),
    RuntimeServiceCatalogEntry::new(
        newengine_entity_api::ENTITY_RUNTIME_REQUIREMENT_SPEC,
        "Entity gateway / service-safe identity and lifecycle commands",
    ),
    RuntimeServiceCatalogEntry::new(
        INPUT_RUNTIME_REQUIREMENT_SPEC,
        "platform_input::poll_input_frame / UI input projection",
    ),
    RuntimeServiceCatalogEntry::new(
        newengine_physics_api::PHYSICS_RUNTIME_REQUIREMENT_SPEC,
        "PhysicsSyncModule / gameplay ECS sync",
    ),
    RuntimeServiceCatalogEntry::new(
        LOG_RUNTIME_REQUIREMENT_SPEC,
        "plugin_forward_logger / host log backend",
    ),
    RuntimeServiceCatalogEntry::new(
        newengine_platform_api::PLATFORM_RUNTIME_REQUIREMENT_SPEC,
        "platform runtime / native window surface",
    ),
];

pub(crate) fn runtime_service_user(service_id: &str) -> &'static str {
    RUNTIME_SERVICE_CATALOG
        .iter()
        .find_map(|entry| {
            (entry.requirement.contract.service_id == service_id).then_some(entry.used_by)
        })
        .unwrap_or("<unclassified runtime consumer>")
}

pub(crate) fn runtime_service_requirement_duplicates() -> Vec<&'static str> {
    let mut ids = RUNTIME_SERVICE_CATALOG
        .iter()
        .map(|entry| entry.requirement.contract.service_id)
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
