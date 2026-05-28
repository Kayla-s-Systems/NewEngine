#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable service contract for the `engine.ecs` gateway.
//!
//! The gateway is intentionally **world-neutral**: consumers see summaries,
//! snapshots and command/result envelopes, not `newengine_ecs::World` or typed
//! component storages. In-process gameplay systems may still use typed ECS
//! internals, but service/runtime boundaries should call `engine.ecs`.

use serde::{Deserialize, Serialize};

/// Engine-facing ECS service gateway id. Consumers call this facade; the host
/// resolves it to the active provider by descriptor metadata / engine-runtime facts.
pub const ENGINE_ECS_SERVICE_ID: &str = "engine.ecs";

/// Default/first-party provider service id for ECS backends.
pub const ECS_SERVICE_ID: &str = "ecs.api";
pub const ECS_BACKEND_CAPABILITY_ID: &str = "ecs.backend";

pub const ECS_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const ECS_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const ECS_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
pub const ECS_SERVICE_METHOD_SUMMARY_JSON_V1: &str = "summary_json_v1";
pub const ECS_SERVICE_METHOD_SNAPSHOT_JSON_V1: &str = "snapshot_json_v1";
pub const ECS_SERVICE_METHOD_COMMAND_JSON_V1: &str = "command_json_v1";

pub const ECS_REQUIRED_METHODS_V1: &[&str] = &[
    ECS_SERVICE_METHOD_INFO,
    ECS_SERVICE_METHOD_INVOKE,
    ECS_SERVICE_METHOD_SHUTDOWN_V1,
    ECS_SERVICE_METHOD_SUMMARY_JSON_V1,
    ECS_SERVICE_METHOD_SNAPSHOT_JSON_V1,
    ECS_SERVICE_METHOD_COMMAND_JSON_V1,
];

/// Generic backend-family declaration for ECS providers.
pub const ECS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "ecs",
        ENGINE_ECS_SERVICE_ID,
        ECS_SERVICE_ID,
        ECS_BACKEND_CAPABILITY_ID,
    );

/// Startup validation contract for the engine-facing ECS gateway.
pub const ECS_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_ECS_SERVICE_ID,
        "newengine.ecs-api >= 0.1.x",
        ECS_REQUIRED_METHODS_V1,
    );

/// Missing `engine.ecs` degrades by default; strict profiles can require it.
pub const ECS_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        ECS_RUNTIME_CONTRACT_SPEC,
        Some(ECS_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_ECS_BACKEND"),
    );

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcsServiceInfo {
    pub protocol: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub methods: Vec<String>,
}

impl Default for EcsServiceInfo {
    #[inline]
    fn default() -> Self {
        Self {
            protocol: "newengine.ecs-api/v1".to_owned(),
            features: vec![
                "gateway-summary".to_owned(),
                "entity-snapshot".to_owned(),
                "command-envelope".to_owned(),
                "semantic-component-packets".to_owned(),
            ],
            methods: ECS_REQUIRED_METHODS_V1.iter().map(|it| (*it).to_owned()).collect(),
        }
    }
}

/// Stable, provider-neutral summary of the active ECS world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EcsWorldSummary {
    #[serde(default)]
    pub tick: u64,
    #[serde(default)]
    pub entity_count: u64,
    #[serde(default)]
    pub storage_count: u64,
    #[serde(default)]
    pub resource_count: u64,
    #[serde(default)]
    pub entities_changed_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcsSnapshotRequest {
    #[serde(default = "default_true")]
    pub include_entities: bool,
    #[serde(default = "default_entity_limit")]
    pub entity_limit: usize,
}

impl Default for EcsSnapshotRequest {
    #[inline]
    fn default() -> Self {
        Self { include_entities: true, entity_limit: default_entity_limit() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcsEntitySnapshot {
    /// Stable engine-wide entity representation; opaque to service consumers.
    pub stable_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EcsWorldSnapshot {
    pub summary: EcsWorldSummary,
    #[serde(default)]
    pub entities: Vec<EcsEntitySnapshot>,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EcsCommandRequest {
    #[serde(default)]
    pub commands: Vec<EcsCommand>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EcsCommand {
    /// Sets the world tick. The implementation must clamp to its valid range.
    SetTick { tick: u64 },
    /// Advances the world tick and returns the new tick in the command result.
    AdvanceTick,
    /// Creates an empty entity and returns its opaque stable id.
    SpawnEmpty,
    /// Attaches or replaces a provider-neutral semantic component packet.
    ///
    /// The packet is intentionally JSON and component-type tagged. Typed hot-path
    /// runtimes may cache it in native storages, but service boundaries must not
    /// expose concrete `World` component storage.
    SetComponentJson {
        entity_id: u64,
        component_type: String,
        #[serde(default)]
        payload: serde_json::Value,
    },
    /// Removes a provider-neutral semantic component packet from an entity.
    RemoveComponentJson {
        entity_id: u64,
        component_type: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcsCommandResult {
    pub index: usize,
    pub ok: bool,
    #[serde(default)]
    pub entity_id: Option<u64>,
    #[serde(default)]
    pub tick: u64,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcsCommandResponse {
    pub ok: bool,
    pub summary: EcsWorldSummary,
    #[serde(default)]
    pub results: Vec<EcsCommandResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcsInvokeRequest {
    /// One of `summary_json_v1`, `snapshot_json_v1`, `command_json_v1`.
    pub method: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[inline]
fn default_true() -> bool { true }
#[inline]
fn default_entity_limit() -> usize { 4096 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecs_service_ids_are_gateway_first() {
        assert_eq!(ENGINE_ECS_SERVICE_ID, "engine.ecs");
        assert_eq!(ECS_BACKEND_SERVICE_SPEC.engine_gateway_id, ENGINE_ECS_SERVICE_ID);
        assert_eq!(ECS_BACKEND_SERVICE_SPEC.provider_service_id, ECS_SERVICE_ID);
        assert_eq!(ECS_BACKEND_SERVICE_SPEC.backend_capability_id, ECS_BACKEND_CAPABILITY_ID);
    }

    #[test]
    fn minimal_snapshot_request_decodes_with_defaults() {
        let req: EcsSnapshotRequest = serde_json::from_str("{}").expect("defaults decode");
        assert!(req.include_entities);
        assert!(req.entity_limit > 0);
    }
}
