use super::module_slot::{ModuleSlot, ModuleState};
use super::{Engine, ModuleFaultTolerance};

use crate::error::{EngineError, EngineResult, ModuleStage};
use crate::lifecycle_events::EngineLifecycleEvent;
use crate::module::ModuleCtx;

use newengine_math::collections_prelude::{ne_hash_map_with_capacity, NeHashMap as HashMap, NeVecDeque as VecDeque};

impl<E: Send + 'static> Engine<E> {
    pub fn start(&mut self) -> EngineResult<()> {
        match self.module_fault_tolerance {
            ModuleFaultTolerance::Strict => self.start_strict(),
            ModuleFaultTolerance::Resilient => self.start_resilient(),
        }
    }

    fn start_strict(&mut self) -> EngineResult<()> {
        self.started = true;
        self.last = std::time::Instant::now();
        self.sync_shutdown_state();

        if self.is_exit_requested() {
            return Err(EngineError::ExitRequested);
        }

        self.validate_api_contracts_strict()?;

        let n = self.modules.len();

        let mut id_to_index = ne_hash_map_with_capacity::<&'static str, usize>(n);
        for (i, s) in self.modules.iter().enumerate() {
            let id = s.id();
            if id_to_index.insert(id, i).is_some() {
                return Err(EngineError::Other(format!("duplicate module id: {id}")));
            }
        }

        let mut indegree = vec![0usize; n];
        let mut rev_edges: Vec<Vec<usize>> = vec![Vec::new(); n];

        for (i, s) in self.modules.iter().enumerate() {
            let m = s.module.as_ref();
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

        let mut sorted: Vec<ModuleSlot<E>> = Vec::with_capacity(n);
        let mut old = std::mem::take(&mut self.modules);
        let mut slots: Vec<Option<ModuleSlot<E>>> = old.drain(..).map(Some).collect();

        for idx in order {
            let s = slots[idx].take().expect("module slot already moved");
            sorted.push(s);
        }

        #[inline]
        fn shutdown_modules<E: Send + 'static>(engine: &mut Engine<E>, slots: &mut [ModuleSlot<E>]) {
            for s in slots.iter_mut().rev() {
                if s.shutdown_called {
                    continue;
                }
                let mut ctx = ModuleCtx::new(
                    engine.services.as_ref(),
                    &mut engine.resources,
                    &engine.bus,
                    &engine.events,
                    &mut engine.scheduler,
                    &mut engine.exit_requested,
                );
                let _ = s.module.shutdown(&mut ctx);
                s.shutdown_called = true;
                s.state = ModuleState::Disabled;
            }
        }

        let mut initialized = 0usize;

        for i in 0..sorted.len() {
            self.sync_shutdown_state();

            let init_result = {
                let s = &mut sorted[i];
                let mut ctx = ModuleCtx::new(
                    self.services.as_ref(),
                    &mut self.resources,
                    &self.bus,
                    &self.events,
                    &mut self.scheduler,
                    &mut self.exit_requested,
                );
                s.module.init(&mut ctx)
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

        self.modules = sorted;
        self.refresh_readiness_snapshot();
        self.log_startup_graph_snapshot("after-init");
        self.start_modules_ready_by_graph("initial")?;

        if !self.plugins_loaded && !self.engine_plugins_loaded {
            self.try_load_plugins_once()?;
        }
        self.log_plugins_diagnostics("after module init");
        self.plugins_start_all()?;
        self.dispatch_startup_readiness_events("engine.start.strict")?;

        Ok(())
    }

    fn start_resilient(&mut self) -> EngineResult<()> {
        self.started = true;
        self.last = std::time::Instant::now();
        self.sync_shutdown_state();

        if self.is_exit_requested() {
            return Err(EngineError::ExitRequested);
        }

        let n = self.modules.len();

        let mut id_to_index = ne_hash_map_with_capacity::<&'static str, usize>(n);
        for (i, s) in self.modules.iter().enumerate() {
            let id = s.id();
            if id_to_index.insert(id, i).is_some() {
                log::error!("engine: duplicate module id: {}", id);
            }
        }

        let mut enabled = vec![true; n];
        let mut reasons: Vec<Option<String>> = vec![None; n];

        loop {
            let mut changed = false;

            for (i, s) in self.modules.iter().enumerate() {
                if !enabled[i] {
                    continue;
                }
                let m = s.module.as_ref();
                for &dep in m.dependencies() {
                    let Some(&dep_i) = id_to_index.get(dep) else {
                        enabled[i] = false;
                        reasons[i] = Some(format!("missing dependency: {} -> {}", m.id(), dep));
                        changed = true;
                        break;
                    };
                    if !enabled[dep_i] {
                        enabled[i] = false;
                        reasons[i] = Some(format!("dependency disabled: {} -> {}", m.id(), dep));
                        changed = true;
                        break;
                    }
                }
            }

            use crate::module::ApiVersion;
            let mut provided: HashMap<&'static str, ApiVersion> = HashMap::default();
            let mut provider: HashMap<&'static str, &'static str> = HashMap::default();

            for (i, s) in self.modules.iter().enumerate() {
                if !enabled[i] {
                    continue;
                }
                let m = s.module.as_ref();
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

            for (i, s) in self.modules.iter().enumerate() {
                if !enabled[i] {
                    continue;
                }
                let m = s.module.as_ref();
                for r in m.requires().iter() {
                    let Some(have) = provided.get(r.id) else {
                        enabled[i] = false;
                        reasons[i] = Some(format!(
                            "requires API '{}' >= {}.{}.{} but it is not provided",
                            r.id, r.min_version.major, r.min_version.minor, r.min_version.patch
                        ));
                        changed = true;
                        break;
                    };

                    if *have < r.min_version {
                        let prov = provider.get(r.id).copied().unwrap_or("<unknown>");
                        enabled[i] = false;
                        reasons[i] = Some(format!(
                            "requires API '{}' >= {}.{}.{} but provider '{}' offers {}.{}.{}",
                            r.id,
                            r.min_version.major,
                            r.min_version.minor,
                            r.min_version.patch,
                            prov,
                            have.major,
                            have.minor,
                            have.patch,
                        ));
                        changed = true;
                        break;
                    }
                }
            }

            if !changed {
                break;
            }
        }

        let mut indegree = vec![0usize; n];
        let mut rev_edges: Vec<Vec<usize>> = vec![Vec::new(); n];

        for (i, s) in self.modules.iter().enumerate() {
            if !enabled[i] {
                continue;
            }
            let m = s.module.as_ref();
            for &dep in m.dependencies() {
                let Some(&dep_i) = id_to_index.get(dep) else {
                    continue;
                };
                if !enabled[dep_i] {
                    continue;
                }
                indegree[i] += 1;
                rev_edges[dep_i].push(i);
            }
        }

        let mut q: VecDeque<usize> = VecDeque::new();
        for i in 0..n {
            if enabled[i] && indegree[i] == 0 {
                q.push_back(i);
            }
        }

        let mut order: Vec<usize> = Vec::with_capacity(n);
        while let Some(i) = q.pop_front() {
            order.push(i);
            for &to in rev_edges[i].iter() {
                indegree[to] = indegree[to].saturating_sub(1);
                if enabled[to] && indegree[to] == 0 {
                    q.push_back(to);
                }
            }
        }

        for i in 0..n {
            if enabled[i] && indegree[i] != 0 {
                enabled[i] = false;
                reasons[i] = Some("dependency cycle detected".to_string());
            }
        }

        let mut new_slots: Vec<ModuleSlot<E>> = Vec::with_capacity(n);

        let mut opt: Vec<Option<ModuleSlot<E>>> =
            std::mem::take(&mut self.modules).into_iter().map(Some).collect();

        let mut moved = vec![false; n];
        for &idx in order.iter() {
            if idx >= opt.len() || !enabled[idx] {
                continue;
            }
            moved[idx] = true;
            let mut s = opt[idx].take().expect("slot already moved");
            s.state = ModuleState::Pending;
            new_slots.push(s);
        }

        for i in 0..opt.len() {
            if moved[i] {
                continue;
            }
            let mut s = opt[i].take().expect("slot already moved");
            let reason = reasons[i].take().unwrap_or_else(|| "disabled".to_string());
            s.disable(reason.clone());
            log::warn!("engine: module disabled: {} ({})", s.id(), reason);
            new_slots.push(s);
        }

        for i in 0..new_slots.len() {
            self.sync_shutdown_state();
            if self.is_exit_requested() {
                break;
            }

            if new_slots[i].state != ModuleState::Pending {
                continue;
            }

            let module_id = new_slots[i].id();

            let init_res = {
                let mut ctx = ModuleCtx::new(
                    self.services.as_ref(),
                    &mut self.resources,
                    &self.bus,
                    &self.events,
                    &mut self.scheduler,
                    &mut self.exit_requested,
                );
                new_slots[i].module.init(&mut ctx)
            };

            if let Err(err) = init_res {
                let reason = format!("init failed: {err}");
                log::error!("engine: module init failed: {} ({})", module_id, reason);
                new_slots[i].disable(reason);
                self.shutdown_slot(&mut new_slots[i]);
                continue;
            }
        }

        self.modules = new_slots;
        self.refresh_readiness_snapshot();
        self.log_startup_graph_snapshot("after-init");
        self.start_modules_ready_by_graph("initial")?;

        if !self.plugins_loaded && !self.engine_plugins_loaded {
            self.try_load_plugins_once()?;
        }
        self.log_plugins_diagnostics("after module init");
        self.plugins_start_all()?;
        self.dispatch_startup_readiness_events("engine.start.resilient")?;

        Ok(())
    }


    #[inline]
    fn dispatch_startup_readiness_events(&mut self, origin: &'static str) -> EngineResult<()> {
        let plugin_count = self.plugins.snapshot().len();

        if self.plugins_loaded || self.engine_plugins_loaded {
            self.dispatch(EngineLifecycleEvent::EnginePluginsReady {
                loaded_count: plugin_count,
                origin,
            })?;
        }

        let module_count = self
            .modules
            .iter()
            .filter(|s| s.state == ModuleState::Running)
            .count();
        self.dispatch(EngineLifecycleEvent::EngineStartCompleted {
            module_count,
            plugin_count,
        })?;

        self.refresh_readiness_snapshot();
        self.log_startup_graph_snapshot("after-startup-dispatch");
        Ok(())
    }

    pub(crate) fn start_modules_ready_by_graph(&mut self, origin: &'static str) -> EngineResult<usize> {
        let mut started = 0usize;

        for i in 0..self.modules.len() {
            self.sync_shutdown_state();
            if self.is_exit_requested() {
                return Err(EngineError::ExitRequested);
            }

            if self.modules[i].state != ModuleState::Pending {
                continue;
            }

            let module_id = self.modules[i].id();
            let requirements = self.modules[i].module.startup_requires();
            if !self.startup_graph.all_satisfied(requirements) {
                let missing = self
                    .startup_graph
                    .missing(requirements)
                    .map(|key| key.as_str())
                    .collect::<Vec<_>>()
                    .join(",");
                log::info!(
                    "startup graph: module gated module='{}' origin='{}' missing='{}'",
                    module_id,
                    origin,
                    if missing.is_empty() { "-" } else { missing.as_str() },
                );
                continue;
            }

            log::info!(
                "startup graph: starting module module='{}' origin='{}' requires='{}'",
                module_id,
                origin,
                super::startup_graph::readiness_csv(requirements),
            );

            let start_res = {
                let mut ctx = ModuleCtx::new(
                    self.services.as_ref(),
                    &mut self.resources,
                    &self.bus,
                    &self.events,
                    &mut self.scheduler,
                    &mut self.exit_requested,
                );
                self.modules[i].module.start(&mut ctx)
            };

            if let Err(err) = start_res {
                match self.module_fault_tolerance {
                    ModuleFaultTolerance::Strict => {
                        self.exit_requested = true;
                        self.shutdown.request();
                        return Err(EngineError::with_module_stage(
                            module_id,
                            ModuleStage::Start,
                            err,
                        ));
                    }
                    ModuleFaultTolerance::Resilient => {
                        let reason = format!("start failed: {err}");
                        log::error!("engine: module start failed: {} ({})", module_id, reason);
                        self.modules[i].disable(reason);
                        self.shutdown_slot_by_index(i);
                        continue;
                    }
                }
            }

            self.modules[i].state = ModuleState::Running;
            started += 1;
        }

        if started > 0 {
            log::info!(
                "startup graph: started modules origin='{}' count={} satisfied='{}'",
                origin,
                started,
                self.startup_graph.satisfied_csv(),
            );
        }

        self.refresh_readiness_snapshot();
        self.log_startup_graph_snapshot(origin);
        Ok(started)
    }

    #[inline]
    fn shutdown_slot_by_index(&mut self, index: usize) {
        if self.modules[index].shutdown_called {
            return;
        }
        let mut ctx = ModuleCtx::new(
            self.services.as_ref(),
            &mut self.resources,
            &self.bus,
            &self.events,
            &mut self.scheduler,
            &mut self.exit_requested,
        );
        let _ = self.modules[index].module.shutdown(&mut ctx);
        self.modules[index].shutdown_called = true;
        self.modules[index].state = ModuleState::Disabled;
    }

    #[inline]
    fn shutdown_slot(&mut self, s: &mut ModuleSlot<E>) {
        if s.shutdown_called {
            return;
        }
        let mut ctx = ModuleCtx::new(
            self.services.as_ref(),
            &mut self.resources,
            &self.bus,
            &self.events,
            &mut self.scheduler,
            &mut self.exit_requested,
        );
        let _ = s.module.shutdown(&mut ctx);
        s.shutdown_called = true;
        s.state = ModuleState::Disabled;
    }
}