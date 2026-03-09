#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;

use newengine_plugin_api::{PluginBootstrapPhase, PluginKind};

#[derive(Debug, Clone)]
pub(super) enum ScannedDynlibKind {
    PlatformRuntime {
        id: String,
        version: String,
    },
    RenderBackend {
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
    pub(super) path: PathBuf,
    pub(super) file_name: String,
    pub(super) kind: ScannedDynlibKind,
}

#[derive(Debug, Clone)]
pub(in crate::manager) struct DiscoveryGraph {
    pub(super) dir: PathBuf,
    pub(super) entries_total: usize,
    pub(super) skipped_non_dynlib: usize,
    pub(super) items: Vec<ScannedDynlib>,
    pub(super) scan_errors: Vec<String>,
    pub(super) platform_runtime_count: usize,
    pub(super) render_backend_count: usize,
    pub(super) bootstrap_total: usize,
    pub(super) engine_total: usize,
    pub(super) unknown_dynlibs: Vec<String>,
}

#[derive(Copy, Clone)]
pub(super) enum LoadPhaseFilter {
    All,
    BootstrapOnly,
    EngineOnly,
}

impl LoadPhaseFilter {
    #[inline]
    pub(super) fn allows(self, phase: PluginBootstrapPhase) -> bool {
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
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::BootstrapOnly => "bootstrap-only",
            Self::EngineOnly => "engine-only",
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
