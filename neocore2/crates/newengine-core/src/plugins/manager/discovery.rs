#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};

use libloading::Library;
use newengine_plugin_api::{
    HostApiV1, PluginBootstrapPhase, PluginDescriptor, PluginInfo, PluginKind, PluginModuleDyn,
    PluginRootV1Ref, PluginSignatureV1,
};

use crate::path_fmt::{canonicalize_if_exists, display_clean};
use crate::plugins::install_forward_logger_once;
use crate::plugins::paths::{default_plugins_dir, is_dynamic_lib, resolve_plugins_dir};

use super::adapter::{ModuleAdapterAny, V1Adapter, V2Adapter, V3Adapter};
use super::types::PluginLoadError;
use super::PluginManager;

const PLATFORM_RUNTIME_SYMBOL: &[u8] = b"newengine_platform_runtime_run_v1\0";
const PLUGIN_SIGNATURE_SYMBOL: &[u8] = b"newengine_plugin_signature_v1\0";
const PLUGIN_ROOT_SYMBOL: &[u8] = b"export_plugin_root\0";

#[derive(Debug, Clone)]
pub(super) enum ScannedDynlibKind {
    PlatformRuntime {
        id: String,
        version: String,
    },
    Plugin {
        id: String,
        version: String,
        phase: PluginBootstrapPhase,
        descriptor_kind: Option<PluginKind>,
        declared_capabilities: Option<usize>,
    },
    Unknown,
}

#[derive(Debug, Clone)]
pub(super) struct ScannedDynlib {
    path: PathBuf,
    file_name: String,
    kind: ScannedDynlibKind,
}

#[derive(Debug, Clone)]
pub(super) struct DiscoveryGraph {
    pub(super) dir: PathBuf,
    pub(super) entries_total: usize,
    pub(super) skipped_non_dynlib: usize,
    pub(super) items: Vec<ScannedDynlib>,
    pub(super) scan_errors: Vec<String>,
    pub(super) platform_runtime_count: usize,
    pub(super) bootstrap_total: usize,
    pub(super) engine_total: usize,
    pub(super) unknown_dynlibs: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct LoadSelection {
    bootstrap_candidates: Vec<PathBuf>,
    engine_candidates: Vec<PathBuf>,
}

#[derive(Copy, Clone)]
enum LoadPhaseFilter {
    All,
    BootstrapOnly,
    EngineOnly,
}

impl LoadPhaseFilter {
    #[inline]
    fn allows(self, phase: PluginBootstrapPhase) -> bool {
        match self {
            Self::All => true,
            Self::BootstrapOnly => matches!(phase, PluginBootstrapPhase::Bootstrap),
            Self::EngineOnly => matches!(
                phase,
                PluginBootstrapPhase::Platform | PluginBootstrapPhase::Engine
            ),
        }
    }

    #[inline]
    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::BootstrapOnly => "bootstrap-only",
            Self::EngineOnly => "engine-only",
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ScanPluginProbe {
    signature: Option<PluginSignatureV1>,
    info: Option<PluginInfo>,
    descriptor: Option<PluginDescriptor>,
}

impl PluginManager {
    #[inline]
    pub fn load_default(&mut self, host: HostApiV1) -> Result<(), PluginLoadError> {
        self.load_default_with_policy(host, false)
    }

    #[inline]
    pub fn load_default_with_policy(
        &mut self,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        let dir = default_plugins_dir()?;
        self.load_from_dir_with_policy(&dir, host, strict)
    }

    #[inline]
    pub fn load_bootstrap_default(
        &mut self,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        let dir = default_plugins_dir()?;
        self.load_from_dir_with_policy_and_filter(
            &dir,
            host,
            strict,
            LoadPhaseFilter::BootstrapOnly,
        )
    }

    #[inline]
    pub fn load_bootstrap_from_dir(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        self.load_from_dir_with_policy_and_filter(
            dir,
            host,
            strict,
            LoadPhaseFilter::BootstrapOnly,
        )
    }

    #[inline]
    pub fn load_engine_default(
        &mut self,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        let dir = default_plugins_dir()?;
        self.load_from_dir_with_policy_and_filter(&dir, host, strict, LoadPhaseFilter::EngineOnly)
    }

    #[inline]
    pub fn load_engine_from_dir(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        self.load_from_dir_with_policy_and_filter(dir, host, strict, LoadPhaseFilter::EngineOnly)
    }

