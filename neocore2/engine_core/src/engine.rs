use std::{collections::HashMap, fs, path::Path, time::Instant};

use anyhow::{anyhow, Result};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

use crate::{
    config::{EngineConfig, ModuleConfig},
    frame::{FrameConstitution, FrameContext},
    log::Logger,
    module::Module,
    phase::FramePhase,
    schedule::FrameSchedule,
    signals::ExitSignal,
    telemetry::Telemetry,
    time::Time,
};

// ✅ Лучший фасад: внешний код импортирует EngineConfig через engine модуль.
// app может писать: use engine_core::engine::{Engine, EngineConfig};
pub use crate::config::EngineConfig as EngineConfigPublic;

pub type ModuleFactory = fn(&toml::Value) -> Result<Box<dyn Module>>;

pub struct Engine {
    cfg: EngineConfig,
    log: Logger,
    schedule: FrameSchedule,
    factories: HashMap<String, ModuleFactory>,
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Self {
        Self {
            cfg,
            log: Logger::new("Engine"),
            schedule: FrameSchedule::new(),
            factories: HashMap::new(),
        }
    }

    /// ✅ Публичный доступ к конфигу (только чтение) — полезно для app/инструментов.
    pub fn config(&self) -> &EngineConfig {
        &self.cfg
    }

    /// ✅ Dev/Tests/Bootstrap: добавить модуль напрямую (без TOML).
    /// Это не ломает "интерпретируемость": конфиг остаётся главным,
    /// но мы оставляем быстрый путь для прототипов и внутренних модулей.
    pub fn add_module<M: Module + 'static>(&mut self, m: M) {
        self.schedule.add_boxed(Box::new(m));
    }

    /// Регистрация модулей платформы (движка).
    /// Ядро не зависит от модулей: модули подключаются по id через конфиг.
    pub fn register_module_factory(&mut self, id: &str, f: ModuleFactory) {
        self.factories.insert(id.to_string(), f);
    }

    /// Сборка пайплайна из конфигурации (интерпретируемо).
    /// ВАЖНО: если кто-то уже добавил модули вручную — они останутся первыми.
    pub fn build_schedule_from_config(&mut self) -> Result<()> {
        for m in self.cfg.modules.iter() {
            if !m.enabled {
                self.log.info(format!("module '{}' disabled by config", m.id));
                continue;
            }
            let boxed = self.instantiate_module(m)?;
            self.log.info(format!("module '{}' loaded", m.id));
            self.schedule.add_boxed(boxed);
        }
        Ok(())
    }

    fn instantiate_module(&self, m: &ModuleConfig) -> Result<Box<dyn Module>> {
        let Some(f) = self.factories.get(&m.id) else {
            return Err(anyhow!("module factory not registered for id='{}'", m.id));
        };
        f(&m.settings)
    }

    pub fn run(mut self) -> Result<()> {
        // ✅ Конфиг-модули достраиваются при старте.
        // Ручные модули (add_module) уже лежат в schedule.
        self.build_schedule_from_config()?;

        let event_loop = EventLoop::new()?;
        let mut app = EngineApp::new(self);
        event_loop.run_app(&mut app)?;
        Ok(())
    }

    pub fn load_config_toml(path: impl AsRef<Path>) -> Result<EngineConfig> {
        let text = fs::read_to_string(path)?;
        let cfg: EngineConfig = toml::from_str(&text)?;
        Ok(cfg)
    }
}

/// EngineApp — runtime-обвязка вокруг winit.
/// Важно: хранит snapshot нужных runtime-параметров,
/// чтобы не лезть в приватности Engine и не держать лишние borrow’ы.
struct EngineApp {
    engine: Engine,

    window: Option<Window>,
    window_id: Option<WindowId>,

    exit_requested: bool,
    shutdown_done: bool,
    started: bool,

    constitution: FrameConstitution,
    time: Time,
    telemetry: Telemetry,

    last: Instant,
    accumulator: f32,

    exit_signal: ExitSignal,
    last_fixed_tick_logged: u64,

    // runtime snapshot
    control_flow_poll: bool,
    window_title: String,
    window_w: u32,
    window_h: u32,
}

impl EngineApp {
    fn new(engine: Engine) -> Self {
        // ✅ Снимаем snapshot того, что нужно рантайму
        let title = engine.cfg.window.title.clone();
        let w = engine.cfg.window.width;
        let h = engine.cfg.window.height;

        let fixed_hz = engine.cfg.frame.fixed_hz.max(1);
        let fixed_dt = 1.0 / (fixed_hz as f32);

        let constitution = FrameConstitution {
            fixed_dt_sec: fixed_dt,
            max_fixed_steps_per_frame: engine.cfg.frame.max_fixed_steps_per_frame.max(1),
            max_dt_sec: (engine.cfg.frame.max_dt_ms as f32 / 1000.0).max(0.001),
            log_fps: engine.cfg.frame.log_fps,
            fps_log_period_sec: (engine.cfg.frame.fps_log_period_ms as f32 / 1000.0).max(0.25),
        };

        let exit_signal = ExitSignal::new();
        let _ = exit_signal.install_ctrlc_handler();

        let mut telemetry = Telemetry::new();
        telemetry.configure_fps_logging(constitution.log_fps, constitution.fps_log_period_sec);

        let control_flow_poll = engine
            .cfg
            .runtime
            .control_flow
            .to_ascii_lowercase()
            .trim()
            == "poll";

        Self {
            engine,

            window: None,
            window_id: None,

            exit_requested: false,
            shutdown_done: false,
            started: false,

            constitution,
            time: Time::new(fixed_dt),
            telemetry,

            last: Instant::now(),
            accumulator: 0.0,

            exit_signal,
            last_fixed_tick_logged: 0,

            control_flow_poll,
            window_title: title,
            window_w: w,
            window_h: h,
        }
    }

