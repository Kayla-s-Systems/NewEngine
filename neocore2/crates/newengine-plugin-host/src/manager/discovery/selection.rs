#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;
use std::path::PathBuf;

use newengine_math::collections::prelude::NeHashSet;

use super::graph::{DiscoveryGraph, LoadPhaseFilter, ScannedDynlibKind};

#[derive(Debug, Clone, Default)]
pub(super) struct LoadSelection {
    pub(super) bootstrap_candidates: Vec<PathBuf>,
    pub(super) engine_candidates: Vec<PathBuf>,
    pub(super) decisions: HashMap<PathBuf, SelectionDecision>,
}

#[derive(Debug, Clone)]
pub(super) enum SelectionDecision {
    Selected,
    Runtime { label: &'static str },
    Unsupported { reason: &'static str },
    Unknown,
    AlreadyLoaded,
    Filtered { filter_label: &'static str },
    DuplicateId { winner_file: String },
}

impl SelectionDecision {
    #[inline]
    pub(super) fn is_selected(&self) -> bool {
        matches!(self, Self::Selected)
    }

    #[inline]
    pub(super) fn is_runtime(&self) -> bool {
        matches!(self, Self::Runtime { .. })
    }

    #[inline]
    pub(super) fn is_duplicate(&self) -> bool {
        matches!(self, Self::DuplicateId { .. })
    }

    #[inline]
    pub(super) fn selected_label(&self) -> &'static str {
        match self {
            Self::Selected => "yes",
            Self::Runtime { .. } => "runtime",
            Self::Unsupported { .. }
            | Self::Unknown
            | Self::AlreadyLoaded
            | Self::Filtered { .. }
            | Self::DuplicateId { .. } => "no",
        }
    }

    #[inline]
    pub(super) fn reason_label(&self) -> String {
        match self {
            Self::Selected => "phase match".to_owned(),
            Self::Runtime { label } => format!("{label} runtime"),
            Self::Unsupported { reason } => (*reason).to_owned(),
            Self::Unknown => "unknown dynlib".to_owned(),
            Self::AlreadyLoaded => "already loaded".to_owned(),
            Self::Filtered { filter_label } => format!("filtered by {filter_label}"),
            Self::DuplicateId { winner_file } => {
                format!("duplicate plugin id, winner='{winner_file}'")
            }
        }
    }
}

pub(super) fn build_load_selection(
    graph: &DiscoveryGraph,
    filter: LoadPhaseFilter,
    loaded_ids: &NeHashSet<String>,
) -> LoadSelection {
    let mut out = LoadSelection::default();
    let mut selected_by_id: HashMap<&str, String> = HashMap::new();

    for item in &graph.items {
        let decision = match &item.kind {
            ScannedDynlibKind::PlatformRuntime { .. } => SelectionDecision::Runtime {
                label: "platform",
            },
            ScannedDynlibKind::LegacyRenderBackend { .. } => SelectionDecision::Unsupported {
                reason: "legacy render backend ABI without plugin root",
            },
            ScannedDynlibKind::Unknown => SelectionDecision::Unknown,
            ScannedDynlibKind::Plugin { id, phase, .. } => {
                if loaded_ids.contains(id) {
                    SelectionDecision::AlreadyLoaded
                } else if !filter.allows(*phase) {
                    SelectionDecision::Filtered {
                        filter_label: filter.label(),
                    }
                } else if let Some(winner_file) = selected_by_id.get(id.as_str()) {
                    SelectionDecision::DuplicateId {
                        winner_file: winner_file.clone(),
                    }
                } else {
                    selected_by_id.insert(id.as_str(), item.file_name.clone());
                    match phase {
                        newengine_plugin_api::PluginBootstrapPhase::Bootstrap => {
                            out.bootstrap_candidates.push(item.path.clone());
                        }
                        newengine_plugin_api::PluginBootstrapPhase::Platform
                        | newengine_plugin_api::PluginBootstrapPhase::Engine => {
                            out.engine_candidates.push(item.path.clone());
                        }
                    }
                    SelectionDecision::Selected
                }
            }
        };

        out.decisions.insert(item.path.clone(), decision);
    }

    out
}
