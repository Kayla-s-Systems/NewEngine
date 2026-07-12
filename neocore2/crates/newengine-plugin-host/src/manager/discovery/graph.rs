#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;

use newengine_plugin_api::{PluginBootstrapPhase, PluginKind};

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
        service_gateways: Vec<String>,
        backend_priority: i32,
    },
    Unknown,
}

#[derive(Debug, Clone)]
pub(super) struct ScannedDynlib {
    pub(super) path: PathBuf,
    pub(super) file_name: String,
    pub(super) kind: ScannedDynlibKind,
}

#[derive(Debug, Clone)]
pub struct DiscoveryGraph {
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

#[derive(Copy, Clone)]
pub(super) enum LoadPhaseFilter {
    All,
    BootstrapOnly,
    /// Runtime/engine load stage. Includes bootstrap plugins when early
    /// bootstrap preloading is deferred, but still excludes platform runtimes.
    BootstrapAndEngine,
}

impl LoadPhaseFilter {
    #[inline]
    pub(super) fn allows(self, phase: PluginBootstrapPhase) -> bool {
        match self {
            Self::All => true,
            Self::BootstrapOnly => matches!(phase, PluginBootstrapPhase::Bootstrap),
            Self::BootstrapAndEngine => matches!(
                phase,
                PluginBootstrapPhase::Bootstrap | PluginBootstrapPhase::Engine
            ),
        }
    }

    #[inline]
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::BootstrapOnly => "bootstrap-only",
            Self::BootstrapAndEngine => "bootstrap+engine",
        }
    }
}

#[inline]
pub(super) fn phase_name(phase: PluginBootstrapPhase) -> &'static str {
    match phase {
        PluginBootstrapPhase::Bootstrap => "bootstrap",
        PluginBootstrapPhase::Platform => "platform",
        PluginBootstrapPhase::Engine => "engine",
    }
}
