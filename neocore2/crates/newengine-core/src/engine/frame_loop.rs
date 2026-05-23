use super::Engine;

use crate::error::{EngineError, EngineResult, ModuleStage};
use crate::frame::Frame;

use std::time::Duration;

use newengine_time_api::{time_method, TimeBeginFrameRequestV1, TimeSnapshotV1, ENGINE_TIME_SERVICE_ID};

fn variable_delta_seconds_from_time(snapshot: &TimeSnapshotV1) -> f32 {
    if snapshot.simulation.paused {
        return 0.0;
    }
    let seconds = snapshot.real.clamped_delta_ns as f64 / 1_000_000_000.0;
    (seconds * snapshot.simulation.scale.max(0.0)).clamp(0.0, 0.2) as f32
}

fn advance_time_fixed_snapshot() -> EngineResult<TimeSnapshotV1> {
    match crate::call_service_v1_optional(
        ENGINE_TIME_SERVICE_ID,
        time_method::ADVANCE_FIXED_V1,
        &[],
    ) {
        Ok(Some(bytes)) => serde_json::from_slice::<TimeSnapshotV1>(&bytes)
            .map_err(|e| EngineError::Other(format!(
                "engine.time: advance_fixed_v1 returned invalid TimeSnapshotV1: {e}"
            ))),
        Ok(None) => Err(EngineError::Other(
            "engine.time: required gateway missing during fixed timestep advance".to_owned(),
        )),
        Err(e) => Err(EngineError::Other(format!(
            "engine.time: advance_fixed_v1 failed: {e}"
        ))),
    }
}

impl<E: Send + 'static> Engine<E> {
    fn begin_time_frame_snapshot(&mut self) -> EngineResult<TimeSnapshotV1> {
        let request = TimeBeginFrameRequestV1 {
            frame_index: self.frame_index,
            fixed_delta_ns: Duration::from_secs_f32(self.fixed_dt).as_nanos().min(u128::from(u64::MAX)) as u64,
        };
        let payload = match serde_json::to_vec(&request) {
            Ok(payload) => payload,
            Err(e) => {
                return Err(EngineError::Other(format!("engine.time: failed to encode begin_frame_v1 request: {e}")));
            }
        };
        match crate::call_service_v1_optional(
            ENGINE_TIME_SERVICE_ID,
            time_method::BEGIN_FRAME_V1,
            &payload,
        ) {
            Ok(Some(bytes)) => match serde_json::from_slice::<TimeSnapshotV1>(&bytes) {
                Ok(snapshot) => {
                    self.acc = snapshot.simulation.accumulator_ns as f32 / 1_000_000_000.0;
                    log::debug!(
                        "engine.time: begin_frame_v1 frame={} delta_ns={} fixed_ticks={} normalized_day={:.6}",
                        snapshot.frame_index,
                        snapshot.real.clamped_delta_ns,
                        snapshot.simulation.ticks_to_run,
                        snapshot.game.normalized_day
                    );
                    Ok(snapshot)
                }
                Err(e) => Err(EngineError::Other(format!(
                    "engine.time: begin_frame_v1 returned invalid TimeSnapshotV1: {e}"
                ))),
            },
            Ok(None) => Err(EngineError::Other(
                "engine.time: required gateway missing; register EngineOwnedTimeProvider or another time.backend provider before frame loop".to_owned(),
            )),
            Err(e) => Err(EngineError::Other(format!(
                "engine.time: begin_frame_v1 failed: {e}"
            ))),
        }
    }

    pub fn begin_frame(&mut self) -> EngineResult<Frame> {
        self.sync_shutdown_state();
        if self.is_shutdown_requested() {
            return Err(EngineError::ExitRequested);
        }

        if !self.fsm.can_run_frame() {
            return Err(EngineError::Other(format!(
                "engine.begin_frame requires running core FSM state; current={}",
                self.run_state().as_str()
            )));
        }

        let time_snapshot = self.begin_time_frame_snapshot()?;
        // Keep the wall-clock anchor read-only until the Engine struct is
        // trimmed; frame time itself is now owned by engine.time.
        let _wall_clock_anchor = self.last;
        let mut dt = variable_delta_seconds_from_time(&time_snapshot);
        dt = dt.clamp(0.0, 0.2);
        self.acc = time_snapshot.simulation.accumulator_ns as f32 / 1_000_000_000.0;

        let frame_dt = Duration::from_secs_f32(dt);
        self.scheduler.begin_frame(frame_dt);

        self.process_plugin_control()?;
        self.expose_plugins_snapshot();

        let mut steps_to_run = time_snapshot.simulation.ticks_to_run;
        steps_to_run = steps_to_run.min(8);

        for step_index in 0..steps_to_run {
            self.sync_shutdown_state();
            if self.is_shutdown_requested() {
                return Err(EngineError::ExitRequested);
            }

            let fixed_snapshot = advance_time_fixed_snapshot()?;
            self.fixed_tick = fixed_snapshot.simulation.tick;
            self.acc = fixed_snapshot.simulation.accumulator_ns as f32 / 1_000_000_000.0;

            let fixed_frame = Frame {
                frame_index: self.frame_index,
                dt: self.fixed_dt,
                fixed_dt: self.fixed_dt,
                fixed_alpha: 0.0,
                fixed_step_count: steps_to_run,
                fixed_step_index: step_index,
                fixed_tick: self.fixed_tick,
            };

            self.scheduler.run_fixed_update(Duration::from_secs_f32(self.fixed_dt));

            if let Err(e) = self.plugins.fixed_update_all(self.fixed_dt) {
                return Err(EngineError::Other(format!(
                    "plugins: fixed_update failed: {e}"
                )));
            }

            self.run_stage(&fixed_frame, ModuleStage::FixedUpdate, |m, ctx| {
                m.fixed_update(ctx)
            })?;
        }

        let frame = Frame {
            frame_index: self.frame_index,
            dt,
            fixed_dt: self.fixed_dt,
            fixed_alpha: (self.acc / self.fixed_dt).clamp(0.0, 0.999_999),
            fixed_step_count: steps_to_run,
            fixed_step_index: 0,
            fixed_tick: self.fixed_tick,
        };

        self.scheduler.run_update(frame_dt);

        if let Err(e) = self.plugins.update_all(dt) {
            return Err(EngineError::Other(format!("plugins: update failed: {e}")));
        }
        self.run_stage(&frame, ModuleStage::Update, |m, ctx| m.update(ctx))?;

        self.scheduler.run_render(frame_dt);

        if let Err(e) = self.plugins.render_all(dt) {
            return Err(EngineError::Other(format!("plugins: render failed: {e}")));
        }
        self.run_stage(&frame, ModuleStage::Render, |m, ctx| m.render(ctx))?;

        self.scheduler.end_frame(frame_dt);
        self.frame_index = self.frame_index.wrapping_add(1);

        Ok(frame)
    }

    /// Single engine tick.
    ///
    /// Delegates to `begin_frame()` and propagates shutdown state.
    #[inline]
    pub fn step(&mut self) -> EngineResult<()> {
        let _ = self.begin_frame()?;
        self.propagate_shutdown_request();
        Ok(())
    }

    /// Single engine tick returning the computed frame (optional helper).
    #[inline]
    pub fn step_frame(&mut self) -> EngineResult<Frame> {
        let frame = self.begin_frame()?;
        self.propagate_shutdown_request();
        Ok(frame)
    }
}
