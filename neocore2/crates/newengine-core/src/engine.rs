#![forbid(unsafe_op_in_unsafe_fn)]

use crate::error::{EngineError, EngineResult, ModuleStage};
use crate::events::EventHub;
use crate::frame::Frame;
use crate::module::{ApiVersion, Bus, Module, ModuleCtx, Resources, Services};
use crate::plugins::PluginsSnapshot;
use crate::plugins::{default_host_api, init_host_context, PluginManager};
use crate::sched::Scheduler;
use crate::sync::ShutdownToken;

use std::any::Any;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub fixed_dt_ms: u32,
    pub plugins_dir: Option<PathBuf>,

    /// Controls how the engine reacts to panics inside module callbacks.
    ///
    /// - When `true` (default), the engine converts panics to `EngineError` and requests shutdown.
    /// - When `false`, panics unwind normally (useful for debugging).
    pub catch_panics: bool,
}

impl EngineConfig {
    #[inline]
    pub fn new(fixed_dt_ms: u32) -> Self {
        Self {
            fixed_dt_ms,
            plugins_dir: None,
            catch_panics: true,
        }
    }

    #[inline]
    pub fn with_plugins_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.plugins_dir = dir;
        self
    }

    #[inline]
    pub fn with_catch_panics(mut self, enabled: bool) -> Self {
        self.catch_panics = enabled;
        self
    }
}

pub struct Engine<E: Send + 'static> {
    fixed_dt: f32,
    catch_panics: bool,
    services: Box<dyn Services>,
    modules: Vec<Box<dyn Module<E>>>,
    module_ids: HashSet<&'static str>,

    pub resources: Resources,
    bus: Bus<E>,

    events: EventHub,
    scheduler: Scheduler,

    plugins: PluginManager,
    plugins_loaded: bool,
    plugins_dir: Option<PathBuf>,

    shutdown: ShutdownToken,
    exit_requested: bool,

    frame_index: u64,
    fixed_tick: u64,
    started: bool,
    last: Instant,
    acc: f32,
}

#[derive(Copy, Clone, Debug)]
struct Elapsed {
    value: u128,
    unit: &'static str,
}

impl Elapsed {
    #[inline]
    fn from_duration(d: Duration) -> Self {
        let us = d.as_micros();
        if us < 1000 {
            Self { value: us, unit: "us" }
        } else {
            Self {
                value: d.as_millis(),
                unit: "ms",
            }
        }
    }
}

impl fmt::Display for Elapsed {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "elapsed_{}={}", self.unit, self.value)
    }
}

