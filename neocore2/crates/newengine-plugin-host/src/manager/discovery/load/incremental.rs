use super::*;

impl PluginManager {
    #[inline]
    pub fn load_engine_default_incremental_step(
        &mut self,
        host: HostApiV1,
        strict: bool,
    ) -> Result<IncrementalLoadOutcome, PluginLoadError> {
        let dir = default_plugins_dir()?;
        self.load_engine_from_dir_incremental_step(&dir, host, strict)
    }

    #[inline]
    pub fn load_engine_from_dir_incremental_step(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
    ) -> Result<IncrementalLoadOutcome, PluginLoadError> {
        self.load_engine_from_dir_incremental_step_with_origin(
            dir,
            host,
            strict,
            PluginLoadOrigin::Auto,
        )
    }

    pub fn load_engine_from_dir_incremental_step_with_origin(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
        load_origin: PluginLoadOrigin,
    ) -> Result<IncrementalLoadOutcome, PluginLoadError> {
        self.load_from_dir_incremental_step(
            dir,
            host,
            strict,
            LoadPhaseFilter::BootstrapAndEngine,
            load_origin,
        )
    }

    fn load_from_dir_incremental_step(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
        filter: LoadPhaseFilter,
        load_origin: PluginLoadOrigin,
    ) -> Result<IncrementalLoadOutcome, PluginLoadError> {
        if self.incremental_load.is_none() {
            let (graph, graph_is_new) = self.ensure_discovery_graph(dir)?;
            self.begin_incremental_load_from_graph(
                graph,
                graph_is_new,
                filter,
                strict,
                load_origin,
            );
        }

        let Some(mut state) = self.incremental_load.take() else {
            unreachable!("incremental load state must exist after initialization");
        };

        let pending_total = state.pending.len();
        let mut current_path = None;

        if state.next_index < pending_total {
            let path = state.pending[state.next_index].clone();
            current_path = Some(path.clone());
            if let Err(e) = self.load_one_with_origin(&path, host.clone(), state.load_origin) {
                newengine_ulog_api::ulog::warn!(
                    "plugins: failed to load '{}': {}",
                    display_clean(&path),
                    e
                );
                state.load_errors.push(e);
            }
            state.next_index = state.next_index.saturating_add(1);
        }

        let loaded_total = self.loaded.len();
        let loaded_this_phase = loaded_total.saturating_sub(state.loaded_ids_before_len);
        let completed = state.next_index.min(pending_total);
        let load_errors = state.load_errors.len();

        if completed < pending_total {
            let outcome = IncrementalLoadOutcome::running(
                loaded_total,
                loaded_this_phase,
                pending_total,
                completed,
                load_errors,
                current_path,
            );
            self.incremental_load = Some(state);
            return Ok(outcome);
        }

        if state.graph_is_new {
            emit_discovery_logs(&state.graph);
        }

        emit_selection_table(&state.graph, &state.selection, state.filter);

        emit_boxed_kv(
            &format!(
                "PluginDiscovery :: Incremental Load Result [{}]",
                state.filter.label()
            ),
            &[
                ("loaded_total", loaded_total.to_string()),
                ("loaded_this_phase", loaded_this_phase.to_string()),
                ("load_errors", load_errors.to_string()),
                ("scan_errors", state.graph.scan_errors.len().to_string()),
            ],
        );

        self.validate_required_capabilities();

        if newengine_ulog_api::ulog::debug_enabled() {
            for p in &self.loaded {
                newengine_ulog_api::ulog::debug!(
                    "plugins: loaded '{}' ver='{}' path='{}'",
                    p.info.id,
                    p.info.version,
                    display_clean(&p.path)
                );
            }
        }

        if state.strict && (!state.load_errors.is_empty() || !state.graph.scan_errors.is_empty()) {
            let mut msg = String::new();
            use std::fmt::Write as _;

            if !state.graph.scan_errors.is_empty() {
                let _ = writeln!(
                    msg,
                    "one or more dynamic libraries failed signature scan (count={}):",
                    state.graph.scan_errors.len()
                );
                for e in &state.graph.scan_errors {
                    let _ = writeln!(msg, "- {}", e);
                }
            }

            if !state.load_errors.is_empty() {
                let _ = writeln!(
                    msg,
                    "one or more plugins failed to load (count={}):",
                    state.load_errors.len()
                );
                for e in &state.load_errors {
                    let _ = writeln!(
                        msg,
                        "- path='{}' err='{}'",
                        display_clean(&e.path),
                        e.message
                    );
                }
            }

            return Err(PluginLoadError {
                path: state.graph.dir.clone(),
                message: msg,
            });
        }

        Ok(IncrementalLoadOutcome::finished(
            loaded_total,
            loaded_this_phase,
            pending_total,
            load_errors,
        ))
    }
}
