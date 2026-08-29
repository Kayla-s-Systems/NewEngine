use serde::{Deserialize, Serialize};

use crate::{
    PhysicsBackendInfo, PhysicsCapabilityNegotiationRequest, PhysicsCapabilityNegotiationResponse,
    PhysicsFrameInput, PhysicsFrameOutput, PhysicsProblemDetails,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhysicsServiceRequest {
    Negotiate(PhysicsCapabilityNegotiationRequest),
    StepFrame(PhysicsFrameInput),
    DiagnosticsSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhysicsServiceResponse {
    Unit,
    Negotiation(PhysicsCapabilityNegotiationResponse),
    FrameOutput(PhysicsFrameOutput),
    BackendInfo(PhysicsBackendInfo),
    DiagnosticsSnapshot(PhysicsBackendInfo),
    Problem(PhysicsProblemDetails),
}
