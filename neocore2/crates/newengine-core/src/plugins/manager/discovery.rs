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
enum ScannedDynlibKind {
    PlatformRuntime,
    Plugin {
        id: String,
        version: String,
        phase: PluginBootstrapPhase,
        descriptor_kind: Option<PluginKind>,
        capabilities: usize,
    },
    Unknown,
}

#[derive(Debug, Clone)]
struct ScannedDynlib {
    path: PathBuf,
    file_name: String,
    kind: ScannedDynlibKind,
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

    fn load_from_dir_with_policy_and_filter(
        &mut self,
        dir: &Path,
        host: HostApiV1,
        strict: bool,
        filter: LoadPhaseFilter,
    ) -> Result<(), PluginLoadError> {
        let dir = resolve_plugins_dir(dir)?;

        if let Err(e) = std::fs::create_dir_all(&dir) {
            return Err(PluginLoadError {
                path: dir.clone(),
                message: format!("create_dir_all failed: {e}"),
            });
        }

        let dir = canonicalize_if_exists(&dir);
        let rd = std::fs::read_dir(&dir).map_err(|e| PluginLoadError {
            path: dir.clone(),
            message: format!("read_dir failed: {e}"),
        })?;

        let mut entries_total: usize = 0;
        let mut skipped_non_dynlib: usize = 0;
        let mut scanned: Vec<ScannedDynlib> = Vec::new();
        let mut scan_errors: Vec<String> = Vec::new();

        for ent in rd {
            entries_total = entries_total.saturating_add(1);
            let ent = ent.map_err(|e| PluginLoadError {
                path: dir.clone(),
                message: format!("read_dir entry failed: {e}"),
            })?;

            let p = ent.path();
            if !is_dynamic_lib(&p) {
                skipped_non_dynlib = skipped_non_dynlib.saturating_add(1);
                continue;
            }

            match scan_dynamic_lib(&p) {
                Ok(v) => scanned.push(v),
                Err(e) => {
                    log::warn!("plugins: scan failed for '{}': {}", display_clean(&p), e);
                    scan_errors.push(format!("{}: {}", display_clean(&p), e));
                }
            }
        }

        scanned.sort_by(|a, b| a.file_name.cmp(&b.file_name));

        let mut platform_runtime_count = 0usize;
        let mut bootstrap_total = 0usize;
        let mut engine_total = 0usize;
        let mut bootstrap_candidates: Vec<PathBuf> = Vec::new();
        let mut engine_candidates: Vec<PathBuf> = Vec::new();
        let mut unknown_dynlibs: Vec<String> = Vec::new();

        for item in &scanned {
            match &item.kind {
                ScannedDynlibKind::PlatformRuntime => {
                    platform_runtime_count = platform_runtime_count.saturating_add(1);
                }
                ScannedDynlibKind::Plugin { phase, .. } => {
                    match phase {
                        PluginBootstrapPhase::Bootstrap => {
                            bootstrap_total = bootstrap_total.saturating_add(1);
                        }
                        PluginBootstrapPhase::Platform | PluginBootstrapPhase::Engine => {
                            engine_total = engine_total.saturating_add(1);
                        }
                    }

                    if filter.allows(*phase) {
                        match phase {
                            PluginBootstrapPhase::Bootstrap => {
                                bootstrap_candidates.push(item.path.clone())
                            }
                            PluginBootstrapPhase::Platform | PluginBootstrapPhase::Engine => {
                                engine_candidates.push(item.path.clone())
                            }
                        }
                    }
                }
                ScannedDynlibKind::Unknown => unknown_dynlibs.push(item.file_name.clone()),
            }
        }

        if log::log_enabled!(log::Level::Debug) {
            log::debug!(
                "plugins: scan summary dir='{}' entries_total={} dynlibs={} skipped_non_dynlib={} platform_runtime_candidates={} unknown_dynlibs={} scan_errors={}",
                display_clean(&dir),
                entries_total,
                scanned.len(),
                skipped_non_dynlib,
                platform_runtime_count,
                unknown_dynlibs.len(),
                scan_errors.len(),
            );
        }

        emit_scan_table(&scanned, filter);

        if !scan_errors.is_empty() {
            for err in &scan_errors {
                log::warn!("plugins: scan error {}", err);
            }
        }

        let mut load_errors: Vec<PluginLoadError> = Vec::new();

        for path in bootstrap_candidates.iter().chain(engine_candidates.iter()) {
            if let Err(e) = self.load_one(path, host.clone()) {
                log::warn!("plugins: failed to load '{}': {}", display_clean(path), e);
                load_errors.push(e);
            }
        }

        install_forward_logger_once(host.clone());

        if matches!(filter, LoadPhaseFilter::BootstrapOnly) {
            let rid = crate::run_id::run_id().unwrap_or("<unknown>");
            log::info!("startup: Run ID: {}", rid);

            crate::startup::SystemProbe::probe().emit_table("startup");
            if let Some(r) = crate::startup::last_load_report() {
                r.emit_logs();
            }
        }

        log::info!("plugins: scanning directory '{}'", display_clean(&dir));
        log::info!(
            "plugins: phase discovery bootstrap={} engine={} platform_runtime={} unknown={} dir='{}'",
            bootstrap_total,
            engine_total,
            platform_runtime_count,
            unknown_dynlibs.len(),
            display_clean(&dir),
        );
        log::info!(
            "plugins: phase selection filter='{}' bootstrap={} engine={} platform_runtime={} unknown={} dir='{}'",
            filter.label(),
            bootstrap_candidates.len(),
            engine_candidates.len(),
            platform_runtime_count,
            unknown_dynlibs.len(),
            display_clean(&dir),
        );

        log::info!("plugins: load complete loaded_count={}", self.loaded.len());

        self.validate_required_capabilities();
        if log::log_enabled!(log::Level::Debug) {
            for p in self.loaded.iter() {
                log::debug!(
                    "plugins: loaded '{}' ver='{}' path='{}'",
                    p.info.id,
                    p.info.version,
                    display_clean(&p.path)
                );
            }
        }

        if strict && (!load_errors.is_empty() || !scan_errors.is_empty()) {
            let mut msg = String::new();
            use std::fmt::Write as _;
            if !scan_errors.is_empty() {
                let _ = writeln!(
                    msg,
                    "one or more dynamic libraries failed signature scan (count={}):",
                    scan_errors.len()
                );
                for e in &scan_errors {
                    let _ = writeln!(msg, "- {}", e);
                }
            }
            if !load_errors.is_empty() {
                let _ = writeln!(
                    msg,
                    "one or more plugins failed to load (count={}):",
                    load_errors.len()
                );
                for e in load_errors.iter() {
                    let _ = writeln!(
                        msg,
                        "- path='{}' err='{}'",
                        display_clean(&e.path),
                        e.message
                    );
                }
            }
            return Err(PluginLoadError {
                path: dir.clone(),
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
        ScannedDynlibKind::PlatformRuntime => "platform-runtime",
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
        ScannedDynlibKind::PlatformRuntime => "platform",
        ScannedDynlibKind::Plugin { phase, .. } => phase_name(*phase),
        ScannedDynlibKind::Unknown => "-",
    }
}

fn scanned_id(kind: &ScannedDynlibKind) -> String {
    match kind {
        ScannedDynlibKind::PlatformRuntime => "<platform-runtime>".to_string(),
        ScannedDynlibKind::Plugin { id, .. } => id.clone(),
        ScannedDynlibKind::Unknown => "<unknown>".to_string(),
    }
}

fn scanned_version(kind: &ScannedDynlibKind) -> String {
    match kind {
        ScannedDynlibKind::PlatformRuntime => "-".to_string(),
        ScannedDynlibKind::Plugin { version, .. } => version.clone(),
        ScannedDynlibKind::Unknown => "-".to_string(),
    }
}

fn scanned_caps(kind: &ScannedDynlibKind) -> String {
    match kind {
        ScannedDynlibKind::PlatformRuntime => "-".to_string(),
        ScannedDynlibKind::Plugin { capabilities, .. } => capabilities.to_string(),
        ScannedDynlibKind::Unknown => "-".to_string(),
    }
}

fn scanned_selected(kind: &ScannedDynlibKind, filter: LoadPhaseFilter) -> &'static str {
    match kind {
        ScannedDynlibKind::PlatformRuntime => "runtime",
        ScannedDynlibKind::Plugin { phase, .. } => {
            if filter.allows(*phase) {
                "yes"
            } else {
                "no"
            }
        }
        ScannedDynlibKind::Unknown => "skip",
    }
}

fn pad_right(value: &str, width: usize) -> String {
    let len = value.chars().count();
    if len >= width {
        value.to_string()
    } else {
        let mut out = String::with_capacity(width);
        out.push_str(value);
        out.push_str(&" ".repeat(width - len));
        out
    }
}

fn emit_scan_table(scanned: &[ScannedDynlib], filter: LoadPhaseFilter) {
    let headers = ["file", "type", "phase", "id", "ver", "caps", "selected"];

    let mut rows: Vec<[String; 7]> = Vec::with_capacity(scanned.len());
    for item in scanned {
        rows.push([
            item.file_name.clone(),
            scanned_kind_label(&item.kind).to_string(),
            scanned_phase_label(&item.kind).to_string(),
            scanned_id(&item.kind),
            scanned_version(&item.kind),
            scanned_caps(&item.kind),
            scanned_selected(&item.kind, filter).to_string(),
        ]);
    }

    let mut widths = [
        headers[0].chars().count(),
        headers[1].chars().count(),
        headers[2].chars().count(),
        headers[3].chars().count(),
        headers[4].chars().count(),
        headers[5].chars().count(),
        headers[6].chars().count(),
    ];

    for row in &rows {
        for (i, col) in row.iter().enumerate() {
            widths[i] = widths[i].max(col.chars().count());
        }
    }

    let border = format!(
        "+-{}-+-{}-+-{}-+-{}-+-{}-+-{}-+-{}-+",
        "-".repeat(widths[0]),
        "-".repeat(widths[1]),
        "-".repeat(widths[2]),
        "-".repeat(widths[3]),
        "-".repeat(widths[4]),
        "-".repeat(widths[5]),
        "-".repeat(widths[6]),
    );

    log::info!("[bootstrap] PluginDiscovery :: Phase 1 [scan-table]");
    log::info!("[bootstrap] {}", border);
    log::info!(
        "[bootstrap] | {} | {} | {} | {} | {} | {} | {} |",
        pad_right(headers[0], widths[0]),
        pad_right(headers[1], widths[1]),
        pad_right(headers[2], widths[2]),
        pad_right(headers[3], widths[3]),
        pad_right(headers[4], widths[4]),
        pad_right(headers[5], widths[5]),
        pad_right(headers[6], widths[6]),
    );
    log::info!("[bootstrap] {}", border);

    for row in &rows {
        log::info!(
            "[bootstrap] | {} | {} | {} | {} | {} | {} | {} |",
            pad_right(&row[0], widths[0]),
            pad_right(&row[1], widths[1]),
            pad_right(&row[2], widths[2]),
            pad_right(&row[3], widths[3]),
            pad_right(&row[4], widths[4]),
            pad_right(&row[5], widths[5]),
            pad_right(&row[6], widths[6]),
        );
    }

    log::info!("[bootstrap] {}", border);
}

fn file_name_only(p: &Path) -> String {
    p.file_name()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "<unnamed>".to_owned())
}

fn scan_dynamic_lib(path: &Path) -> Result<ScannedDynlib, String> {
    let file_name = file_name_only(path);
    let lib = unsafe { Library::new(path) }.map_err(|e| format!("Library::new failed: {e}"))?;

    if unsafe { lib.get::<unsafe extern "C" fn()>(PLATFORM_RUNTIME_SYMBOL) }.is_ok() {
        return Ok(ScannedDynlib {
            path: path.to_path_buf(),
            file_name,
            kind: ScannedDynlibKind::PlatformRuntime,
        });
    }

    if let Ok(sym) =
        unsafe { lib.get::<unsafe extern "C" fn() -> PluginSignatureV1>(PLUGIN_SIGNATURE_SYMBOL) }
    {
        let sig = unsafe { sym() };
        return Ok(ScannedDynlib {
            path: path.to_path_buf(),
            file_name,
            kind: ScannedDynlibKind::Plugin {
                id: sig.id.to_string(),
                version: sig.version.to_string(),
                phase: sig.bootstrap_phase,
                descriptor_kind: Some(sig.kind),
                capabilities: 0,
            },
        });
    }

    if let Ok(sym) =
        unsafe { lib.get::<unsafe extern "C" fn() -> PluginRootV1Ref>(PLUGIN_ROOT_SYMBOL) }
    {
        let root = unsafe { sym() };
        let (_module, info, descriptor) = select_abi_for_scan(root);
        let (phase, descriptor_kind, capabilities) = match descriptor {
            Some(d) => (PluginBootstrapPhase::Engine, Some(d.kind), d.capabilities.len()),
            None => (PluginBootstrapPhase::Engine, None, 0),
        };
        return Ok(ScannedDynlib {
            path: path.to_path_buf(),
            file_name,
            kind: ScannedDynlibKind::Plugin {
                id: info.id.to_string(),
                version: info.version.to_string(),
                phase,
                descriptor_kind,
                capabilities,
            },
        });
    }

    Ok(ScannedDynlib {
        path: path.to_path_buf(),
        file_name,
        kind: ScannedDynlibKind::Unknown,
    })
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