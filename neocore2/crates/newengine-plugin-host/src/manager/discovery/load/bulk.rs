use super::*;

impl PluginManager {
    pub(super) fn begin_incremental_load_from_graph(
        &mut self,
        graph: DiscoveryGraph,
        graph_is_new: bool,
        filter: LoadPhaseFilter,
        strict: bool,
        load_origin: PluginLoadOrigin,
    ) {
        let selection = build_load_selection(
            &graph,
            filter,
            &self.loaded_ids,
            self.frozen_composition_plan.as_ref(),
        );
        let pending = selection
            .bootstrap_candidates
            .iter()
            .chain(selection.engine_candidates.iter())
            .cloned()
            .collect::<Vec<_>>();

        emit_boxed_kv(
            &format!(
                "PluginDiscovery :: Incremental Phase Selection [{}]",
                filter.label()
            ),
            &[
                ("dir", display_clean(&graph.dir)),
                ("pending", pending.len().to_string()),
                (
                    "bootstrap_queue",
                    selection.bootstrap_candidates.len().to_string(),
                ),
                (
                    "engine_queue",
                    selection.engine_candidates.len().to_string(),
                ),
                ("platform_runtime", graph.platform_runtime_count.to_string()),
                ("unknown_dynlibs", graph.unknown_dynlibs.len().to_string()),
            ],
        );

        self.discovery_cache = Some(graph.clone());
        self.incremental_load = Some(IncrementalLoadState {
            graph,
            graph_is_new,
            selection,
            filter,
            pending,
            next_index: 0,
            loaded_ids_before_len: self.loaded_ids.len(),
            load_errors: Vec::new(),
            strict,
            load_origin,
        });
    }

    pub(super) fn load_from_dir_with_policy_and_filter(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
        filter: LoadPhaseFilter,
        load_origin: PluginLoadOrigin,
    ) -> Result<(), PluginLoadError> {
        let (graph, graph_is_new) = self.ensure_discovery_graph(dir)?;
        let selection = build_load_selection(
            &graph,
            filter,
            &self.loaded_ids,
            self.frozen_composition_plan.as_ref(),
        );

        let loaded_ids_before = self.loaded_ids.clone();

        emit_boxed_kv(
            &format!("PluginDiscovery :: Phase Selection [{}]", filter.label()),
            &[
                ("dir", display_clean(&graph.dir)),
                (
                    "bootstrap_queue",
                    selection.bootstrap_candidates.len().to_string(),
                ),
                (
                    "engine_queue",
                    selection.engine_candidates.len().to_string(),
                ),
                ("platform_runtime", graph.platform_runtime_count.to_string()),
                ("unknown_dynlibs", graph.unknown_dynlibs.len().to_string()),
            ],
        );

        let mut load_errors: Vec<PluginLoadError> = Vec::new();

        for path in selection
            .bootstrap_candidates
            .iter()
            .chain(selection.engine_candidates.iter())
        {
            if let Err(e) = self.load_one_with_origin(path, host.clone(), load_origin) {
                newengine_ulog_api::ulog::warn!(
                    "plugins: failed to load '{}': {}",
                    display_clean(path),
                    e
                );
                load_errors.push(e);
            }
        }

        if graph_is_new {
            emit_discovery_logs(&graph);
        }

        emit_selection_table(&graph, &selection, filter);

        let loaded_total = self.loaded.len();
        let loaded_this_phase = loaded_total.saturating_sub(loaded_ids_before.len());

        emit_boxed_kv(
            &format!("PluginDiscovery :: Load Result [{}]", filter.label()),
            &[
                ("loaded_total", loaded_total.to_string()),
                ("loaded_this_phase", loaded_this_phase.to_string()),
                ("load_errors", load_errors.len().to_string()),
                ("scan_errors", graph.scan_errors.len().to_string()),
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

        if strict && (!load_errors.is_empty() || !graph.scan_errors.is_empty()) {
            let mut msg = String::new();
            use std::fmt::Write as _;

            if !graph.scan_errors.is_empty() {
                let _ = writeln!(
                    msg,
                    "one or more dynamic libraries failed signature scan (count={}):",
                    graph.scan_errors.len()
                );
                for e in &graph.scan_errors {
                    let _ = writeln!(msg, "- {}", e);
                }
            }

            if !load_errors.is_empty() {
                let _ = writeln!(
                    msg,
                    "one or more plugins failed to load (count={}):",
                    load_errors.len()
                );
                for e in &load_errors {
                    let _ = writeln!(
                        msg,
                        "- path='{}' err='{}'",
                        display_clean(&e.path),
                        e.message
                    );
                }
            }

            return Err(PluginLoadError {
                path: graph.dir.clone(),
                message: msg,
            });
        }

        Ok(())
    }
}
