#![forbid(unsafe_op_in_unsafe_fn)]

use serde::{Deserialize, Serialize};

pub const ENGINE_TIME_SERVICE_ID: &str = "engine.time";
pub const TIME_SERVICE_ID: &str = "time.api";
pub const TIME_BACKEND_CAPABILITY_ID: &str = "time.backend";
pub const TIME_RUNTIME_CONTRACT: &str = "newengine.time.runtime.v1";

pub mod time_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const SNAPSHOT_V1: &str = "time.snapshot_v1";
    pub const BEGIN_FRAME_V1: &str = "time.begin_frame_v1";
    pub const ADVANCE_FIXED_V1: &str = "time.advance_fixed_v1";
    pub const SET_SCALE_V1: &str = "time.set_scale_v1";
    pub const SET_PAUSE_V1: &str = "time.set_pause_v1";
    pub const SET_GAME_CLOCK_V1: &str = "time.set_game_clock_v1";
    pub const SCHEDULE_EVENT_V1: &str = "time.schedule_event_v1";
    pub const CANCEL_EVENT_V1: &str = "time.cancel_event_v1";
    pub const DUE_EVENTS_V1: &str = "time.due_events_v1";
    pub const DESCRIBE_CLOCK_V1: &str = "time.describe_clock_v1";
}

pub const TIME_SERVICE_METHODS: &[&str] = &[
    time_method::INFO_JSON,
    time_method::INVOKE_JSON,
    time_method::SHUTDOWN_V1,
    time_method::SNAPSHOT_V1,
    time_method::BEGIN_FRAME_V1,
    time_method::ADVANCE_FIXED_V1,
    time_method::SET_SCALE_V1,
    time_method::SET_PAUSE_V1,
    time_method::SET_GAME_CLOCK_V1,
    time_method::SCHEDULE_EVENT_V1,
    time_method::CANCEL_EVENT_V1,
    time_method::DUE_EVENTS_V1,
    time_method::DESCRIBE_CLOCK_V1,
];

pub const TIME_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "time",
        ENGINE_TIME_SERVICE_ID,
        TIME_SERVICE_ID,
        TIME_BACKEND_CAPABILITY_ID,
    );

pub const TIME_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_TIME_SERVICE_ID,
        TIME_RUNTIME_CONTRACT,
        TIME_SERVICE_METHODS,
    );

pub const TIME_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        TIME_RUNTIME_CONTRACT_SPEC,
        Some(TIME_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_TIME_BACKEND"),
    );

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeSnapshotV1 {
    pub schema: String,
    pub provider: String,
    pub frame_index: u64,
    pub real: TimeRealClockV1,
    pub simulation: TimeSimulationClockV1,
    pub game: TimeGameClockV1,
    pub replay: TimeReplayClockV1,
}

impl Default for TimeSnapshotV1 {
    fn default() -> Self {
        Self {
            schema: TIME_RUNTIME_CONTRACT.to_owned(),
            provider: "EngineOwnedTimeProvider".to_owned(),
            frame_index: 0,
            real: TimeRealClockV1::default(),
            simulation: TimeSimulationClockV1::default(),
            game: TimeGameClockV1::default(),
            replay: TimeReplayClockV1::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeRealClockV1 {
    pub monotonic_ns: u64,
    pub delta_ns: u64,
    pub clamped_delta_ns: u64,
}

impl Default for TimeRealClockV1 {
    fn default() -> Self {
        Self { monotonic_ns: 0, delta_ns: 0, clamped_delta_ns: 0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeSimulationClockV1 {
    pub tick: u64,
    pub fixed_delta_ns: u64,
    pub accumulator_ns: u64,
    pub ticks_to_run: u32,
    pub paused: bool,
    pub scale: f64,
}

impl Default for TimeSimulationClockV1 {
    fn default() -> Self {
        Self {
            tick: 0,
            fixed_delta_ns: 16_666_667,
            accumulator_ns: 0,
            ticks_to_run: 0,
            paused: false,
            scale: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeGameClockV1 {
    pub day_index: u64,
    pub seconds_of_day: f64,
    pub normalized_day: f64,
    pub seconds_per_game_day: f64,
    pub time_scale: f64,
}

impl Default for TimeGameClockV1 {
    fn default() -> Self {
        Self {
            day_index: 0,
            seconds_of_day: 0.0,
            normalized_day: 0.0,
            seconds_per_game_day: 86_400.0,
            time_scale: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeReplayClockV1 {
    pub deterministic: bool,
    pub seed: u64,
    pub replay_frame: u64,
}

impl Default for TimeReplayClockV1 {
    fn default() -> Self {
        Self { deterministic: false, seed: 0, replay_frame: 0 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeBeginFrameRequestV1 {
    pub frame_index: u64,
    pub fixed_delta_ns: u64,
}

impl Default for TimeBeginFrameRequestV1 {
    fn default() -> Self {
        Self { frame_index: 0, fixed_delta_ns: 16_666_667 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeScaleRequestV1 {
    pub scale: f64,
}

impl Default for TimeScaleRequestV1 {
    fn default() -> Self { Self { scale: 1.0 } }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimePauseRequestV1 {
    pub paused: bool,
}

impl Default for TimePauseRequestV1 {
    fn default() -> Self { Self { paused: false } }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeGameClockSetRequestV1 {
    pub day_index: u64,
    pub seconds_of_day: f64,
    pub seconds_per_game_day: f64,
    pub time_scale: f64,
}

impl Default for TimeGameClockSetRequestV1 {
    fn default() -> Self { Self { day_index: 0, seconds_of_day: 0.0, seconds_per_game_day: 86_400.0, time_scale: 1.0 } }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeScheduledEventV1 {
    pub id: String,
    pub due_simulation_tick: Option<u64>,
    pub due_monotonic_ns: Option<u64>,
    pub payload_json: serde_json::Value,
}

impl Default for TimeScheduledEventV1 {
    fn default() -> Self { Self { id: String::new(), due_simulation_tick: None, due_monotonic_ns: None, payload_json: serde_json::Value::Null } }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeCancelEventRequestV1 {
    pub id: String,
}

impl Default for TimeCancelEventRequestV1 {
    fn default() -> Self { Self { id: String::new() } }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeDueEventsV1 {
    pub events: Vec<TimeScheduledEventV1>,
}

impl Default for TimeDueEventsV1 {
    fn default() -> Self { Self { events: Vec::new() } }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeServiceInfoV1 {
    pub service_id: String,
    pub gateway: String,
    pub provider: String,
    pub contract: String,
    pub methods: Vec<String>,
    pub deterministic: bool,
}

impl Default for TimeServiceInfoV1 {
    fn default() -> Self {
        Self {
            service_id: TIME_SERVICE_ID.to_owned(),
            gateway: ENGINE_TIME_SERVICE_ID.to_owned(),
            provider: "EngineOwnedTimeProvider".to_owned(),
            contract: TIME_RUNTIME_CONTRACT.to_owned(),
            methods: TIME_SERVICE_METHODS.iter().map(|m| (*m).to_owned()).collect(),
            deterministic: false,
        }
    }
}
