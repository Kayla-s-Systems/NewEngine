use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeFixedStepRequestV1 {
    pub fixed_delta_ns: u64,
    pub max_fixed_ticks_per_frame: u32,
    pub ai_decision_tick_interval: u32,
    pub ai_tick_budget_ns: u64,
}

impl Default for TimeFixedStepRequestV1 {
    fn default() -> Self {
        Self {
            fixed_delta_ns: 16_666_667,
            max_fixed_ticks_per_frame: 4,
            ai_decision_tick_interval: 4,
            ai_tick_budget_ns: 1_000_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TimeReplayClockSetRequestV1 {
    pub deterministic: bool,
    pub seed: u64,
    pub replay_frame: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeBeginFrameRequestV1 {
    pub frame_index: u64,
    pub fixed_delta_ns: u64,
}

impl Default for TimeBeginFrameRequestV1 {
    fn default() -> Self {
        Self {
            frame_index: 0,
            fixed_delta_ns: 16_666_667,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeScaleRequestV1 {
    pub scale: f64,
}

impl Default for TimeScaleRequestV1 {
    fn default() -> Self {
        Self { scale: 1.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TimePauseRequestV1 {
    pub paused: bool,
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
    fn default() -> Self {
        Self {
            day_index: 0,
            seconds_of_day: 0.0,
            seconds_per_game_day: 86_400.0,
            time_scale: 1.0,
        }
    }
}
