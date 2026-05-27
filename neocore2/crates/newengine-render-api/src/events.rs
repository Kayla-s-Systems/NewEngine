use serde::{Deserialize, Serialize};

/// Host/event-bus topic for renderer backend lifecycle events.
///
/// The renderer owns native GPU queues/fences. Engine runtime consumers must
/// observe these events instead of guessing resource safety from elapsed frames.
pub const RENDER_BACKEND_EVENT_TOPIC_V1: &str = "engine.render.backend.event.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderBackendEventKind {
    FrameSubmitted,
    FrameCompleted,
    ResourceRetired,
    BackendDegraded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderBackendEvent {
    pub schema: String,
    pub backend: String,
    pub kind: RenderBackendEventKind,
    /// Engine frame id when the runtime supplied one. Backend-only callers may
    /// leave this as zero and use backend_frame_index for diagnostics only.
    pub frame_index: u64,
    pub backend_frame_index: u64,
    pub phase: String,
    pub detail: String,
}

impl RenderBackendEvent {
    #[inline]
    pub fn new(
        backend: impl Into<String>,
        kind: RenderBackendEventKind,
        frame_index: u64,
        backend_frame_index: u64,
        phase: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            schema: "newengine.render.backend_event.v1".to_owned(),
            backend: backend.into(),
            kind,
            frame_index,
            backend_frame_index,
            phase: phase.into(),
            detail: detail.into(),
        }
    }

    #[inline]
    pub fn frame_submitted(
        backend: impl Into<String>,
        frame_index: u64,
        backend_frame_index: u64,
    ) -> Self {
        Self::new(
            backend,
            RenderBackendEventKind::FrameSubmitted,
            frame_index,
            backend_frame_index,
            "end_frame.queue_submit",
            "Frame command buffer submitted to the renderer backend queue.",
        )
    }

    #[inline]
    pub fn frame_completed(
        backend: impl Into<String>,
        frame_index: u64,
        backend_frame_index: u64,
    ) -> Self {
        Self::new(
            backend,
            RenderBackendEventKind::FrameCompleted,
            frame_index,
            backend_frame_index,
            "begin_frame.wait_for_fence",
            "Renderer backend fence signaled; frame-owned resources may be retired.",
        )
    }
}
