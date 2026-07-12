#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable service contract for the `engine.world` gateway.
//!
//! `engine.scene` is authored structure while `engine.world` is the living
//! runtime world. ECS remains storage; native ECS entity ids are not exposed.

mod apply_stage;
mod client;
mod lifecycle;
mod partition;
mod service;
mod snapshot;
mod state;

pub use apply_stage::{WorldApplyStageCommand, WorldApplyStageRequest, WorldApplyStageResponse};
pub use client::WorldClient;
pub use lifecycle::{WorldBootRequest, WorldBootResponse, WorldInvokeRequest};
pub use partition::{
    WorldBootPhase, WorldCellCoord, WorldCellRecord, WorldCellResidency, WorldPartitionState,
    WorldStreamingCellDto, WorldStreamingCellsRequest, WorldStreamingCellsResponse,
    WorldStreamingPlanDto,
};
pub use service::{
    WorldServiceInfo, ENGINE_WORLD_SERVICE_ID, WORLD_BACKEND_CAPABILITY_ID,
    WORLD_BACKEND_SERVICE_SPEC, WORLD_REQUIRED_METHODS_V1, WORLD_RUNTIME_CONTRACT_SPEC,
    WORLD_RUNTIME_REQUIREMENT_SPEC, WORLD_SERVICE_ID, WORLD_SERVICE_METHOD_ACTIVE_CELLS_JSON_V1,
    WORLD_SERVICE_METHOD_APPLY_STAGE_JSON_V1, WORLD_SERVICE_METHOD_BOOT_JSON_V1,
    WORLD_SERVICE_METHOD_INFO, WORLD_SERVICE_METHOD_INVOKE,
    WORLD_SERVICE_METHOD_LOAD_SNAPSHOT_JSON_V1, WORLD_SERVICE_METHOD_PARTITION_JSON_V1,
    WORLD_SERVICE_METHOD_RESTORE_SNAPSHOT_JSON_V1, WORLD_SERVICE_METHOD_SAVE_SNAPSHOT_JSON_V1,
    WORLD_SERVICE_METHOD_SHUTDOWN_V1, WORLD_SERVICE_METHOD_SNAPSHOT_JSON_V1,
    WORLD_SERVICE_METHOD_STATE_JSON_V1, WORLD_SERVICE_METHOD_STREAMING_CELLS_JSON_V1,
};
pub use snapshot::{
    WorldLoadSnapshotRequest, WorldLoadSnapshotResponse, WorldRestoreSnapshotRequest,
    WorldRestoreSnapshotResponse, WorldSaveSnapshotRequest, WorldSaveSnapshotResponse,
    WorldSnapshotRequest, WorldSnapshotResponse,
};
pub use state::{
    WorldActiveCellsRequest, WorldActiveCellsResponse, WorldPartitionResponse, WorldRuntimeState,
    WorldStateRequest, WorldStateResponse,
};
