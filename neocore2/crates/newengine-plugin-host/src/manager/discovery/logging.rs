#![forbid(unsafe_op_in_unsafe_fn)]

use crate::log_fmt::{ellipsize, emit_boxed_kv, emit_prefixed_table};

use super::graph::{phase_name, DiscoveryGraph, LoadPhaseFilter, ScannedDynlib, ScannedDynlibKind};
use super::selection::{LoadSelection, SelectionDecision};

pub(super) fn emit_discovery_logs(graph: &DiscoveryGraph) {
    emit_boxed_kv(
        "PluginDiscovery :: Scan Summary",
        &[
            ("dir", crate::path_fmt::display_clean(&graph.dir)),
            ("entries_total", graph.entries_total.to_string()),
            ("dynlibs", graph.items.len().to_string()),
            ("skipped_non_dynlib", graph.skipped_non_dynlib.to_string()),
            (
                "platform_runtime_candidates",
                graph.platform_runtime_count.to_string(),
            ),
            (
                "render_backend_candidates",
                graph.render_backend_count.to_string(),
            ),
            ("bootstrap_candidates", graph.bootstrap_total.to_string()),
            ("engine_candidates", graph.engine_total.to_string()),
            ("unknown_dynlibs", graph.unknown_dynlibs.len().to_string()),
            ("scan_errors", graph.scan_errors.len().to_string()),
        ],
    );

    emit_scan_table(&graph.items);

    if !graph.scan_errors.is_empty() {
        let rows: Vec<(&str, String)> = graph
            .scan_errors
            .iter()
            .enumerate()
            .map(|(index, err)| ("scan_error", format!("#{:02} {}", index + 1, err)))
            .collect();
        emit_boxed_kv("PluginDiscovery :: Scan Errors", &rows);
    }
}

pub(super) fn emit_selection_table(
    graph: &DiscoveryGraph,
    selection: &LoadSelection,
    filter: LoadPhaseFilter,
) {
    let mut selected_yes = 0usize;
    let mut selected_runtime = 0usize;
    let mut selected_duplicates = 0usize;
    let mut selected_no = 0usize;
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(graph.items.len());

    for item in &graph.items {
        let decision = selection
            .decisions
            .get(&item.path)
            .cloned()
            .unwrap_or(SelectionDecision::Unknown);

        if decision.is_selected() {
            selected_yes = selected_yes.saturating_add(1);
        } else if decision.is_runtime() {
            selected_runtime = selected_runtime.saturating_add(1);
        } else {
            selected_no = selected_no.saturating_add(1);
            if decision.is_duplicate() {
                selected_duplicates = selected_duplicates.saturating_add(1);
            }
        }

        rows.push(vec![
            ellipsize(&item.file_name, 32),
            scanned_phase_label(&item.kind).to_owned(),
            ellipsize(&scanned_id(&item.kind), 24),
            decision.selected_label().to_owned(),
            ellipsize(&decision.reason_label(), 42),
        ]);
    }

    emit_boxed_kv(
        &format!("PluginDiscovery :: ExecutionPlan [{}]", filter.label()),
        &[
            ("selected_yes", selected_yes.to_string()),
            ("selected_runtime", selected_runtime.to_string()),
            ("selected_duplicates", selected_duplicates.to_string()),
            ("selected_no", selected_no.to_string()),
        ],
    );

    emit_prefixed_table(
        "[bootstrap]",
        &format!("PluginDiscovery :: ExecutionPlan [{}]", filter.label()),
        &["file", "phase", "id", "selected", "reason"],
        &rows,
    );
}

fn scanned_kind_label(kind: &ScannedDynlibKind) -> &'static str {
    match kind {
        ScannedDynlibKind::PlatformRuntime { .. } => "platform-runtime",
        ScannedDynlibKind::RenderBackend { .. } => "render-backend",
        ScannedDynlibKind::Plugin {
            descriptor_kind, ..
        } => match descriptor_kind {
            Some(newengine_plugin_api::PluginKind::Runtime) => "runtime",
            Some(newengine_plugin_api::PluginKind::Importer) => "importer",
            Some(newengine_plugin_api::PluginKind::Tool) => "tool",
            Some(newengine_plugin_api::PluginKind::Editor) => "editor",
            Some(newengine_plugin_api::PluginKind::Other) => "other",
            None => "plugin",
        },
        ScannedDynlibKind::Unknown => "unknown",
    }
}

fn scanned_phase_label(kind: &ScannedDynlibKind) -> &'static str {
    match kind {
        ScannedDynlibKind::PlatformRuntime { .. } => "platform",
        ScannedDynlibKind::RenderBackend { .. } => "runtime",
        ScannedDynlibKind::Plugin { phase, .. } => phase_name(*phase),
        ScannedDynlibKind::Unknown => "-",
    }
}

fn scanned_id(kind: &ScannedDynlibKind) -> String {
    match kind {
        ScannedDynlibKind::PlatformRuntime { id, .. } => id.clone(),
        ScannedDynlibKind::RenderBackend { id, .. } => id.clone(),
        ScannedDynlibKind::Plugin { id, .. } => id.clone(),
        ScannedDynlibKind::Unknown => "<unknown>".to_owned(),
    }
}

fn scanned_version(kind: &ScannedDynlibKind) -> String {
    match kind {
        ScannedDynlibKind::PlatformRuntime { version, .. } => version.clone(),
        ScannedDynlibKind::RenderBackend { version, .. } => version.clone(),
        ScannedDynlibKind::Plugin { version, .. } => version.clone(),
        ScannedDynlibKind::Unknown => "-".to_owned(),
    }
}

fn scanned_declared_caps(kind: &ScannedDynlibKind) -> String {
    match kind {
        ScannedDynlibKind::PlatformRuntime { .. } => "-".to_owned(),
        ScannedDynlibKind::RenderBackend { .. } => "-".to_owned(),
        ScannedDynlibKind::Plugin {
            declared_capabilities,
            ..
        } => declared_capabilities
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".to_owned()),
        ScannedDynlibKind::Unknown => "-".to_owned(),
    }
}

fn emit_scan_table(scanned: &[ScannedDynlib]) {
    let rows: Vec<Vec<String>> = scanned
        .iter()
        .map(|item| {
            vec![
                ellipsize(&item.file_name, 32),
                ellipsize(scanned_kind_label(&item.kind), 18),
                scanned_phase_label(&item.kind).to_owned(),
                ellipsize(&scanned_id(&item.kind), 24),
                ellipsize(&scanned_version(&item.kind), 12),
                scanned_declared_caps(&item.kind),
            ]
        })
        .collect();

    emit_prefixed_table(
        "[bootstrap]",
        "PluginDiscovery :: Graph [scan-table]",
        &["file", "type", "phase", "id", "ver", "declared_caps"],
        &rows,
    );
}
