#![forbid(unsafe_op_in_unsafe_fn)]

//! Headless runtime fallback for launches where no platform runtime is present.
//!
//! This keeps platform/window integration as an optional provider path. A missing
//! `platform-winit` DLL must not be a fatal engine error: the host can still load
//! plugins, validate contracts, run the startup graph, perform content/bootstrap
//! work and produce logs/diagnostics from a CLI session.

use std::time::Instant;

use newengine_core::host_events::{HostEvent, WindowHostEvent, WindowInitSize};
use newengine_core::{Engine, EngineError, EngineResult, EngineRunState};

use crate::platform_runtime::register_jobs_gateway_service_best_effort;

const DEFAULT_HEADLESS_WIDTH: u32 = 1;
const DEFAULT_HEADLESS_HEIGHT: u32 = 1;
const DEFAULT_STARTUP_STEP_LIMIT: u32 = 4_096;
const DEFAULT_HEADLESS_FRAMES: u64 = 0;

pub(crate) struct HeadlessCliRuntime {
    engine: Engine<()>,
    fixed_dt_sec: f32,
    startup_step_limit: u32,
    frame_limit: Option<u64>,
}

impl HeadlessCliRuntime {
    pub(crate) fn new(engine: Engine<()>, fixed_dt_ms: u32) -> Self {
        Self {
            engine,
            fixed_dt_sec: (fixed_dt_ms.max(1) as f32) / 1000.0,
            startup_step_limit: env_u32("NEWENGINE_HEADLESS_STARTUP_STEP_LIMIT", DEFAULT_STARTUP_STEP_LIMIT),
            frame_limit: headless_frame_limit_from_env(),
        }
    }

    pub(crate) fn run(mut self, reason: impl AsRef<str>) -> EngineResult<()> {
        let reason = reason.as_ref();
        log::warn!(
            "headless runtime: entering CLI fallback reason='{}' startup_step_limit={} frame_limit={} fixed_dt_sec={:.4}",
            reason,
            self.startup_step_limit,
            self.frame_limit
                .map(|v| v.to_string())
                .unwrap_or_else(|| "forever".to_owned()),
            self.fixed_dt_sec
        );
        newengine_core::crash::record_breadcrumb(format!(
            "headless runtime: entering CLI fallback reason='{reason}'"
        ));

        self.install_headless_services();
        self.publish_headless_window_contract()?;

        let plugin_count = self.engine.load_engine_plugins_once()?;
        log::info!(
            "headless runtime: engine plugins loaded count={} mode='platformless-cli'",
            plugin_count
        );

        self.start_engine_incrementally()?;
        self.run_frames()?;
        self.shutdown_engine_once("headless runtime completed");
        Ok(())
    }

    fn install_headless_services(&mut self) {
        newengine_time_runtime::register_time_gateway_best_effort();
        register_jobs_gateway_service_best_effort(self.engine.job_system(), self.engine.events().clone());
        log::info!("headless runtime: engine.time and engine.jobs gateways registered; loading/status stays an engine.ui projection");
    }

    fn publish_headless_window_contract(&mut self) -> EngineResult<()> {
        self.engine.resources_mut().insert(WindowInitSize {
            width: DEFAULT_HEADLESS_WIDTH,
            height: DEFAULT_HEADLESS_HEIGHT,
        });
        self.engine.emit(HostEvent::Window(WindowHostEvent::Ready {
            width: DEFAULT_HEADLESS_WIDTH,
            height: DEFAULT_HEADLESS_HEIGHT,
        }))?;
        log::info!(
            "headless runtime: synthetic WindowReady emitted size={}x{} no_native_handles=true",
            DEFAULT_HEADLESS_WIDTH,
            DEFAULT_HEADLESS_HEIGHT
        );
        Ok(())
    }

    fn start_engine_incrementally(&mut self) -> EngineResult<()> {
        let started = Instant::now();
        for step in 1..=self.startup_step_limit {
            let outcome = self.engine.start_incremental_step()?;
            let snapshot = outcome.snapshot;
            if step == 1 || outcome.finished || step % 16 == 0 || snapshot.terminal {
                log::info!(
                    "headless startup: step={} phase='{}' status='{}' detail='{}' progress={:.0}% finished={} terminal={}",
                    step,
                    snapshot.phase.as_str(),
                    snapshot.status,
                    snapshot.detail,
                    snapshot.progress_01 * 100.0,
                    outcome.finished,
                    snapshot.terminal
                );
            }

            if outcome.finished {
                log::info!(
                    "headless runtime: startup completed elapsed_ms={:.2}",
                    started.elapsed().as_secs_f64() * 1000.0
                );
                return Ok(());
            }

            if snapshot.terminal && self.engine.run_state().is_terminal() {
                return Err(EngineError::other(format!(
                    "headless startup reached terminal state before running: {}",
                    snapshot.error.unwrap_or_else(|| snapshot.detail)
                )));
            }

            // No sleep-loop here: startup is advanced by explicit engine.step() calls.
        }

        Err(EngineError::other(format!(
            "headless startup did not finish within {} incremental step(s)",
            self.startup_step_limit
        )))
    }

    fn run_frames(&mut self) -> EngineResult<()> {
        let Some(frame_limit) = self.frame_limit else {
            log::info!("headless runtime: entering unlimited frame pump; request shutdown to exit");
            let mut frames = 0_u64;
            while !self.engine.run_state().is_terminal() {
                match self.engine.step() {
                    Ok(()) => {}
                    Err(EngineError::ExitRequested) => break,
                    Err(e) => return Err(e),
                }
                frames = frames.wrapping_add(1);
                if frames % 300 == 0 {
                    log::info!("headless runtime: frames={} run_state='{}'", frames, self.engine.run_state().as_str());
                }
                // Headless pacing is owned by engine.time / caller event pump, not by a local sleep loop.
            }
            return Ok(());
        };

        if frame_limit == 0 {
            log::info!("headless runtime: startup-only CLI mode completed; no frame pump requested");
            return Ok(());
        }

        for frame in 0..frame_limit {
            match self.engine.step() {
                Ok(()) => {}
                Err(EngineError::ExitRequested) => break,
                Err(e) => return Err(e),
            }
            if frame == 0 || (frame + 1) % 300 == 0 || frame + 1 == frame_limit {
                log::info!(
                    "headless runtime: frame={}/{} run_state='{}'",
                    frame + 1,
                    frame_limit,
                    self.engine.run_state().as_str()
                );
            }
            // Headless pacing is owned by engine.time / caller event pump, not by a local sleep loop.
        }
        Ok(())
    }

    fn shutdown_engine_once(&mut self, origin: &'static str) {
        if matches!(self.engine.run_state(), EngineRunState::Stopped | EngineRunState::Faulted) {
            return;
        }
        log::info!("headless runtime: engine.shutdown begin origin={origin}");
        if let Err(e) = self.engine.shutdown() {
            log::error!("headless runtime: engine.shutdown failed origin={origin}: {e}");
        } else {
            log::info!("headless runtime: engine.shutdown completed origin={origin}");
        }
    }
}

fn headless_frame_limit_from_env() -> Option<u64> {
    if env_bool("NEWENGINE_HEADLESS_RUN_FOREVER", false) {
        return None;
    }
    Some(env_u64("NEWENGINE_HEADLESS_FRAMES", DEFAULT_HEADLESS_FRAMES))
}

fn env_u32(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(default)
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(default)
}
