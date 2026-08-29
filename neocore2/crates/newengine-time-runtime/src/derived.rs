use newengine_time_api::{
    TimeAiClockV1, TimeAiContextV1, TimeGameClockV1, TimeRealClockV1, TimeReplayClockV1,
    TimeSimulationClockV1, TimeSnapshotV1, TimeTimelineV1, TIME_RUNTIME_CONTRACT,
};

use crate::{
    constants::{PROVIDER_NAME, SECONDS_PER_DAY},
    state::RuntimeHostedTimeState,
};

impl RuntimeHostedTimeState {
    #[inline]
    pub(crate) fn monotonic_ns(&self) -> u64 {
        self.start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    }

    #[inline]
    pub(crate) fn normalized_day(&self) -> f64 {
        if self.seconds_per_game_day <= f64::EPSILON {
            0.0
        } else {
            (self.seconds_of_day / SECONDS_PER_DAY).rem_euclid(1.0)
        }
    }

    #[inline]
    fn time_of_day_phase(normalized_day: f64) -> &'static str {
        let hour = (normalized_day * 24.0).rem_euclid(24.0);
        match hour {
            h if h < 5.0 => "night",
            h if h < 8.0 => "dawn",
            h if h < 17.0 => "day",
            h if h < 20.0 => "dusk",
            _ => "night",
        }
    }

    #[inline]
    fn next_ai_decision_tick(&self) -> u64 {
        let interval = u64::from(self.ai_decision_tick_interval.max(1));
        let remainder = self.tick % interval;
        if remainder == 0 {
            self.tick
        } else {
            self.tick + (interval - remainder)
        }
    }

    fn ai_deterministic_key(&self) -> String {
        format!(
            "ai-time:v1:seed={:016x}:day={}:tick={}:frame={}",
            self.replay_seed, self.day_index, self.tick, self.replay_frame,
        )
    }

    fn ai_clock_with_normalized_day(&self, normalized_day: f64) -> TimeAiClockV1 {
        TimeAiClockV1 {
            tick_budget_ns: self.ai_tick_budget_ns,
            decision_tick_interval: self.ai_decision_tick_interval.max(1),
            next_decision_tick: self.next_ai_decision_tick(),
            time_of_day_phase: Self::time_of_day_phase(normalized_day).to_owned(),
            normalized_day,
            deterministic_key: self.ai_deterministic_key(),
        }
    }

    pub(crate) fn timeline(&self) -> TimeTimelineV1 {
        TimeTimelineV1 {
            frame_index: self.frame_index,
            fixed_tick: self.tick,
            game_day_index: self.day_index,
            game_seconds_of_day: self.seconds_of_day,
            replay_frame: self.replay_frame,
            paused: self.paused,
            scale: self.scale,
            ..Default::default()
        }
    }

    pub(crate) fn game_clock(&self) -> TimeGameClockV1 {
        TimeGameClockV1 {
            day_index: self.day_index,
            seconds_of_day: self.seconds_of_day,
            normalized_day: self.normalized_day(),
            seconds_per_game_day: self.seconds_per_game_day,
            time_scale: self.game_time_scale,
        }
    }

    pub(crate) fn replay_clock(&self) -> TimeReplayClockV1 {
        TimeReplayClockV1 {
            deterministic: self.replay_deterministic,
            seed: self.replay_seed,
            replay_frame: self.replay_frame,
        }
    }

    pub(crate) fn ai_context(&self) -> TimeAiContextV1 {
        let normalized_day = self.normalized_day();
        TimeAiContextV1 {
            frame_index: self.frame_index,
            simulation_tick: self.tick,
            fixed_delta_ns: self.fixed_delta_ns,
            game_day_index: self.day_index,
            game_seconds_of_day: self.seconds_of_day,
            normalized_day,
            time_of_day_phase: Self::time_of_day_phase(normalized_day).to_owned(),
            deterministic: self.replay_deterministic,
            replay_seed: self.replay_seed,
            replay_frame: self.replay_frame,
            decision_tick_interval: self.ai_decision_tick_interval.max(1),
            next_decision_tick: self.next_ai_decision_tick(),
            tick_budget_ns: self.ai_tick_budget_ns,
            deterministic_key: self.ai_deterministic_key(),
            ..Default::default()
        }
    }

    pub(crate) fn snapshot(&self) -> TimeSnapshotV1 {
        let normalized_day = self.normalized_day();
        TimeSnapshotV1 {
            schema: TIME_RUNTIME_CONTRACT.to_owned(),
            provider: PROVIDER_NAME.to_owned(),
            frame_index: self.frame_index,
            real: TimeRealClockV1 {
                monotonic_ns: self.monotonic_ns(),
                delta_ns: self.last_raw_delta_ns,
                clamped_delta_ns: self.last_clamped_delta_ns,
            },
            simulation: TimeSimulationClockV1 {
                tick: self.tick,
                fixed_delta_ns: self.fixed_delta_ns,
                accumulator_ns: self.accumulator_ns,
                ticks_to_run: self.ticks_to_run,
                paused: self.paused,
                scale: self.scale,
            },
            game: TimeGameClockV1 {
                day_index: self.day_index,
                seconds_of_day: self.seconds_of_day,
                normalized_day,
                seconds_per_game_day: self.seconds_per_game_day,
                time_scale: self.game_time_scale,
            },
            replay: self.replay_clock(),
            ai: self.ai_clock_with_normalized_day(normalized_day),
        }
    }
}