impl<E: Send + 'static> Engine<E> {
    #[inline]
    fn panic_message(payload: Box<dyn Any + Send>) -> String {
        if let Some(s) = payload.downcast_ref::<&'static str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic payload>".to_string()
        }
    }

    #[inline]
    pub fn request_exit(&mut self) -> EngineResult<()> {
        self.exit_requested = true;
        self.shutdown.request();
        Ok(())
    }

    /// Loads plugins once (idempotent).
    ///
    /// The engine core never hardcodes plugin categories (assets, input, render, etc.).
    /// Any capability registration and secondary loading (e.g. importers) is owned by plugins.
    #[inline]
    pub fn load_plugins_once(&mut self) -> EngineResult<()> {
        self.try_load_plugins_once()
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
        T: Any + Send + 'static + Sync,
    {
        self.events.publish(event)
    }

    pub fn new(
        fixed_dt_ms: u32,
        services: Box<dyn Services>,
        bus: Bus<E>,
        shutdown: ShutdownToken,
    ) -> EngineResult<Self> {
        let config = EngineConfig::new(fixed_dt_ms);
        Self::new_with_config(config, services, bus, shutdown)
    }

    pub fn new_with_config(
        config: EngineConfig,
        services: Box<dyn Services>,
        bus: Bus<E>,
        shutdown: ShutdownToken,
    ) -> EngineResult<Self> {
        let fixed_dt = (config.fixed_dt_ms as f32 / 1000.0).max(0.001);

        let resources = Resources::default();

        init_host_context();

        Ok(Self {
            fixed_dt,
            catch_panics: config.catch_panics,
            services,
            modules: Vec::new(),
            module_ids: HashSet::new(),

            resources,
            bus,
            events: EventHub::new(),
            scheduler: Scheduler::new(),

            plugins: PluginManager::new(),
            plugins_loaded: false,
            plugins_dir: config.plugins_dir,

            shutdown,
            exit_requested: false,

            frame_index: 0,
            fixed_tick: 0,
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

    #[inline]
    fn elapsed_since(t0: Instant) -> Elapsed {
        Elapsed::from_duration(t0.elapsed())
    }

    #[inline]
    fn log_phase_begin(scope: &'static str, phase: &'static str, count: Option<usize>) {
        match count {
            Some(n) => log::info!("{scope}: starting (phase={phase} count={n})"),
            None => log::info!("{scope}: starting (phase={phase})"),
        }
    }

    #[inline]
    fn log_phase_ok(scope: &'static str, phase: &'static str, count: Option<usize>, elapsed: Elapsed) {
        match count {
            Some(n) => log::info!("{scope}: done (phase={phase} count={n} {elapsed})"),
            None => log::info!("{scope}: done (phase={phase} {elapsed})"),
        }
    }

    #[allow(dead_code)]
    #[inline]
    fn phase_err(phase: &'static str, elapsed: Elapsed, e: impl fmt::Display) -> EngineError {
        EngineError::Other(format!("plugins: failed (phase={phase} {elapsed}): {e}"))
    }

    fn try_load_plugins_once(&mut self) -> EngineResult<()> {
        if self.plugins_loaded {
            log::debug!("plugins: load skipped (already loaded)");
            return Ok(());
        }

        let phase = "load";
        Self::log_phase_begin("plugins", phase, None);
        let t0 = Instant::now();

        let host = default_host_api();

        let load_result = if let Some(dir) = self.plugins_dir.as_deref() {
            self.plugins.load_from_dir(dir, host)
        } else {
            self.plugins.load_default(host)
        };

        if let Err(e) = load_result {
            log::warn!(
                "plugins: non-fatal load error (phase={} {}): {}",
                phase,
                Self::elapsed_since(t0),
                e
            );
        }

        self.plugins_loaded = true;

        let loaded = self.plugins.snapshot().len();
        Self::log_phase_ok("plugins", phase, Some(loaded), Self::elapsed_since(t0));

        Ok(())
    }

    fn log_plugins_diagnostics(&self, tag: &'static str) {
        let list = self.plugins.snapshot();
        log::info!("plugins: diagnostics tag='{}' loaded={}", tag, list.len());

        for (i, p) in list.iter().enumerate() {
            log::info!(
                "plugins: diag [{:02}/{:02}] id='{}' ver='{}' state='{}'",
                i.saturating_add(1),
                list.len().max(1),
                p.id,
                p.version,
                p.state
            );
        }

        if log::log_enabled!(log::Level::Debug) {
            for p in list.iter() {
                log::debug!(
                    "plugins: diag.debug id='{}' ver='{}' kind={:?} caps={} path='{}'",
                    p.id,
                    p.version,
                    p.kind,
                    p.capabilities.len(),
                    p.path.display()
                );
            }
        }
    }

    pub fn start(&mut self) -> EngineResult<()> {
        self.started = true;
        self.last = Instant::now();
        self.sync_shutdown_state();

        if self.is_exit_requested() {
            return Err(EngineError::ExitRequested);
        }

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
                return Err(EngineError::with_module_stage(
                    sorted[i].id(),
                    ModuleStage::Init,
                    err,
                ));
            }

            initialized += 1;

            self.sync_shutdown_state();
            if self.is_exit_requested() {
                shutdown_modules(self, &mut sorted[..initialized]);
                return Err(EngineError::ExitRequested);
            }
        }

        // Start stage (after successful init of all modules).
        let mut started = 0usize;
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
                return Err(EngineError::with_module_stage(
                    sorted[i].id(),
                    ModuleStage::Start,
                    err,
                ));
            }

            started += 1;

            self.sync_shutdown_state();
            if self.is_exit_requested() {
                shutdown_modules(self, &mut sorted[..initialized]);
                return Err(EngineError::ExitRequested);
            }
        }

        self.modules = sorted;

        self.try_load_plugins_once()?;
        self.log_plugins_diagnostics("after module init");

        if let Err(e) = self.plugins.start_all() {
            return Err(EngineError::Other(format!("plugins: start failed: {e}")));
        }

        Ok(())
    }

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

        // Expose engine/plugin telemetry to modules and UI.
        // This is a snapshot (no interior mutability), so UI can read without synchronization.
        self.resources.insert(PluginsSnapshot {
            plugins: self.plugins.snapshot(),
        });

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

    #[deprecated(
        note = "Use Engine::emit(...) + EventHub subscriptions instead of synchronous fan-out"
    )]
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

            let result = if self.catch_panics {
                #[allow(deprecated)]
                match panic::catch_unwind(AssertUnwindSafe(|| m.on_external_event(&mut ctx, event))) {
                    Ok(r) => r,
                    Err(payload) => {
                        *exit_requested = true;
                        Err(EngineError::Other(format!(
                            "panic in module callback (module='{module_id}' stage={:?} msg='{}')",
                            ModuleStage::ExternalEvent,
                            Self::panic_message(payload)
                        )))
                    }
                }
            } else {
                #[allow(deprecated)]
                m.on_external_event(&mut ctx, event)
            };

            result.map_err(|e| {
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

        self.plugins.shutdown();

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

            let result = if self.catch_panics {
                match panic::catch_unwind(AssertUnwindSafe(|| call(m.as_mut(), &mut ctx))) {
                    Ok(r) => r,
                    Err(payload) => {
                        *exit_requested = true;
                        Err(EngineError::Other(format!(
                            "panic in module callback (module='{module_id}' stage={stage:?} msg='{}')",
                            Self::panic_message(payload)
                        )))
                    }
                }
            } else {
                call(m.as_mut(), &mut ctx)
            };

            result.map_err(|e| EngineError::with_module_stage(module_id, stage, e))?;

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