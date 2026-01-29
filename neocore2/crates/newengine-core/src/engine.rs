use crate::error::{EngineError, EngineResult, ModuleStage};
use crate::frame::Frame;
use crate::module::{Bus, Module, ModuleCtx, Resources, Services};
use crate::sched::Scheduler;
use crate::sync::ShutdownToken;

use std::time::{Duration, Instant};

pub struct Engine<E: Send + 'static> {
    fixed_dt: f32,
    services: Box<dyn Services>,
    modules: Vec<Box<dyn Module<E>>>,

    resources: Resources,
    bus: Bus<E>,
    scheduler: Scheduler,

    shutdown: ShutdownToken,
    exit_requested: bool,

    frame_index: u64,
    started: bool,
    last: Instant,
    acc: f32,
}

impl<E: Send + 'static> Engine<E> {
    #[inline]
    pub fn request_exit(&mut self) {
        self.shutdown.request();
        self.exit_requested = true;
    }

    #[inline]
    pub fn shutdown_token(&self) -> ShutdownToken {
        self.shutdown.clone()
    }

    pub fn new(
        fixed_dt_ms: u32,
        services: Box<dyn Services>,
        bus: Bus<E>,
        shutdown: ShutdownToken,
    ) -> EngineResult<Self> {
        let fixed_dt = (fixed_dt_ms as f32 / 1000.0).max(0.001);
        Ok(Self {
            fixed_dt,
            services,
            modules: Vec::new(),
            resources: Resources::default(),
            bus,
            scheduler: Scheduler::new(),
            shutdown,
            exit_requested: false,
            frame_index: 0,
            started: false,
            last: Instant::now(),
            acc: 0.0,
        })
    }

    #[inline]
    pub fn new_default_shutdown(
        fixed_dt_ms: u32,
        services: Box<dyn Services>,
        bus: Bus<E>,
    ) -> EngineResult<Self> {
        Self::new(fixed_dt_ms, services, bus, ShutdownToken::new())
    }

    #[inline]
    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    #[inline]
    pub fn bus(&self) -> &Bus<E> {
        &self.bus
    }

    pub fn register_module(&mut self, mut module: Box<dyn Module<E>>) -> EngineResult<()> {
        self.sync_shutdown_state();

        let mut ctx = ModuleCtx::new(
            self.services.as_ref(),
            &mut self.resources,
            &self.bus,
            &mut self.scheduler,
            &mut self.exit_requested,
        );

        module
            .init(&mut ctx)
            .map_err(|e| EngineError::with_stage(ModuleStage::Init, e))?;

        self.propagate_shutdown_request();
        self.modules.push(module);
        Ok(())
    }

    pub fn start(&mut self) -> EngineResult<()> {
        self.started = true;
        self.last = Instant::now();
        self.sync_shutdown_state();

        let mut modules = std::mem::take(&mut self.modules);

        for m in &mut modules {
            let mut ctx = ModuleCtx::new(
                self.services.as_ref(),
                &mut self.resources,
                &self.bus,
                &mut self.scheduler,
                &mut self.exit_requested,
            );

            m.start(&mut ctx)
                .map_err(|e| EngineError::with_stage(ModuleStage::Start, e))?;

            self.propagate_shutdown_request();

            if self.is_exit_requested() {
                self.modules = modules;
                return Err(EngineError::ExitRequested);
            }
        }

        self.modules = modules;
        Ok(())
    }

    pub fn step(&mut self) -> EngineResult<Frame> {
        self.sync_shutdown_state();
        if self.is_exit_requested() {
            return Err(EngineError::ExitRequested);
        }

        const MAX_FRAME_DT: f32 = 0.25;

        let now = Instant::now();

        if !self.started {
            self.start()?;
            self.last = now;
        }

        let mut dt = (now - self.last).as_secs_f32();
        self.last = now;

        if !dt.is_finite() || dt < 0.0 {
            dt = 0.0;
        }
        dt = dt.min(MAX_FRAME_DT);

        self.acc = (self.acc + dt).min(self.fixed_dt * 8.0);

        let mut modules: Vec<Box<dyn Module<E>>> = std::mem::take(&mut self.modules);

        fn run_stage<E, F>(
            engine: &mut Engine<E>,
            modules: &mut [Box<dyn Module<E>>],
            frame: &Frame,
            stage: ModuleStage,
            mut call: F,
        ) -> EngineResult<()>
        where
            E: Send + 'static,
            F: FnMut(&mut dyn Module<E>, &mut ModuleCtx<'_, E>) -> EngineResult<()>,
        {
            for m in modules.iter_mut() {
                engine.sync_shutdown_state();

                {
                    let mut ctx = ModuleCtx::new(
                        engine.services.as_ref(),
                        &mut engine.resources,
                        &engine.bus,
                        &mut engine.scheduler,
                        &mut engine.exit_requested,
                    );
                    ctx.set_frame(frame);

                    call(m.as_mut(), &mut ctx)
                        .map_err(|e| EngineError::with_stage(stage, e))?;
                }

                engine.propagate_shutdown_request();
                if engine.is_exit_requested() {
                    return Err(EngineError::ExitRequested);
                }
            }
            Ok(())
        }

        let result: EngineResult<Frame> = (|| {
            let mut fixed_steps = 0u32;

            while self.acc >= self.fixed_dt {
                self.acc -= self.fixed_dt;
                fixed_steps = fixed_steps.saturating_add(1);

                let fixed_frame = Frame {
                    frame_index: self.frame_index,
                    dt: self.fixed_dt,
                    fixed_dt: self.fixed_dt,
                    fixed_alpha: 0.0,
                    fixed_steps: 1,
                };

                run_stage(
                    self,
                    modules.as_mut_slice(),
                    &fixed_frame,
                    ModuleStage::FixedUpdate,
                    |m, ctx| m.fixed_update(ctx),
                )?;
            }

            let frame = Frame {
                frame_index: self.frame_index,
                dt,
                fixed_dt: self.fixed_dt,
                fixed_alpha: (self.acc / self.fixed_dt).clamp(0.0, 0.999_999),
                fixed_steps,
            };

            run_stage(self, modules.as_mut_slice(), &frame, ModuleStage::Update, |m, ctx| {
                m.update(ctx)
            })?;

            run_stage(self, modules.as_mut_slice(), &frame, ModuleStage::Render, |m, ctx| {
                m.render(ctx)
            })?;

            self.scheduler.tick(Duration::from_secs_f32(dt));
            self.frame_index = self.frame_index.wrapping_add(1);

            Ok(frame)
        })();

        self.modules = modules;
        result
    }

    pub fn dispatch_external_event(&mut self, event: &dyn std::any::Any) -> EngineResult<()> {
        self.sync_shutdown_state();

        let mut modules: Vec<Box<dyn Module<E>>> = std::mem::take(&mut self.modules);

        let result: EngineResult<()> = (|| {
            for m in modules.iter_mut() {
                self.sync_shutdown_state();

                {
                    let mut ctx = ModuleCtx::new(
                        self.services.as_ref(),
                        &mut self.resources,
                        &self.bus,
                        &mut self.scheduler,
                        &mut self.exit_requested,
                    );

                    m.on_external_event(&mut ctx, event)
                        .map_err(|e| EngineError::with_stage(ModuleStage::ExternalEvent, e))?;
                }

                self.propagate_shutdown_request();
                if self.is_exit_requested() {
                    return Err(EngineError::ExitRequested);
                }
            }
            Ok(())
        })();

        self.modules = modules;
        result
    }

    pub fn shutdown(&mut self) -> EngineResult<()> {
        self.sync_shutdown_state();

        for m in self.modules.iter_mut().rev() {
            let mut ctx = ModuleCtx::new(
                self.services.as_ref(),
                &mut self.resources,
                &self.bus,
                &mut self.scheduler,
                &mut self.exit_requested,
            );

            let _ = m
                .shutdown(&mut ctx)
                .map_err(|e| EngineError::with_stage(ModuleStage::Shutdown, e));
        }

        Ok(())
    }

    #[inline]
    pub fn exit_requested(&self) -> bool {
        self.is_exit_requested()
    }

    #[inline]
    fn is_exit_requested(&self) -> bool {
        self.exit_requested || self.shutdown.is_requested()
    }

    #[inline]
    fn sync_shutdown_state(&mut self) {
        if self.shutdown.is_requested() {
            self.exit_requested = true;
        }
    }

    #[inline]
    fn propagate_shutdown_request(&mut self) {
        if self.exit_requested {
            self.shutdown.request();
        }
    }
}
