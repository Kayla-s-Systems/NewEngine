use crate::error::{EngineError, EngineResult, ModuleStage};
use crate::events::EventHub;
use crate::frame::Frame;
use crate::module::{ApiVersion, Bus, Module, ModuleCtx, Resources, Services};

use crate::sched::Scheduler;
use crate::sync::ShutdownToken;

use std::any::Any;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

pub struct Engine<E: Send + 'static> {
    fixed_dt: f32,
    services: Box<dyn Services>,
    modules: Vec<Box<dyn Module<E>>>,
    module_ids: HashSet<&'static str>,

    resources: Resources,
    bus: Bus<E>,
    events: EventHub,
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
        self.exit_requested = true;
        self.shutdown.request();
    }

    #[inline]
    pub fn shutdown_token(&self) -> ShutdownToken {
        self.shutdown.clone()
    }

    #[inline]
    pub fn events(&self) -> &EventHub {
        &self.events
    }

    pub fn emit<T>(&self, event: T) -> EngineResult<()>
    where
        T: Any + Send + 'static + std::marker::Sync,
    {
        self.events.publish(event)
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
            module_ids: HashSet::new(),
            resources: Resources::default(),
            bus,
            events: EventHub::new(),
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
    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    #[inline]
    pub fn bus(&self) -> &Bus<E> {
        &self.bus
    }

    pub fn register_module(&mut self, module: Box<dyn Module<E>>) -> EngineResult<()> {
        self.sync_shutdown_state();

        let id = module.id();
        if self.module_ids.contains(id) {
            return Err(EngineError::Other(format!("module already registered: {id}")));
        }

        self.modules.push(module);
        self.module_ids.insert(id);
        Ok(())
    }

    pub fn start(&mut self) -> EngineResult<()> {
        self.started = true;
        self.last = Instant::now();
        self.sync_shutdown_state();

        self.validate_api_contracts()?;

        let n = self.modules.len();

        let mut id_to_index: HashMap<&'static str, usize> = HashMap::with_capacity(n);
        for (i, m) in self.modules.iter().enumerate() {
            let id = m.id();
            if id_to_index.insert(id, i).is_some() {
                return Err(EngineError::Other(format!("duplicate module id: {id}")));
            }
        }

        let mut indegree = vec![0usize; n];
        let mut rev_edges: Vec<Vec<usize>> = vec![Vec::new(); n];

        for (i, m) in self.modules.iter().enumerate() {
            for &dep in m.dependencies() {
                let Some(&dep_i) = id_to_index.get(dep) else {
                    return Err(EngineError::Other(format!(
                        "module dependency missing: {} -> {dep}",
                        m.id()
                    )));
                };
                indegree[i] += 1;
                rev_edges[dep_i].push(i);
            }
        }

        let mut q: VecDeque<usize> = VecDeque::new();
        for i in 0..n {
            if indegree[i] == 0 {
                q.push_back(i);
            }
        }

        let mut order: Vec<usize> = Vec::with_capacity(n);
        while let Some(i) = q.pop_front() {
            order.push(i);
            for &to in rev_edges[i].iter() {
                indegree[to] = indegree[to].saturating_sub(1);
                if indegree[to] == 0 {
                    q.push_back(to);
                }
            }
        }

        if order.len() != n {
            let mut cyclic = Vec::new();
            for (i, deg) in indegree.iter().enumerate() {
                if *deg != 0 {
                    cyclic.push(self.modules[i].id());
                }
            }
            return Err(EngineError::Other(format!(
                "module dependency cycle detected among: {:?}",
                cyclic
            )));
        }

        let mut sorted: Vec<Box<dyn Module<E>>> = Vec::with_capacity(n);
        let mut old = std::mem::take(&mut self.modules);
        let mut slots: Vec<Option<Box<dyn Module<E>>>> = old.drain(..).map(Some).collect();

        for idx in order {
            let m = slots[idx].take().expect("module slot already moved");
            sorted.push(m);
        }

        #[inline]
        fn shutdown_modules<E: Send + 'static>(
            engine: &mut Engine<E>,
            modules: &mut [Box<dyn Module<E>>],
        ) {
            for m in modules.iter_mut().rev() {
                let mut ctx = ModuleCtx::new(
                    engine.services.as_ref(),
                    &mut engine.resources,
                    &engine.bus,
                    &engine.events,
                    &mut engine.scheduler,
                    &mut engine.exit_requested,
                );
                let _ = m.shutdown(&mut ctx);
            }
        }

        let mut initialized = 0usize;

        for i in 0..sorted.len() {
            self.sync_shutdown_state();

            let init_result = {
                let m = &mut sorted[i];
                let mut ctx = ModuleCtx::new(
                    self.services.as_ref(),
                    &mut self.resources,
                    &self.bus,
                    &self.events,
                    &mut self.scheduler,
                    &mut self.exit_requested,
                );
                m.init(&mut ctx)
            };

            if let Err(err) = init_result {
                shutdown_modules(self, &mut sorted[..initialized]);
                return Err(EngineError::with_module_stage(sorted[i].id(), ModuleStage::Init, err));
            }

            initialized = initialized.saturating_add(1);

            self.propagate_shutdown_request();
            if self.is_exit_requested() {
                shutdown_modules(self, &mut sorted[..initialized]);
                self.modules = sorted;
                self.module_ids = self.modules.iter().map(|mm| mm.id()).collect();
                return Err(EngineError::ExitRequested);
            }
        }

        for i in 0..sorted.len() {
            self.sync_shutdown_state();

            let start_result = {
                let m = &mut sorted[i];
                let mut ctx = ModuleCtx::new(
                    self.services.as_ref(),
                    &mut self.resources,
                    &self.bus,
                    &self.events,
                    &mut self.scheduler,
                    &mut self.exit_requested,
                );
                m.start(&mut ctx)
            };

            if let Err(err) = start_result {
                shutdown_modules(self, &mut sorted[..initialized]);
                return Err(EngineError::with_module_stage(sorted[i].id(), ModuleStage::Start, err));
            }

            self.propagate_shutdown_request();
            if self.is_exit_requested() {
                shutdown_modules(self, &mut sorted[..initialized]);
                self.modules = sorted;
                self.module_ids = self.modules.iter().map(|mm| mm.id()).collect();
                return Err(EngineError::ExitRequested);
            }
        }

        self.modules = sorted;
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

            self.run_stage(&fixed_frame, ModuleStage::FixedUpdate, |m, ctx| m.fixed_update(ctx))?;
        }

        let frame = Frame {
            frame_index: self.frame_index,
            dt,
            fixed_dt: self.fixed_dt,
            fixed_alpha: (self.acc / self.fixed_dt).clamp(0.0, 0.999_999),
            fixed_steps,
        };

        self.run_stage(&frame, ModuleStage::Update, |m, ctx| m.update(ctx))?;
        self.run_stage(&frame, ModuleStage::Render, |m, ctx| m.render(ctx))?;

        self.scheduler.tick(Duration::from_secs_f32(dt));
        self.frame_index = self.frame_index.wrapping_add(1);

        Ok(frame)
    }

    #[deprecated(note = "Use Engine::emit(...) + EventHub subscriptions instead of synchronous fan-out")]
    pub fn dispatch_external_event(&mut self, event: &dyn Any) -> EngineResult<()> {
        self.sync_shutdown_state();
        if self.is_exit_requested() {
            return Err(EngineError::ExitRequested);
        }

        let services = self.services.as_ref();
        let bus = &self.bus;
        let events = &self.events;
        let shutdown = &self.shutdown;

        let resources = &mut self.resources;
        let scheduler = &mut self.scheduler;
        let exit_requested = &mut self.exit_requested;

        for m in self.modules.iter_mut() {
            if shutdown.is_requested() {
                *exit_requested = true;
            }
            if *exit_requested {
                shutdown.request();
                return Err(EngineError::ExitRequested);
            }

            let module_id = m.id();
            let mut ctx = ModuleCtx::new(services, resources, bus, events, scheduler, exit_requested);

            #[allow(deprecated)]
            m.on_external_event(&mut ctx, event).map_err(|e| {
                EngineError::with_module_stage(module_id, ModuleStage::ExternalEvent, e)
            })?;

            if *exit_requested {
                shutdown.request();
                return Err(EngineError::ExitRequested);
            }
        }

        Ok(())
    }

    pub fn shutdown(&mut self) -> EngineResult<()> {
        self.sync_shutdown_state();

        for m in self.modules.iter_mut().rev() {
            let module_id = m.id();

            let mut ctx = ModuleCtx::new(
                self.services.as_ref(),
                &mut self.resources,
                &self.bus,
                &self.events,
                &mut self.scheduler,
                &mut self.exit_requested,
            );

            let _ = m
                .shutdown(&mut ctx)
                .map_err(|e| EngineError::with_module_stage(module_id, ModuleStage::Shutdown, e));
        }

        Ok(())
    }

    #[inline]
    fn run_stage<F>(&mut self, frame: &Frame, stage: ModuleStage, mut call: F) -> EngineResult<()>
    where
        F: FnMut(&mut dyn Module<E>, &mut ModuleCtx<'_, E>) -> EngineResult<()>,
    {
        self.sync_shutdown_state();
        if self.is_exit_requested() {
            return Err(EngineError::ExitRequested);
        }

        let services = self.services.as_ref();
        let bus = &self.bus;
        let events = &self.events;
        let shutdown = &self.shutdown;

        let resources = &mut self.resources;
        let scheduler = &mut self.scheduler;
        let exit_requested = &mut self.exit_requested;

        for m in self.modules.iter_mut() {
            if shutdown.is_requested() {
                *exit_requested = true;
            }
            if *exit_requested {
                shutdown.request();
                return Err(EngineError::ExitRequested);
            }

            let module_id = m.id();

            let mut ctx = ModuleCtx::new(services, resources, bus, events, scheduler, exit_requested);
            ctx.set_frame(frame);

            call(m.as_mut(), &mut ctx)
                .map_err(|e| EngineError::with_module_stage(module_id, stage, e))?;

            if *exit_requested {
                shutdown.request();
                return Err(EngineError::ExitRequested);
            }
        }

        Ok(())
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

    fn validate_api_contracts(&self) -> EngineResult<()> {
        let mut provided: HashMap<&'static str, ApiVersion> = HashMap::new();
        let mut provider: HashMap<&'static str, &'static str> = HashMap::new();

        for m in self.modules.iter() {
            for p in m.provides().iter() {
                match provided.get(p.id) {
                    Some(v) if *v >= p.version => {}
                    _ => {
                        provided.insert(p.id, p.version);
                        provider.insert(p.id, m.id());
                    }
                }
            }
        }

        for m in self.modules.iter() {
            for r in m.requires().iter() {
                let Some(have) = provided.get(r.id) else {
                    return Err(EngineError::Other(format!(
                        "module '{}' requires API '{}' >= {}.{}.{} but it is not provided",
                        m.id(),
                        r.id,
                        r.min_version.major,
                        r.min_version.minor,
                        r.min_version.patch,
                    )));
                };

                if *have < r.min_version {
                    let prov = provider.get(r.id).copied().unwrap_or("<unknown>");
                    return Err(EngineError::Other(format!(
                        "module '{}' requires API '{}' >= {}.{}.{} but provider '{}' offers {}.{}.{}",
                        m.id(),
                        r.id,
                        r.min_version.major,
                        r.min_version.minor,
                        r.min_version.patch,
                        prov,
                        have.major,
                        have.minor,
                        have.patch,
                    )));
                }
            }
        }

        Ok(())
    }
}