    #[inline]
    pub fn load_from_dir(&mut self, dir: &Path, host: HostApiV1) -> Result<(), PluginLoadError> {
        self.load_from_dir_with_policy(dir, host, false)
    }

    pub fn load_from_dir_with_policy(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
    ) -> Result<(), PluginLoadError> {
        self.load_from_dir_with_policy_and_filter(dir, host, strict, LoadPhaseFilter::All)
    }

    #[inline]
    pub(crate) fn invalidate_discovery_cache(&mut self) {
        self.discovery_cache = None;
    }

    fn load_from_dir_with_policy_and_filter(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
        filter: LoadPhaseFilter,
    ) -> Result<(), PluginLoadError> {
        let (graph, graph_is_new) = self.ensure_discovery_graph(dir)?;
        let selection = self.build_load_selection(&graph, filter);

        let loaded_ids_before = self.loaded_ids.clone();

        log::info!(
            "plugins: phase selection filter='{}' bootstrap={} engine={} platform_runtime={} unknown={} dir='{}'",
            filter.label(),
            selection.bootstrap_candidates.len(),
            selection.engine_candidates.len(),
            graph.platform_runtime_count,
            graph.unknown_dynlibs.len(),
            display_clean(&graph.dir),
        );

        let mut load_errors: Vec<PluginLoadError> = Vec::new();

        for path in selection
            .bootstrap_candidates
            .iter()
            .chain(selection.engine_candidates.iter())
        {
            if let Err(e) = self.load_one(path, host.clone()) {
                log::warn!("plugins: failed to load '{}': {}", display_clean(path), e);
                load_errors.push(e);
            }
        }

        install_forward_logger_once(host.clone());

        if graph_is_new {
            emit_discovery_logs(&graph);
        }

        emit_selection_table(&graph, &selection, filter, &loaded_ids_before);

        if matches!(filter, LoadPhaseFilter::BootstrapOnly) {
            let rid = crate::run_id::run_id().unwrap_or("<unknown>");
            log::info!("startup: Run ID: {}", rid);

            crate::startup::SystemProbe::probe().emit_table("startup");
            if let Some(r) = crate::startup::last_load_report() {
                r.emit_logs();
            }
        }

        log::info!("plugins: load complete loaded_count={}", self.loaded.len());

        self.validate_required_capabilities();

        if log::log_enabled!(log::Level::Debug) {
            for p in &self.loaded {
                log::debug!(
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

    #[inline]
    pub fn load_path(&mut self, path: &Path, host: HostApiV1) -> Result<(), PluginLoadError> {
        let res = self.load_one(path, host.clone());
        install_forward_logger_once(host);
        res
    }

    fn ensure_discovery_graph(
        &mut self,
        dir: &Path,
    ) -> Result<(DiscoveryGraph, bool), PluginLoadError> {
        let dir = resolve_plugins_dir(dir)?;

        if let Err(e) = std::fs::create_dir_all(&dir) {
            return Err(PluginLoadError {
                path: dir.clone(),
                message: format!("create_dir_all failed: {e}"),
            });
        }

        let dir = canonicalize_if_exists(&dir);

        if let Some(graph) = &self.discovery_cache {
            if graph.dir == dir {
                log::debug!(
                    "plugins: discovery cache hit dir='{}' entries={} dynlibs={}",
                    display_clean(&graph.dir),
                    graph.entries_total,
                    graph.items.len(),
                );
                return Ok((graph.clone(), false));
            }
        }

        let graph = scan_plugins_dir(&dir)?;
        self.discovery_cache = Some(graph.clone());
        Ok((graph, true))
    }

    fn build_load_selection(&self, graph: &DiscoveryGraph, filter: LoadPhaseFilter) -> LoadSelection {
        let mut out = LoadSelection::default();

        for item in &graph.items {
            let ScannedDynlibKind::Plugin { id, phase, .. } = &item.kind else {
                continue;
            };

            if !filter.allows(*phase) {
                continue;
            }

            if self.loaded_ids.contains(id) {
                continue;
            }

            match phase {
                PluginBootstrapPhase::Bootstrap => {
                    out.bootstrap_candidates.push(item.path.clone());
                }
                PluginBootstrapPhase::Platform | PluginBootstrapPhase::Engine => {
                    out.engine_candidates.push(item.path.clone());
                }
            }
        }

        out
    }
}

fn scan_plugins_dir(dir: &Path) -> Result<DiscoveryGraph, PluginLoadError> {
    let rd = std::fs::read_dir(dir).map_err(|e| PluginLoadError {
        path: dir.to_path_buf(),
        message: format!("read_dir failed: {e}"),
    })?;

    let mut entries_total: usize = 0;
    let mut skipped_non_dynlib: usize = 0;
    let mut items: Vec<ScannedDynlib> = Vec::new();
    let mut scan_errors: Vec<String> = Vec::new();

    for ent in rd {
        entries_total = entries_total.saturating_add(1);

        let ent = ent.map_err(|e| PluginLoadError {
            path: dir.to_path_buf(),
            message: format!("read_dir entry failed: {e}"),
        })?;

        let path = ent.path();
        if !is_dynamic_lib(&path) {
            skipped_non_dynlib = skipped_non_dynlib.saturating_add(1);
            continue;
        }

        match scan_dynamic_lib(&path) {
            Ok(v) => items.push(v),
            Err(e) => {
                log::warn!("plugins: scan failed for '{}': {}", display_clean(&path), e);
                scan_errors.push(format!("{}: {}", display_clean(&path), e));
            }
        }
    }

    items.sort_by(|a, b| a.file_name.cmp(&b.file_name));

    let mut platform_runtime_count = 0usize;
    let mut bootstrap_total = 0usize;
    let mut engine_total = 0usize;
    let mut unknown_dynlibs: Vec<String> = Vec::new();

    for item in &items {
        match &item.kind {
            ScannedDynlibKind::PlatformRuntime { .. } => {
                platform_runtime_count = platform_runtime_count.saturating_add(1);
            }
            ScannedDynlibKind::Plugin { phase, .. } => match phase {
                PluginBootstrapPhase::Bootstrap => {
                    bootstrap_total = bootstrap_total.saturating_add(1);
                }
                PluginBootstrapPhase::Platform | PluginBootstrapPhase::Engine => {
                    engine_total = engine_total.saturating_add(1);
                }
            },
            ScannedDynlibKind::Unknown => {
                unknown_dynlibs.push(item.file_name.clone());
            }
        }
    }

    Ok(DiscoveryGraph {
        dir: dir.to_path_buf(),
        entries_total,
        skipped_non_dynlib,
        items,
        scan_errors,
        platform_runtime_count,
        bootstrap_total,
        engine_total,
        unknown_dynlibs,
    })
}

fn emit_discovery_logs(graph: &DiscoveryGraph) {
    log::info!("plugins: scanning directory '{}'", display_clean(&graph.dir));

    if log::log_enabled!(log::Level::Debug) {
        log::debug!(
            "plugins: scan summary dir='{}' entries_total={} dynlibs={} skipped_non_dynlib={} platform_runtime_candidates={} unknown_dynlibs={} scan_errors={}",
            display_clean(&graph.dir),
            graph.entries_total,
            graph.items.len(),
            graph.skipped_non_dynlib,
            graph.platform_runtime_count,
            graph.unknown_dynlibs.len(),
            graph.scan_errors.len(),
        );
    }

    emit_scan_table(&graph.items);

    log::info!(
        "plugins: phase discovery bootstrap={} engine={} platform_runtime={} unknown={} dir='{}'",
        graph.bootstrap_total,
        graph.engine_total,
        graph.platform_runtime_count,
        graph.unknown_dynlibs.len(),
        display_clean(&graph.dir),
    );

    if !graph.scan_errors.is_empty() {
        for err in &graph.scan_errors {
            log::warn!("plugins: scan error {}", err);
        }
    }
}

fn emit_selection_table(
    graph: &DiscoveryGraph,
    selection: &LoadSelection,
    filter: LoadPhaseFilter,
    loaded_ids_before: &newengine_math::collections::prelude::NeHashSet<String>,
) {
    let headers = ["file", "phase", "id", "selected", "reason"];

    let mut selected_paths: std::collections::HashSet<&Path> = std::collections::HashSet::new();
    for path in &selection.bootstrap_candidates {
        selected_paths.insert(path.as_path());
    }
    for path in &selection.engine_candidates {
        selected_paths.insert(path.as_path());
    }

    let mut rows: Vec<[String; 5]> = Vec::with_capacity(graph.items.len());
    for item in &graph.items {
        let phase = scanned_phase_label(&item.kind).to_owned();
        let id = scanned_id(&item.kind);

        let (selected, reason) = match &item.kind {
            ScannedDynlibKind::PlatformRuntime { .. } => {
                ("runtime".to_owned(), "platform runtime".to_owned())
            }
            ScannedDynlibKind::Unknown => ("no".to_owned(), "unknown dynlib".to_owned()),
            ScannedDynlibKind::Plugin { phase, .. } => {
                if loaded_ids_before.contains(id.as_str()) {
                    ("no".to_owned(), "already loaded".to_owned())
                } else if !filter.allows(*phase) {
                    ("no".to_owned(), format!("filtered by {}", filter.label()))
                } else if selected_paths.contains(item.path.as_path()) {
                    ("yes".to_owned(), "phase match".to_owned())
                } else {
                    ("no".to_owned(), "not selected".to_owned())
                }
            }
        };

        rows.push([item.file_name.clone(), phase, id, selected, reason]);
    }

    let mut widths = [
        headers[0].chars().count(),
        headers[1].chars().count(),
        headers[2].chars().count(),
        headers[3].chars().count(),
        headers[4].chars().count(),
    ];

    for row in &rows {
        for (i, col) in row.iter().enumerate() {
            widths[i] = widths[i].max(col.chars().count());
        }
    }

    let border = format!(
        "+-{}-+-{}-+-{}-+-{}-+-{}-+",
        "-".repeat(widths[0]),
        "-".repeat(widths[1]),
        "-".repeat(widths[2]),
        "-".repeat(widths[3]),
        "-".repeat(widths[4]),
    );

    log::info!(
        "[bootstrap] PluginDiscovery :: ExecutionPlan [{}]",
        filter.label()
    );
    log::info!("[bootstrap] {}", border);
    log::info!(
        "[bootstrap] | {} | {} | {} | {} | {} |",
        pad_right(headers[0], widths[0]),
        pad_right(headers[1], widths[1]),
        pad_right(headers[2], widths[2]),
        pad_right(headers[3], widths[3]),
        pad_right(headers[4], widths[4]),
    );
    log::info!("[bootstrap] {}", border);

    for row in &rows {
        log::info!(
            "[bootstrap] | {} | {} | {} | {} | {} |",
            pad_right(&row[0], widths[0]),
            pad_right(&row[1], widths[1]),
            pad_right(&row[2], widths[2]),
            pad_right(&row[3], widths[3]),
            pad_right(&row[4], widths[4]),
        );
    }

    log::info!("[bootstrap] {}", border);
}

fn phase_name(phase: PluginBootstrapPhase) -> &'static str {
    match phase {
        PluginBootstrapPhase::Bootstrap => "bootstrap",
        PluginBootstrapPhase::Platform => "platform",
        PluginBootstrapPhase::Engine => "engine",
    }
}

fn scanned_kind_label(kind: &ScannedDynlibKind) -> &'static str {
    match kind {
        ScannedDynlibKind::PlatformRuntime { .. } => "platform-runtime",
        ScannedDynlibKind::Plugin {
            descriptor_kind, ..
        } => match descriptor_kind {
            Some(PluginKind::Runtime) => "runtime",
            Some(PluginKind::Importer) => "importer",
            Some(PluginKind::Tool) => "tool",
            Some(PluginKind::Editor) => "editor",
            Some(PluginKind::Other) => "other",
            Some(_) => "plugin",
            None => "plugin",
        },
        ScannedDynlibKind::Unknown => "unknown",
    }
}

fn scanned_phase_label(kind: &ScannedDynlibKind) -> &'static str {
    match kind {
        ScannedDynlibKind::PlatformRuntime { .. } => "platform",
        ScannedDynlibKind::Plugin { phase, .. } => phase_name(*phase),
        ScannedDynlibKind::Unknown => "-",
    }
}

fn scanned_id(kind: &ScannedDynlibKind) -> String {
    match kind {
        ScannedDynlibKind::PlatformRuntime { id, .. } => id.clone(),
        ScannedDynlibKind::Plugin { id, .. } => id.clone(),
        ScannedDynlibKind::Unknown => "<unknown>".to_owned(),
    }
}

fn scanned_version(kind: &ScannedDynlibKind) -> String {
    match kind {
        ScannedDynlibKind::PlatformRuntime { version, .. } => version.clone(),
        ScannedDynlibKind::Plugin { version, .. } => version.clone(),
        ScannedDynlibKind::Unknown => "-".to_owned(),
    }
}

fn scanned_declared_caps(kind: &ScannedDynlibKind) -> String {
    match kind {
        ScannedDynlibKind::PlatformRuntime { .. } => "-".to_owned(),
        ScannedDynlibKind::Plugin {
            declared_capabilities,
            ..
        } => declared_capabilities
            .map(|v| v.to_string())
            .unwrap_or_else(|| "?".to_owned()),
        ScannedDynlibKind::Unknown => "-".to_owned(),
    }
}

fn pad_right(value: &str, width: usize) -> String {
    let len = value.chars().count();
    if len >= width {
        value.to_owned()
    } else {
        let mut out = String::with_capacity(width);
        out.push_str(value);
        out.push_str(&" ".repeat(width - len));
        out
    }
}

fn emit_scan_table(scanned: &[ScannedDynlib]) {
    let headers = ["file", "type", "phase", "id", "ver", "declared_caps"];

    let mut rows: Vec<[String; 6]> = Vec::with_capacity(scanned.len());
    for item in scanned {
        rows.push([
            item.file_name.clone(),
            scanned_kind_label(&item.kind).to_owned(),
            scanned_phase_label(&item.kind).to_owned(),
            scanned_id(&item.kind),
            scanned_version(&item.kind),
            scanned_declared_caps(&item.kind),
        ]);
    }

    let mut widths = [
        headers[0].chars().count(),
        headers[1].chars().count(),
        headers[2].chars().count(),
        headers[3].chars().count(),
        headers[4].chars().count(),
        headers[5].chars().count(),
    ];

    for row in &rows {
        for (i, col) in row.iter().enumerate() {
            widths[i] = widths[i].max(col.chars().count());
        }
    }

    let border = format!(
        "+-{}-+-{}-+-{}-+-{}-+-{}-+-{}-+",
        "-".repeat(widths[0]),
        "-".repeat(widths[1]),
        "-".repeat(widths[2]),
        "-".repeat(widths[3]),
        "-".repeat(widths[4]),
        "-".repeat(widths[5]),
    );

    log::info!("[bootstrap] PluginDiscovery :: Graph [scan-table]");
    log::info!("[bootstrap] {}", border);
    log::info!(
        "[bootstrap] | {} | {} | {} | {} | {} | {} |",
        pad_right(headers[0], widths[0]),
        pad_right(headers[1], widths[1]),
        pad_right(headers[2], widths[2]),
        pad_right(headers[3], widths[3]),
        pad_right(headers[4], widths[4]),
        pad_right(headers[5], widths[5]),
    );
    log::info!("[bootstrap] {}", border);

    for row in &rows {
        log::info!(
            "[bootstrap] | {} | {} | {} | {} | {} | {} |",
            pad_right(&row[0], widths[0]),
            pad_right(&row[1], widths[1]),
            pad_right(&row[2], widths[2]),
            pad_right(&row[3], widths[3]),
            pad_right(&row[4], widths[4]),
            pad_right(&row[5], widths[5]),
        );
    }

    log::info!("[bootstrap] {}", border);
}

fn file_name_only(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "<unnamed>".to_owned())
}

fn scan_dynamic_lib(path: &Path) -> Result<ScannedDynlib, String> {
    let file_name = file_name_only(path);
    let lib = unsafe { Library::new(path) }.map_err(|e| format!("Library::new failed: {e}"))?;

    if unsafe { lib.get::<unsafe extern "C" fn()>(PLATFORM_RUNTIME_SYMBOL) }.is_ok() {
        let (id, version) = infer_platform_runtime_identity(path);
        return Ok(ScannedDynlib {
            path: path.to_path_buf(),
            file_name,
            kind: ScannedDynlibKind::PlatformRuntime { id, version },
        });
    }

    let plugin_probe = probe_plugin_metadata(&lib)?;

    if let Some(kind) = build_scanned_plugin_kind(&plugin_probe) {
        return Ok(ScannedDynlib {
            path: path.to_path_buf(),
            file_name,
            kind,
        });
    }

    Ok(ScannedDynlib {
        path: path.to_path_buf(),
        file_name,
        kind: ScannedDynlibKind::Unknown,
    })
}

fn probe_plugin_metadata(lib: &Library) -> Result<ScanPluginProbe, String> {
    let mut out = ScanPluginProbe::default();

    if let Ok(sym) =
        unsafe { lib.get::<unsafe extern "C" fn() -> PluginSignatureV1>(PLUGIN_SIGNATURE_SYMBOL) }
    {
        out.signature = Some(unsafe { sym() });
    }

    if let Ok(sym) =
        unsafe { lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(PLUGIN_ROOT_SYMBOL) }
    {
        let root = unsafe { sym() };
        let (_module, info, descriptor) = select_abi_for_scan(root);
        out.info = Some(info);
        out.descriptor = descriptor;
    }

    Ok(out)
}

fn build_scanned_plugin_kind(probe: &ScanPluginProbe) -> Option<ScannedDynlibKind> {
    if probe.signature.is_none() && probe.info.is_none() && probe.descriptor.is_none() {
        return None;
    }

    let id = probe
        .signature
        .as_ref()
        .map(|s| s.id.to_string())
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            probe.info
                .as_ref()
                .map(|i| i.id.to_string())
                .filter(|v| !v.trim().is_empty())
        })
        .unwrap_or_else(|| "<unknown-plugin>".to_owned());

