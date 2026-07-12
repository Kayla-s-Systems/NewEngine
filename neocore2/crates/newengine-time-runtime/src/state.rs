use std::{collections::BTreeMap, time::Instant};

use newengine_time_api::TimeScheduledEventV1;

use crate::constants::{
    DEFAULT_AI_DECISION_INTERVAL, DEFAULT_AI_TICK_BUDGET_NS, DEFAULT_FIXED_DELTA_NS,
    DEFAULT_MAX_FIXED_TICKS_PER_FRAME, SECONDS_PER_DAY,
};

#[derive(Debug)]
pub(crate) struct RuntimeHostedTimeState {
    pub(crate) start: Instant,
    pub(crate) last: Instant,
    pub(crate) frame_index: u64,
    pub(crate) last_raw_delta_ns: u64,
    pub(crate) last_clamped_delta_ns: u64,
    pub(crate) fixed_delta_ns: u64,
    pub(crate) max_fixed_ticks_per_frame: u32,
    pub(crate) accumulator_ns: u64,
    pub(crate) tick: u64,
    pub(crate) ticks_to_run: u32,
    pub(crate) paused: bool,
    pub(crate) scale: f64,
    pub(crate) day_index: u64,
    pub(crate) seconds_of_day: f64,
    pub(crate) seconds_per_game_day: f64,
    pub(crate) game_time_scale: f64,
    pub(crate) replay_deterministic: bool,
    pub(crate) replay_seed: u64,
    pub(crate) replay_frame: u64,
    pub(crate) ai_tick_budget_ns: u64,
    pub(crate) ai_decision_tick_interval: u32,
    pub(crate) scheduled_events: BTreeMap<String, TimeScheduledEventV1>,
}

impl Default for RuntimeHostedTimeState {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            start: now,
            last: now,
            frame_index: 0,
            last_raw_delta_ns: 0,
            last_clamped_delta_ns: 0,
            fixed_delta_ns: DEFAULT_FIXED_DELTA_NS,
            max_fixed_ticks_per_frame: DEFAULT_MAX_FIXED_TICKS_PER_FRAME,
            accumulator_ns: 0,
            tick: 0,
            ticks_to_run: 0,
            paused: false,
            scale: 1.0,
            day_index: 0,
            seconds_of_day: 0.0,
            seconds_per_game_day: SECONDS_PER_DAY,
            game_time_scale: 1.0,
            replay_deterministic: false,
            replay_seed: 0,
            replay_frame: 0,
            ai_tick_budget_ns: DEFAULT_AI_TICK_BUDGET_NS,
            ai_decision_tick_interval: DEFAULT_AI_DECISION_INTERVAL,
            scheduled_events: BTreeMap::new(),
        }
    }
}
