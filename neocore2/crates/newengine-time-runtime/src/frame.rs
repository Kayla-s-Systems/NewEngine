use std::time::Instant;

use newengine_time_api::{TimeBeginFrameRequestV1, TimeSnapshotV1};

const PROFILER_SAMPLE_TOPIC: &str = "newengine.diagnostics.profiler.sample.v1";
const FRAME_CADENCE_BASELINE_INTERVAL: u64 = 30;

use crate::{
    constants::{HARD_MAX_FIXED_TICKS_PER_FRAME, SECONDS_PER_DAY},
    state::RuntimeHostedTimeState,
};

impl RuntimeHostedTimeState {
    pub(crate) fn begin_frame(&mut self, request: TimeBeginFrameRequestV1) -> TimeSnapshotV1 {
        let now = Instant::now();
        let raw_delta_ns = now
            .duration_since(self.last)
            .as_nanos()
            .min(u128::from(u64::MAX)) as u64;
        self.last = now;
        self.begin_frame_with_raw_delta(request, raw_delta_ns)
    }

    pub(crate) fn begin_frame_with_raw_delta(
        &mut self,
        request: TimeBeginFrameRequestV1,
        raw_delta_ns: u64,
    ) -> TimeSnapshotV1 {
        self.frame_index = request.frame_index;
        if request.fixed_delta_ns > 0 {
            self.fixed_delta_ns = request.fixed_delta_ns;
        }
        self.last_raw_delta_ns = raw_delta_ns;

        let max_ticks = self
            .max_fixed_ticks_per_frame
            .clamp(1, HARD_MAX_FIXED_TICKS_PER_FRAME);
        let accumulator_cap = self
            .fixed_delta_ns
            .saturating_mul(u64::from(max_ticks))
            .max(self.fixed_delta_ns);
        self.last_clamped_delta_ns = raw_delta_ns.min(accumulator_cap);

        let scaled_delta_ns = if self.paused {
            0
        } else {
            ((self.last_clamped_delta_ns as f64) * self.scale.max(0.0)) as u64
        };
        let accumulated_before_cap = self.accumulator_ns.saturating_add(scaled_delta_ns);
        let debt_dropped = !self.paused
            && (raw_delta_ns > accumulator_cap || accumulated_before_cap > accumulator_cap);

        self.accumulator_ns = if self.paused {
            0
        } else {
            accumulated_before_cap.min(accumulator_cap)
        };
        self.ticks_to_run = if self.paused {
            0
        } else {
            (self.accumulator_ns / self.fixed_delta_ns).min(u64::from(max_ticks)) as u32
        };

        if !self.paused && self.seconds_per_game_day > f64::EPSILON {
            let game_delta_seconds = (self.last_clamped_delta_ns as f64 / 1_000_000_000.0)
                * self.game_time_scale.max(0.0)
                * (SECONDS_PER_DAY / self.seconds_per_game_day);
            let total_seconds = self.seconds_of_day + game_delta_seconds;
            let elapsed_days = (total_seconds / SECONDS_PER_DAY).floor() as u64;
            self.seconds_of_day = total_seconds.rem_euclid(SECONDS_PER_DAY);
            self.day_index = self.day_index.wrapping_add(elapsed_days);
        }

        self.replay_frame = self.replay_frame.wrapping_add(1);

        // Preserve rare frame-time spikes directly in profiler telemetry. The old
        // diagnostic only logged dropped debt on frame indices divisible by 120,
        // which made most micro-stutters invisible. Emit only slow frames plus a
        // sparse healthy baseline so telemetry itself stays off the hot path.
        let frame_budget_ns = self.fixed_delta_ns.max(1);
        let slow_frame_threshold_ns = frame_budget_ns.saturating_mul(5) / 4;
        let cadence_slow = raw_delta_ns > slow_frame_threshold_ns || debt_dropped;
        let cadence_hitch = raw_delta_ns > frame_budget_ns.saturating_mul(2) || debt_dropped;
        let cadence_baseline = self.frame_index > 0
            && self
                .frame_index
                .is_multiple_of(FRAME_CADENCE_BASELINE_INTERVAL);
        if self.frame_index > 0 && (cadence_hitch || cadence_baseline) {
            let raw_delta_ms = raw_delta_ns as f64 / 1_000_000.0;
            let frame_budget_ms = frame_budget_ns as f64 / 1_000_000.0;
            let clamped_ms = self.last_clamped_delta_ns as f64 / 1_000_000.0;
            let payload = serde_json::json!({
                "schema": "newengine.diagnostics.profiler.sample.v1",
                "category": "frame-cadence",
                "source": "newengine-time-runtime",
                "name": "realtime frame cadence",
                "lane": "main-frame",
                "priority": "critical",
                "dependency_group": format!("frame.{}.cadence", self.frame_index),
                "frame_index": self.frame_index,
                "elapsed_ms": raw_delta_ms,
                "budget_ms": frame_budget_ms,
                "frame_budget_ms": frame_budget_ms,
                "exceeded_frame_budget": raw_delta_ns > frame_budget_ns,
                "wait_reason": if cadence_slow { "realtime-frame-gap" } else { "baseline-sample" },
                "raw_delta_ms": raw_delta_ms,
                "clamped_delta_ms": clamped_ms,
                "ticks_to_run": self.ticks_to_run,
                "max_ticks": max_ticks,
                "debt_dropped": debt_dropped,
                "accumulator_ms": self.accumulator_ns as f64 / 1_000_000.0,
                "time_scale": self.scale,
            });
            if let Ok(bytes) = serde_json::to_vec(&payload) {
                let _ = newengine_plugin_host::emit_plugin_event(PROFILER_SAMPLE_TOPIC, &bytes);
            }
        }

        if debt_dropped && self.frame_index > 0 && self.frame_index.is_multiple_of(120) {
            newengine_ulog_api::ulog::warn!(
                "time gateway: realtime fixed-step debt dropped frame={} raw_delta_ns={} clamped_ns={} ticks_to_run={} max_ticks={} accumulator_ns={} scale={:.3}",
                self.frame_index,
                self.last_raw_delta_ns,
                self.last_clamped_delta_ns,
                self.ticks_to_run,
                self.max_fixed_ticks_per_frame,
                self.accumulator_ns,
                self.scale
            );
        }

        self.snapshot()
    }

    pub(crate) fn advance_fixed(&mut self) -> TimeSnapshotV1 {
        self.accumulator_ns = self.accumulator_ns.saturating_sub(self.fixed_delta_ns);
        self.tick = self.tick.wrapping_add(1);
        self.ticks_to_run = self.ticks_to_run.saturating_sub(1);
        self.snapshot()
    }
}
