use super::Engine;

use crate::error::{EngineError, EngineResult, ModuleStage};
use crate::frame::Frame;

use std::time::{Duration, Instant};

const MAX_ENGINE_FIXED_STEPS_PER_FRAME: u32 = 4;
const FIXED_CATCHUP_WARN_INTERVAL_FRAMES: u64 = 300;

/// Lightweight per-frame CPU phase timings published through `Resources`.
///
/// This intentionally measures coarse engine orchestration boundaries rather
/// than every module/plugin callback. It keeps the hot path allocation-free
/// while giving the external profiler enough information to localize frame
/// pacing spikes.
#[derive(Debug, Clone, Default)]
pub struct EngineFrameTimingTelemetry {
    pub frame_index: u64,
    pub fixed_steps: u32,
    pub total_ms: f64,
    pub time_begin_ms: f64,
    pub plugin_control_ms: f64,
    pub fixed_time_ms: f64,
    pub fixed_scheduler_ms: f64,
    pub fixed_plugins_ms: f64,
    pub fixed_modules_ms: f64,
    pub update_scheduler_ms: f64,
    pub update_plugins_ms: f64,
    pub update_modules_ms: f64,
    pub render_scheduler_ms: f64,
    pub render_plugins_ms: f64,
    pub render_modules_ms: f64,
    pub scheduler_end_ms: f64,
}

#[inline]
fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

