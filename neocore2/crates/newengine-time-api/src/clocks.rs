use serde::{Deserialize, Serialize};

use crate::TIME_RUNTIME_CONTRACT;

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
    pub ai: TimeAiClockV1,
}

impl Default for TimeSnapshotV1 {
    fn default() -> Self {
        Self {
            schema: TIME_RUNTIME_CONTRACT.to_owned(),
            provider: "AstrolabeTimeProvider".to_owned(),
            frame_index: 0,
            real: TimeRealClockV1::default(),
            simulation: TimeSimulationClockV1::default(),
            game: TimeGameClockV1::default(),
            replay: TimeReplayClockV1::default(),
            ai: TimeAiClockV1::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeTimelineV1 {
    pub schema: String,
    pub frame_index: u64,
    pub fixed_tick: u64,
    pub game_day_index: u64,
    pub game_seconds_of_day: f64,
    pub replay_frame: u64,
    pub paused: bool,
    pub scale: f64,
}

impl Default for TimeTimelineV1 {
    fn default() -> Self {
        Self {
            schema: "newengine.time.timeline.v1".to_owned(),
            frame_index: 0,
            fixed_tick: 0,
            game_day_index: 0,
            game_seconds_of_day: 0.0,
            replay_frame: 0,
            paused: false,
            scale: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TimeRealClockV1 {
    pub monotonic_ns: u64,
    pub delta_ns: u64,
    pub clamped_delta_ns: u64,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TimeReplayClockV1 {
    pub deterministic: bool,
    pub seed: u64,
    pub replay_frame: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeAiClockV1 {
    pub tick_budget_ns: u64,
    pub decision_tick_interval: u32,
    pub next_decision_tick: u64,
    pub time_of_day_phase: String,
    pub normalized_day: f64,
    pub deterministic_key: String,
}

impl Default for TimeAiClockV1 {
    fn default() -> Self {
        Self {
            tick_budget_ns: 1_000_000,
            decision_tick_interval: 4,
            next_decision_tick: 0,
            time_of_day_phase: "night".to_owned(),
            normalized_day: 0.0,
            deterministic_key: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeAiContextV1 {
    pub schema: String,
    pub frame_index: u64,
    pub simulation_tick: u64,
    pub fixed_delta_ns: u64,
    pub game_day_index: u64,
    pub game_seconds_of_day: f64,
    pub normalized_day: f64,
    pub time_of_day_phase: String,
    pub deterministic: bool,
    pub replay_seed: u64,
    pub replay_frame: u64,
    pub decision_tick_interval: u32,
    pub next_decision_tick: u64,
    pub tick_budget_ns: u64,
    pub deterministic_key: String,
}

impl Default for TimeAiContextV1 {
    fn default() -> Self {
        Self {
            schema: "newengine.time.ai_context.v1".to_owned(),
            frame_index: 0,
            simulation_tick: 0,
            fixed_delta_ns: 16_666_667,
            game_day_index: 0,
            game_seconds_of_day: 0.0,
            normalized_day: 0.0,
            time_of_day_phase: "night".to_owned(),
            deterministic: false,
            replay_seed: 0,
            replay_frame: 0,
            decision_tick_interval: 4,
            next_decision_tick: 0,
            tick_budget_ns: 1_000_000,
            deterministic_key: String::new(),
        }
    }
}
