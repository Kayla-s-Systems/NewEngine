#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable DTO and service contract for the `engine.time` gateway.

mod clocks;
mod control;
mod events;
mod service;

pub use clocks::{
    TimeAiClockV1, TimeAiContextV1, TimeGameClockV1, TimeRealClockV1, TimeReplayClockV1,
    TimeSimulationClockV1, TimeSnapshotV1, TimeTimelineV1,
};
pub use control::{
    TimeBeginFrameRequestV1, TimeFixedStepRequestV1, TimeGameClockSetRequestV1, TimePauseRequestV1,
    TimeReplayClockSetRequestV1, TimeScaleRequestV1,
};
pub use events::{TimeCancelEventRequestV1, TimeDueEventsV1, TimeScheduledEventV1};
pub use service::{
    time_method, TimeServiceInfoV1, ENGINE_TIME_SERVICE_ID, TIME_BACKEND_CAPABILITY_ID,
    TIME_BACKEND_SERVICE_SPEC, TIME_RUNTIME_CONTRACT, TIME_RUNTIME_CONTRACT_SPEC,
    TIME_RUNTIME_REQUIREMENT_SPEC, TIME_SERVICE_ID, TIME_SERVICE_METHODS,
};
