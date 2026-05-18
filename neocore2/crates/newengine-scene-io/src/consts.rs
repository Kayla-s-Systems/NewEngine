#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_service_api::{RuntimeServiceContractSpec, RuntimeServiceRequirementSpec};

/// Engine-owned facade id for the scene service.
///
/// `engine.scene` is the only consumer-facing scene gateway. Providers and
/// engine-owned implementations route through the same gateway registry; there
/// is no secondary legacy scene service id.
pub const ENGINE_SCENE_SERVICE_ID: &str = "engine.scene";

/// Capability id used by scene providers or engine-owned scene gateway sources.
pub const SCENE_BACKEND_CAPABILITY_ID: &str = "scene.backend";

/// Canonical method names for the scene gateway.
///
/// Method naming is contract-first and stable across versions.
pub mod method {
    /// Returns a JSON descriptor of supported scene formats.
    pub const FORMATS_JSON: &str = "scene.formats_json";

    /// Load a scene from a JSON payload stored at `path`.
    ///
    /// Request payload: json `{ path, replace, options }`.
    pub const LOAD_JSON_V1: &str = "scene.load_json_v1";

    /// Save the current scene into a JSON payload.
    ///
    /// Request payload: json `{ path, pretty, options }`.
    pub const SAVE_JSON_V1: &str = "scene.save_json_v1";
}

pub const SCENE_REQUIRED_METHODS: &[&str] = &[
    method::FORMATS_JSON,
    method::LOAD_JSON_V1,
    method::SAVE_JSON_V1,
];

pub const SCENE_RUNTIME_CONTRACT_SPEC: RuntimeServiceContractSpec = RuntimeServiceContractSpec::new(
    ENGINE_SCENE_SERVICE_ID,
    "newengine.scene gateway >= 0.1.x",
    SCENE_REQUIRED_METHODS,
);

pub const SCENE_RUNTIME_REQUIREMENT_SPEC: RuntimeServiceRequirementSpec = RuntimeServiceRequirementSpec::new(
    SCENE_RUNTIME_CONTRACT_SPEC,
    Some(SCENE_BACKEND_CAPABILITY_ID),
    Some("NEWENGINE_REQUIRE_SCENE_BACKEND"),
);
