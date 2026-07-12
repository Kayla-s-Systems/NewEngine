use newengine_time_api::{
    TimeFixedStepRequestV1, TimeGameClockSetRequestV1, TimePauseRequestV1,
    TimeReplayClockSetRequestV1, TimeScaleRequestV1, TimeSnapshotV1,
};

use crate::{
    constants::{HARD_MAX_FIXED_TICKS_PER_FRAME, SECONDS_PER_DAY},
    state::RuntimeHostedTimeState,
};

impl RuntimeHostedTimeState {
    pub(crate) fn set_pause(&mut self, request: TimePauseRequestV1) -> TimeSnapshotV1 {
        self.paused = request.paused;
        self.snapshot()
    }

    pub(crate) fn set_scale(&mut self, request: TimeScaleRequestV1) -> TimeSnapshotV1 {
        self.scale = request.scale.clamp(0.0, 64.0);
        self.snapshot()
    }

    pub(crate) fn set_game_clock(&mut self, request: TimeGameClockSetRequestV1) -> TimeSnapshotV1 {
        self.day_index = request.day_index;
        self.seconds_of_day = request.seconds_of_day.rem_euclid(SECONDS_PER_DAY);
        if request.seconds_per_game_day > f64::EPSILON {
            self.seconds_per_game_day = request.seconds_per_game_day;
        }
        self.game_time_scale = request.time_scale.max(0.0);
        self.snapshot()
    }

    pub(crate) fn set_fixed_step(&mut self, request: TimeFixedStepRequestV1) -> TimeSnapshotV1 {
        if request.fixed_delta_ns > 0 {
            self.fixed_delta_ns = request.fixed_delta_ns;
        }
        self.max_fixed_ticks_per_frame = request
            .max_fixed_ticks_per_frame
            .clamp(1, HARD_MAX_FIXED_TICKS_PER_FRAME);
        self.ai_decision_tick_interval = request.ai_decision_tick_interval.max(1);
        self.ai_tick_budget_ns = request.ai_tick_budget_ns.max(1_000);
        self.snapshot()
    }

    pub(crate) fn set_replay_clock(
        &mut self,
        request: TimeReplayClockSetRequestV1,
    ) -> TimeSnapshotV1 {
        self.replay_deterministic = request.deterministic;
        self.replay_seed = request.seed;
        self.replay_frame = request.replay_frame;
        self.snapshot()
    }
}
