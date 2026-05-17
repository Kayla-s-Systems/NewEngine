#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_service_api::{RuntimeServiceContractSpec, RuntimeServiceRequirementSpec};

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
    newengine_physics_api::PHYSICS_RUNTIME_REQUIREMENT_SPEC,
    PLATFORM_RUNTIME_REQUIREMENT_SPEC,
];
