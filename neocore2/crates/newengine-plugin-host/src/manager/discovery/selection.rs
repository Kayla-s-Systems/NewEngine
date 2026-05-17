#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections_prelude::NeHashMap as HashMap;
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

#[inline]
fn runtime_target_plugins_only() -> bool {
    std::env::var("NEWENGINE_PLUGIN_TARGET")
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "runtime" | "game" | "standalone"))
        .unwrap_or(false)
}

#[inline]
fn is_editor_only_plugin(kind: Option<newengine_plugin_api::PluginKind>) -> bool {
    matches!(kind, Some(newengine_plugin_api::PluginKind::Editor))
}

pub(super) fn build_load_selection(
    graph: &DiscoveryGraph,
    filter: LoadPhaseFilter,
    loaded_ids: &NeHashSet<String>,
) -> LoadSelection {
    let mut out = LoadSelection::default();
    let runtime_only = runtime_target_plugins_only();
    let mut winners_by_id: HashMap<&str, &super::graph::ScannedDynlib> = HashMap::default();

    // First pass: choose one deterministic winner per plugin id.
    // The scan order is filesystem-dependent, so duplicate descriptors must be
    // resolved deterministically by descriptor version and build profile.
    for item in &graph.items {
        let ScannedDynlibKind::Plugin {
            id,
            phase,
            descriptor_kind,
            ..
        } = &item.kind else {
            continue;
        };
        if loaded_ids.contains(id)
            || !filter.allows(*phase)
            || (runtime_only && is_editor_only_plugin(*descriptor_kind))
        {
            continue;
        }

        match winners_by_id.get(id.as_str()).copied() {
            Some(current) if is_better_plugin_candidate(item, current) => {
                winners_by_id.insert(id.as_str(), item);
            }
            None => {
                winners_by_id.insert(id.as_str(), item);
            }
            _ => {}
        }
    }

    let selected_gateway_provider_paths = select_gateway_provider_paths(&winners_by_id);

    for item in &graph.items {
        let decision = match &item.kind {
            ScannedDynlibKind::PlatformRuntime { .. } => SelectionDecision::Runtime {
                label: "platform",
            },
            ScannedDynlibKind::Unknown => SelectionDecision::Unknown,
            ScannedDynlibKind::Plugin {
                id,
                phase,
                descriptor_kind,
                service_gateways,
                ..
            } => {
                if loaded_ids.contains(id) {
                    SelectionDecision::AlreadyLoaded
                } else if runtime_only && is_editor_only_plugin(*descriptor_kind) {
                    SelectionDecision::Unsupported {
                        reason: "editor plugin disabled for runtime target",
                    }
                } else if !filter.allows(*phase) {
                    SelectionDecision::Filtered {
                        filter_label: filter.label(),
                    }
                } else if service_gateways.iter().any(|gateway| {
                    selected_gateway_provider_paths
                        .get(gateway.as_str())
                        .is_some_and(|selected_path| selected_path != &item.path)
                }) {
                    SelectionDecision::Filtered {
                        filter_label: "gateway-provider-selection",
                    }
                } else if let Some(winner) = winners_by_id.get(id.as_str()).copied() {
                    if winner.path == item.path {
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
                    } else {
                        SelectionDecision::DuplicateId {
                            winner_file: winner.file_name.clone(),
                        }
                    }
                } else {
                    SelectionDecision::Unknown
                }
            }
        };

        out.decisions.insert(item.path.clone(), decision);
    }

    out
}

fn select_gateway_provider_paths(
    winners_by_id: &HashMap<&str, &super::graph::ScannedDynlib>,
) -> HashMap<String, std::path::PathBuf> {
    let mut by_gateway: HashMap<String, Vec<&super::graph::ScannedDynlib>> = HashMap::default();

    for item in winners_by_id.values().copied() {
        let ScannedDynlibKind::Plugin { service_gateways, .. } = &item.kind else {
            continue;
        };
        for gateway in service_gateways {
            by_gateway.entry(gateway.clone()).or_default().push(item);
        }
    }

    let mut selected = HashMap::default();
    for (gateway, mut candidates) in by_gateway {
        candidates.sort_by(|a, b| {
            backend_provider_priority(a)
                .cmp(&backend_provider_priority(b))
                .then_with(|| plugin_candidate_rank(a).cmp(&plugin_candidate_rank(b)))
        });
        if let Some(item) = candidates.last() {
            selected.insert(gateway, item.path.clone());
        }
    }

    selected
}

fn backend_provider_priority(item: &super::graph::ScannedDynlib) -> i32 {
    match &item.kind {
        ScannedDynlibKind::Plugin { backend_priority, .. } => *backend_priority,
        _ => 0,
    }
}

fn is_better_plugin_candidate(
    candidate: &super::graph::ScannedDynlib,
    current: &super::graph::ScannedDynlib,
) -> bool {
    let candidate_rank = plugin_candidate_rank(candidate);
    let current_rank = plugin_candidate_rank(current);
    candidate_rank > current_rank
}

fn plugin_candidate_rank(item: &super::graph::ScannedDynlib) -> ((u64, u64, u64, u64), i32, usize, String) {
    let (version, backend_priority, declared_capabilities) = match &item.kind {
        ScannedDynlibKind::Plugin {
            version,
            backend_priority,
            declared_capabilities,
            ..
        } => (semver_rank(version), *backend_priority, declared_capabilities.unwrap_or(0)),
        _ => ((0, 0, 0, 0), 0, 0),
    };

    // The final path string is only a deterministic tie-breaker for two descriptors
    // with equal id/version/capability metadata. It is not used for plugin identity.
    (version, backend_priority, declared_capabilities, item.path.to_string_lossy().to_string())
}

fn semver_rank(version: &str) -> (u64, u64, u64, u64) {
    let core = version
        .split_once('+')
        .map(|(l, _)| l)
        .unwrap_or(version)
        .split_once('-')
        .map(|(l, _)| l)
        .unwrap_or(version);

    let mut parts = core.split('.').map(|part| part.parse::<u64>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}
