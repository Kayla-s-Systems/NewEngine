use super::Engine;

use crate::error::{EngineError, EngineResult, ModuleStage};
use crate::frame::Frame;

use std::time::{Duration, Instant};

impl<E: Send + 'static> Engine<E> {
    pub fn begin_frame(&mut self) -> EngineResult<Frame> {
        self.sync_shutdown_state();
        if self.is_exit_requested() {
            return Err(EngineError::ExitRequested);
        }

        if !self.started {
            return Err(EngineError::Other(
                "engine.begin_frame called before engine.start".to_string(),
            ));
        }

        let now = Instant::now();
        let mut dt = (now - self.last).as_secs_f32();
        self.last = now;

        dt = dt.clamp(0.0, 0.2);

        self.acc = (self.acc + dt).min(1.0);

        self.scheduler.begin_frame(Duration::from_secs_f32(dt));

        self.process_plugin_control();
        self.expose_plugins_snapshot();

        let mut steps_to_run = (self.acc / self.fixed_dt).floor() as u32;
        steps_to_run = steps_to_run.min(8);

        for step_index in 0..steps_to_run {
            self.sync_shutdown_state();
            if self.is_exit_requested() {
                return Err(EngineError::ExitRequested);
            }

            self.acc -= self.fixed_dt;
            self.fixed_tick = self.fixed_tick.wrapping_add(1);

            let fixed_frame = Frame {
                frame_index: self.frame_index,
                dt: self.fixed_dt,
                fixed_dt: self.fixed_dt,
                fixed_alpha: 0.0,
                fixed_step_count: steps_to_run,
                fixed_step_index: step_index,
                fixed_tick: self.fixed_tick,
            };

            if let Err(e) = self.plugins.fixed_update_all(self.fixed_dt) {
                return Err(EngineError::Other(format!("plugins: fixed_update failed: {e}")));
            }

            self.run_stage(&fixed_frame, ModuleStage::FixedUpdate, |m, ctx| m.fixed_update(ctx))?;
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

        if let Err(e) = self.plugins.update_all(dt) {
            return Err(EngineError::Other(format!("plugins: update failed: {e}")));
        }
        self.run_stage(&frame, ModuleStage::Update, |m, ctx| m.update(ctx))?;

        if let Err(e) = self.plugins.render_all(dt) {
            return Err(EngineError::Other(format!("plugins: render failed: {e}")));
        }
        self.run_stage(&frame, ModuleStage::Render, |m, ctx| m.render(ctx))?;

        self.scheduler.end_frame(Duration::from_secs_f32(dt));
        self.frame_index = self.frame_index.wrapping_add(1);

        Ok(frame)
    }

    /// Single engine tick (compat facade).
    ///
    /// Keeps external runners stable. Internally delegates to `begin_frame()`.
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