    fn start_if_needed(&mut self) {
        if self.started {
            return;
        }
        let Some(window) = self.window.as_ref() else { return; };

        self.engine.log.info("boot");

        let mut ctx = FrameContext {
            window,
            log: &self.engine.log,
            time: &mut self.time,
            telemetry: &mut self.telemetry,
            exit_requested: &mut self.exit_requested,
        };

        self.engine.schedule.on_register(&mut ctx);
        self.engine.schedule.on_start(&mut ctx);

        self.started = true;
        self.last = Instant::now();
        self.accumulator = 0.0;

        self.engine.log.info("first frame");
    }

    fn shutdown_once(&mut self, el: &ActiveEventLoop) {
        if self.shutdown_done {
            return;
        }
        self.shutdown_done = true;

        if let Some(window) = self.window.as_ref() {
            let mut ctx = FrameContext {
                window,
                log: &self.engine.log,
                time: &mut self.time,
                telemetry: &mut self.telemetry,
                exit_requested: &mut self.exit_requested,
            };
            self.engine.schedule.on_shutdown(&mut ctx);
        }

        self.engine.log.info("shutdown");
        el.exit();
    }
}

impl ApplicationHandler for EngineApp {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title(self.window_title.clone())
            .with_inner_size(LogicalSize::new(self.window_w, self.window_h));

        let window = match el.create_window(attrs) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("failed to create window: {e}");
                el.exit();
                return;
            }
        };

        self.window_id = Some(window.id());
        self.window = Some(window);

        self.start_if_needed();
    }

    fn window_event(&mut self, _el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if Some(id) != self.window_id {
            return;
        }

        match event {
            WindowEvent::CloseRequested => self.exit_requested = true,
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    if let PhysicalKey::Code(code) = event.physical_key {
                        if code == KeyCode::Escape {
                            self.exit_requested = true;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        el.set_control_flow(if self.control_flow_poll {
            ControlFlow::Poll
        } else {
            ControlFlow::Wait
        });

        if !self.started {
            return;
        }

        if self.exit_signal.is_exit_requested() {
            self.exit_requested = true;
        }
        if self.exit_requested {
            self.shutdown_once(el);
            return;
        }

        let now = Instant::now();
        let raw_dt = now.duration_since(self.last);
        self.last = now;

        let dt_sec = raw_dt.as_secs_f32().min(self.constitution.max_dt_sec);

        self.time.dt_sec = dt_sec;
        self.time.t_sec += raw_dt.as_secs_f64();
        self.time.frame_index += 1;

        self.accumulator += dt_sec;

        let Some(window) = self.window.as_ref() else { return; };

        let mut ctx = FrameContext {
            window,
            log: &self.engine.log,
            time: &mut self.time,
            telemetry: &mut self.telemetry,
            exit_requested: &mut self.exit_requested,
        };

        self.engine.schedule.run_phase(FramePhase::BeginFrame, &mut ctx);
        self.engine.schedule.run_phase(FramePhase::Input, &mut ctx);

        // FixedUpdate with cap (anti spiral-of-death)
        let mut steps: u32 = 0;
        while self.accumulator >= self.constitution.fixed_dt_sec {
            if steps >= self.constitution.max_fixed_steps_per_frame {
                self.accumulator = 0.0;
                ctx.log.warn("fixed cap reached (spiral prevented)");
                break;
            }

            ctx.time.fixed_tick_index += 1;
            self.engine.schedule.run_phase(FramePhase::FixedUpdate, &mut ctx);

            self.accumulator -= self.constitution.fixed_dt_sec;
            steps += 1;

            let tick = ctx.time.fixed_tick_index;
            if tick / 60 != self.last_fixed_tick_logged / 60 && (tick % 60 == 0) {
                self.last_fixed_tick_logged = tick;
                ctx.log.debug(format!("fixed tick {}", tick));
            }
        }

        ctx.time.fixed_alpha =
            (self.accumulator / self.constitution.fixed_dt_sec).clamp(0.0, 1.0);

        self.engine.schedule.run_phase(FramePhase::Update, &mut ctx);
        self.engine.schedule.run_phase(FramePhase::LateUpdate, &mut ctx);

        // AAA boundary: two-world pipeline (Extract/Prepare/Render)
        self.engine.schedule.run_phase(FramePhase::Extract, &mut ctx);
        self.engine.schedule.run_phase(FramePhase::Prepare, &mut ctx);
        self.engine.schedule.run_phase(FramePhase::Render, &mut ctx);

        self.engine.schedule.run_phase(FramePhase::Present, &mut ctx);
        self.engine.schedule.run_phase(FramePhase::EndFrame, &mut ctx);

        ctx.telemetry
            .frame_tick(raw_dt, ctx.time.fixed_alpha, ctx.time.fixed_tick_index);

        let exit_now = *ctx.exit_requested;
        drop(ctx);

        if exit_now {
            self.shutdown_once(el);
            return;
        }

        window.request_redraw();
    }
}