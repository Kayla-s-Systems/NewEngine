use crate::{
    RenderBackendEvent, RenderDiagnosticsSnapshot, RenderDrawListKind, RenderGraphCompileReport,
    RenderGraphDesc, RenderGraphPassKind, RenderGraphSubmitReport, RenderGraphValidationReport,
    RenderWorkBudget, UploadPumpDesc, UploadPumpReport,
};
use serde::{Deserialize, Serialize};

use super::{
    RenderCapabilityNegotiationRequest, RenderCapabilityNegotiationResponse, RenderCommand,
    RenderCommandResponse, RenderFrameEnvelope, RenderProblemDetails,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderServiceRequest {
    Negotiate(RenderCapabilityNegotiationRequest),
    Command(RenderCommand),
    /// Executes a sequence of unit render commands in one provider call.
    ///
    /// This keeps the engine-facing API imperative for feature extractors while
    /// avoiding one service-boundary roundtrip per recorded draw command on the
    /// frame hot path. Commands that return ids/snapshots should still use
    /// `Command` so the caller can consume the typed response immediately.
    CommandBatch(Vec<RenderCommand>),
    CompileRenderGraph(RenderGraphDesc),
    ValidateRenderGraph(RenderGraphDesc),
    SetRenderPhase {
        phase: Option<RenderGraphPassKind>,
    },
    SetDrawListKind {
        kind: Option<RenderDrawListKind>,
    },
    DiscardRecordedCommands,
    SubmitRenderGraph(RenderGraphDesc),
    SubmitFrame(Box<RenderFrameEnvelope>),
    SetWorkBudget(RenderWorkBudget),
    PumpUploads(UploadPumpDesc),
    DrainBackendEvents,
    DiagnosticsSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderServiceResponse {
    Unit,
    Negotiation(RenderCapabilityNegotiationResponse),
    Command(RenderCommandResponse),
    CommandBatch(Vec<RenderCommandResponse>),
    GraphCompileReport(RenderGraphCompileReport),
    GraphValidationReport(RenderGraphValidationReport),
    GraphSubmitReport(RenderGraphSubmitReport),
    UploadPumpReport(UploadPumpReport),
    DiagnosticsSnapshot(Box<RenderDiagnosticsSnapshot>),
    BackendEvents(Vec<RenderBackendEvent>),
    Problem(RenderProblemDetails),
}