    let version = probe
        .signature
        .as_ref()
        .map(|s| s.version.to_string())
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            probe.info
                .as_ref()
                .map(|i| i.version.to_string())
                .filter(|v| !v.trim().is_empty())
        })
        .unwrap_or_else(|| "-".to_owned());

    let phase = probe
        .signature
        .as_ref()
        .map(|s| s.bootstrap_phase)
        .unwrap_or(PluginBootstrapPhase::Engine);

    let descriptor_kind = probe
        .descriptor
        .as_ref()
        .map(|d| d.kind)
        .or_else(|| probe.signature.as_ref().map(|s| s.kind));

    let declared_capabilities = probe
        .descriptor
        .as_ref()
        .map(|d| d.capabilities.len());

    Some(ScannedDynlibKind::Plugin {
        id,
        version,
        phase,
        descriptor_kind,
        declared_capabilities,
    })
}

fn infer_platform_runtime_identity(path: &Path) -> (String, String) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "<platform-runtime>".to_owned());

    let parts: Vec<&str> = stem.split('-').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return ("<platform-runtime>".to_owned(), "-".to_owned());
    }

    let version_index = parts
        .iter()
        .position(|part| part.chars().next().is_some_and(|c| c.is_ascii_digit()));

    match version_index {
        Some(idx) => {
            let id = parts[..idx].join("-");
            let raw_version = parts[idx..].join("-");
            let version = normalize_version_suffix(&raw_version);

            let id = if id.trim().is_empty() {
                "<platform-runtime>".to_owned()
            } else {
                id
            };

            let version = if version.trim().is_empty() {
                "-".to_owned()
            } else {
                version
            };

            (id, version)
        }
        None => (stem, "-".to_owned()),
    }
}