use newengine_time_api::{
    time_method, TimeBeginFrameRequestV1, TimeSnapshotV1, ENGINE_TIME_SERVICE_ID,
};

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
        Ok(Some(bytes)) => serde_json::from_slice::<TimeSnapshotV1>(&bytes).map_err(|e| {
            EngineError::Other(format!(
                "engine.time: advance_fixed_v1 returned invalid TimeSnapshotV1: {e}"
            ))
        }),
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
            fixed_delta_ns: Duration::from_secs_f32(self.fixed_dt)
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64,
        };
        let payload = match serde_json::to_vec(&request) {
            Ok(payload) => payload,
            Err(e) => {
                return Err(EngineError::Other(format!(
                    "engine.time: failed to encode begin_frame_v1 request: {e}"
                )));
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
                    Ok(snapshot)
                }
                Err(e) => Err(EngineError::Other(format!(
                    "engine.time: begin_frame_v1 returned invalid TimeSnapshotV1: {e}"
                ))),
            },
            Ok(None) => Err(EngineError::Other(
                "engine.time: required gateway missing; register AstrolabeTimeProvider or another time.backend provider before frame loop".to_owned(),
            )),
            Err(e) => Err(EngineError::Other(format!(
                "engine.time: begin_frame_v1 failed: {e}"
            ))),
        }
    }

    pub fn begin_frame(&mut self) -> EngineResult<Frame> {
        self.activate_host_context();
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

        let engine_frame_started = Instant::now();
        let mut timing = EngineFrameTimingTelemetry {
            frame_index: self.frame_index,
            ..EngineFrameTimingTelemetry::default()
        };

        let phase_started = Instant::now();
        let time_snapshot = self.begin_time_frame_snapshot()?;
        timing.time_begin_ms = elapsed_ms(phase_started);
        // Keep the wall-clock anchor read-only until the Engine struct is
        // trimmed; frame time itself is now owned by engine.time.
        let _wall_clock_anchor = self.last;
        let mut dt = variable_delta_seconds_from_time(&time_snapshot);
        dt = dt.clamp(0.0, 0.2);
        self.acc = time_snapshot.simulation.accumulator_ns as f32 / 1_000_000_000.0;

        let frame_dt = Duration::from_secs_f32(dt);
        self.thread_pool.begin_configured_frame_budget();
        self.scheduler.begin_frame(frame_dt);

        let phase_started = Instant::now();
        self.process_plugin_control()?;
        timing.plugin_control_ms = elapsed_ms(phase_started);

        let mut steps_to_run = time_snapshot.simulation.ticks_to_run;
        if steps_to_run > MAX_ENGINE_FIXED_STEPS_PER_FRAME {
            if self
                .frame_index
                .is_multiple_of(FIXED_CATCHUP_WARN_INTERVAL_FRAMES)
            {
                newengine_ulog_api::ulog::warn!(
                    "engine.time: realtime fixed-step debt dropped frame={} requested_ticks={} per_frame_steps={} accumulator_ns={} fixed_delta_ns={}",
                    self.frame_index,
                    steps_to_run,
                    MAX_ENGINE_FIXED_STEPS_PER_FRAME,
                    time_snapshot.simulation.accumulator_ns,
                    time_snapshot.simulation.fixed_delta_ns,
                );
            }
            steps_to_run = MAX_ENGINE_FIXED_STEPS_PER_FRAME;
        }

        for step_index in 0..steps_to_run {
            self.sync_shutdown_state();
            if self.is_shutdown_requested() {
                return Err(EngineError::ExitRequested);
            }

            let phase_started = Instant::now();
            let fixed_snapshot = advance_time_fixed_snapshot()?;
            timing.fixed_time_ms += elapsed_ms(phase_started);
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

            let phase_started = Instant::now();
            self.scheduler
                .run_fixed_update(Duration::from_secs_f32(self.fixed_dt));
            timing.fixed_scheduler_ms += elapsed_ms(phase_started);

            let phase_started = Instant::now();
            if let Err(e) = self.plugins.fixed_update_all(self.fixed_dt) {
                return Err(EngineError::Other(format!(
                    "plugins: fixed_update failed: {e}"
                )));
            }
            timing.fixed_plugins_ms += elapsed_ms(phase_started);

            let phase_started = Instant::now();
            self.run_stage(&fixed_frame, ModuleStage::FixedUpdate, |m, ctx| {
                m.fixed_update(ctx)
            })?;
            timing.fixed_modules_ms += elapsed_ms(phase_started);
        }
        timing.fixed_steps = steps_to_run;

        let frame = Frame {
            frame_index: self.frame_index,
            dt,
            fixed_dt: self.fixed_dt,
            fixed_alpha: (self.acc / self.fixed_dt).clamp(0.0, 0.999_999),
            fixed_step_count: steps_to_run,
            fixed_step_index: 0,
            fixed_tick: self.fixed_tick,
        };

        let phase_started = Instant::now();
        self.scheduler.run_update(frame_dt);
        timing.update_scheduler_ms = elapsed_ms(phase_started);

        let phase_started = Instant::now();
        if let Err(e) = self.plugins.update_all(dt) {
            return Err(EngineError::Other(format!("plugins: update failed: {e}")));
        }
        timing.update_plugins_ms = elapsed_ms(phase_started);

        let phase_started = Instant::now();
        self.run_stage(&frame, ModuleStage::Update, |m, ctx| m.update(ctx))?;
        timing.update_modules_ms = elapsed_ms(phase_started);

        let phase_started = Instant::now();
        self.scheduler.run_render(frame_dt);
        timing.render_scheduler_ms = elapsed_ms(phase_started);

        let phase_started = Instant::now();
        if let Err(e) = self.plugins.render_all(dt) {
            return Err(EngineError::Other(format!("plugins: render failed: {e}")));
        }
        timing.render_plugins_ms = elapsed_ms(phase_started);

        let phase_started = Instant::now();
        self.run_stage(&frame, ModuleStage::Render, |m, ctx| m.render(ctx))?;
        timing.render_modules_ms = elapsed_ms(phase_started);

        let phase_started = Instant::now();
        self.scheduler.end_frame(frame_dt);
        timing.scheduler_end_ms = elapsed_ms(phase_started);
        timing.total_ms = elapsed_ms(engine_frame_started);
        self.resources.insert(timing);
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
