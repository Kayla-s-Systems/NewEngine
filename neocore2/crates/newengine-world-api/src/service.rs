use serde::{Deserialize, Serialize};

pub const ENGINE_WORLD_SERVICE_ID: &str = "engine.world";
pub const WORLD_SERVICE_ID: &str = "world.api";
pub const WORLD_BACKEND_CAPABILITY_ID: &str = "world.backend";

pub const WORLD_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const WORLD_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const WORLD_SERVICE_METHOD_SHUTDOWN_V1: &str =
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const WORLD_SERVICE_METHOD_BOOT_JSON_V1: &str = "world.boot_json_v1";
pub const WORLD_SERVICE_METHOD_STATE_JSON_V1: &str = "world.state_json_v1";
pub const WORLD_SERVICE_METHOD_PARTITION_JSON_V1: &str = "world.partition_json_v1";
pub const WORLD_SERVICE_METHOD_ACTIVE_CELLS_JSON_V1: &str = "world.active_cells_json_v1";
pub const WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1: &str = "world.snapshot_json_v1";
pub const WORLD_SERVICE_METHOD_RESTORE_SNAPSHOT_JSON_V1: &str = "world.restore_snapshot_json_v1";
pub const WORLD_SERVICE_METHOD_STREAMING_CELLS_JSON_V1: &str = "world.streaming_cells_json_v1";
pub const WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1: &str = "world.apply_stage_json_v1";
pub const WORLD_SERVICE_METHOD_SAVE_SNAPSHOT_JSON_V1: &str = "world.save_snapshot_json_v1";
pub const WORLD_SERVICE_METHOD_LOAD_SNAPSHOT_JSON_V1: &str = "world.load_snapshot_json_v1";

pub const WORLD_REQUIRED_METHODS_V1: &[&str] = &[
    WORLD_SERVICE_METHOD_INFO,
    WORLD_SERVICE_METHOD_INVOKE,
    WORLD_SERVICE_METHOD_SHUTDOWN_V1,
    WORLD_SERVICE_METHOD_BOOT_JSON_V1,
    WORLD_SERVICE_METHOD_STATE_JSON_V1,
    WORLD_SERVICE_METHOD_PARTITION_JSON_V1,
    WORLD_SERVICE_METHOD_ACTIVE_CELLS_JSON_V1,
    WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1,
    WORLD_SERVICE_METHOD_RESTORE_SNAPSHOT_JSON_V1,
    WORLD_SERVICE_METHOD_STREAMING_CELLS_JSON_V1,
    WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1,
    WORLD_SERVICE_METHOD_SAVE_SNAPSHOT_JSON_V1,
    WORLD_SERVICE_METHOD_LOAD_SNAPSHOT_JSON_V1,
];

pub const WORLD_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "world",
        ENGINE_WORLD_SERVICE_ID,
        WORLD_SERVICE_ID,
        WORLD_BACKEND_CAPABILITY_ID,
    );

pub const WORLD_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_WORLD_SERVICE_ID,
        "newengine.world-api >= 0.1.x",
        WORLD_REQUIRED_METHODS_V1,
    );

/// Missing `engine.world` degrades by default; strict profiles can require it.
pub const WORLD_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        WORLD_RUNTIME_CONTRACT_SPEC,
        Some(WORLD_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_WORLD_BACKEND"),
    );

const WORLD_FEATURES_V1: &[&str] = &[
    "runtime-world-instance",
    "deterministic-boot-sequence",
    "world-partition",
    "active-cells",
    "world-state-snapshot",
    "streaming-cells",
    "runtime-apply-stage",
    "save-load-snapshots",
    "opaque-entity-handles",
    "scene-world-separation",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for WorldServiceInfo {
    #[inline]
    fn default() -> Self {
        Self {
            protocol: "newengine.world-api/v1".to_owned(),
            features: WORLD_FEATURES_V1
                .iter()
                .map(|feature| (*feature).to_owned())
                .collect(),
            methods: WORLD_REQUIRED_METHODS_V1
                .iter()
                .map(|method| (*method).to_owned())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_service_ids_are_gateway_first() {
        assert_eq!(ENGINE_WORLD_SERVICE_ID, "engine.world");
        assert_eq!(
            WORLD_BACKEND_SERVICE_SPEC.engine_gateway_id,
            ENGINE_WORLD_SERVICE_ID
        );
        assert_eq!(
            WORLD_BACKEND_SERVICE_SPEC.provider_service_id,
            WORLD_SERVICE_ID
        );
        assert_eq!(
            WORLD_BACKEND_SERVICE_SPEC.backend_capability_id,
            WORLD_BACKEND_CAPABILITY_ID
        );
    }

    #[test]
    fn service_info_advertises_required_methods() {
        let info = WorldServiceInfo::default();
        assert_eq!(info.methods.len(), WORLD_REQUIRED_METHODS_V1.len());
    }
}