fn normalize_version_suffix(raw: &str) -> String {
    raw.strip_suffix("-dev")
        .or_else(|| raw.strip_suffix("-debug"))
        .or_else(|| raw.strip_suffix("-release"))
        .unwrap_or(raw)
        .to_owned()
}

fn select_abi_for_scan(
    root: PluginRootV1Ref,
) -> (ModuleAdapterAny, PluginInfo, Option<PluginDescriptor>) {
    if let Some(create_v3) = root.create_v3() {
        let m3 = create_v3();
        let d = m3.descriptor_v3();
        let info = PluginInfo {
            id: d.id.clone(),
            name: d.name.clone(),
            version: d.version.clone(),
        };
        (
            ModuleAdapterAny::V3(V3Adapter { module: m3 }),
            info,
            Some(d),
        )
    } else if let Some(create_v2) = root.create_v2() {
        let m2 = create_v2();
        let d = m2.descriptor();
        let info = PluginInfo {
            id: d.id.clone(),
            name: d.name.clone(),
            version: d.version.clone(),
        };
        (
            ModuleAdapterAny::V2(V2Adapter { module: m2 }),
            info,
            Some(d),
        )
    } else {
        let m1: PluginModuleDyn<'static> = root.create()();
        let info = m1.info();
        (ModuleAdapterAny::V1(V1Adapter { module: m1 }), info, None)
    }
}