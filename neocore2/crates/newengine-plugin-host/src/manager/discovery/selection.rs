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
    // The scan order is filename-based, so without this pass an older DLL such as
    // `vulkan_renderer-0.3.2-release.dll` can shadow a freshly built
    // `vulkan_renderer-0.3.3-dev.dll`. Runtime discovery must prefer the newest
    // descriptor version, then the strongest build profile.
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

    let selected_render_backend_path = select_render_backend_provider(&winners_by_id);
    let selected_physics_backend_path = select_physics_backend_provider(&winners_by_id);

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
                provides_render_backend,
                provides_render_service,
                provides_physics_backend,
                provides_physics_service,
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
                } else if *provides_render_backend && *provides_render_service
                    && selected_render_backend_path.as_ref() != Some(&item.path)
                {
                    SelectionDecision::Filtered {
                        filter_label: "render-provider-selection",
                    }
                } else if *provides_physics_backend && *provides_physics_service
                    && selected_physics_backend_path.as_ref() != Some(&item.path)
                {
                    SelectionDecision::Filtered {
                        filter_label: "physics-provider-selection",
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

fn select_render_backend_provider(
    winners_by_id: &HashMap<&str, &super::graph::ScannedDynlib>,
) -> Option<std::path::PathBuf> {
    let mut candidates: Vec<&super::graph::ScannedDynlib> = winners_by_id
        .values()
        .copied()
        .filter(|item| match &item.kind {
            ScannedDynlibKind::Plugin {
                provides_render_backend,
                provides_render_service,
                ..
            } => *provides_render_backend && *provides_render_service,
            _ => false,
        })
        .collect();

    candidates.sort_by(|a, b| {
        render_provider_priority(a)
            .cmp(&render_provider_priority(b))
            .then_with(|| plugin_candidate_rank(a).cmp(&plugin_candidate_rank(b)))
    });

    candidates.last().map(|item| item.path.clone())
}

fn render_provider_priority(item: &super::graph::ScannedDynlib) -> u8 {
    match &item.kind {
        ScannedDynlibKind::Plugin { id, .. } if id == "newengine.renderer.null" => 0,
        ScannedDynlibKind::Plugin { .. } => 1,
        _ => 0,
    }
}

fn select_physics_backend_provider(
    winners_by_id: &HashMap<&str, &super::graph::ScannedDynlib>,
) -> Option<std::path::PathBuf> {
    let mut candidates: Vec<&super::graph::ScannedDynlib> = winners_by_id
        .values()
        .copied()
        .filter(|item| match &item.kind {
            ScannedDynlibKind::Plugin {
                provides_physics_backend,
                provides_physics_service,
                ..
            } => *provides_physics_backend && *provides_physics_service,
            _ => false,
        })
        .collect();

    candidates.sort_by(|a, b| {
        physics_provider_priority(a)
            .cmp(&physics_provider_priority(b))
            .then_with(|| plugin_candidate_rank(a).cmp(&plugin_candidate_rank(b)))
    });

    candidates.last().map(|item| item.path.clone())
}

fn physics_provider_priority(item: &super::graph::ScannedDynlib) -> u8 {
    match &item.kind {
        ScannedDynlibKind::Plugin { id, .. } if id == "newengine.physics.null" => 0,
        ScannedDynlibKind::Plugin { id, .. } if id == "newengine.physics.deterministic" => 1,
        ScannedDynlibKind::Plugin { id, .. } if id == "newengine.physics.jolt" => 2,
        ScannedDynlibKind::Plugin { .. } => 1,
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

fn plugin_candidate_rank(item: &super::graph::ScannedDynlib) -> ((u64, u64, u64, u64), u8, String) {
    let version = match &item.kind {
        ScannedDynlibKind::Plugin { version, .. } => semver_rank(version),
        _ => (0, 0, 0, 0),
    };

    (version, build_profile_rank(&item.file_name), item.file_name.clone())
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

fn build_profile_rank(file_name: &str) -> u8 {
    let lower = file_name.to_ascii_lowercase();
    let actual = if lower.contains("-release.") || lower.contains("-release-") {
        "release"
    } else if lower.contains("-dev.") || lower.contains("-dev-") {
        "dev"
    } else if lower.contains("-debug.") || lower.contains("-debug-") {
        "debug"
    } else if lower.contains("-test.") || lower.contains("-test-") {
        "test"
    } else if lower.contains("-bench.") || lower.contains("-bench-") {
        "bench"
    } else {
        "unknown"
    };

    let desired = if cfg!(debug_assertions) { "dev" } else { "release" };
    if actual == desired {
        5
    } else {
        match actual {
            "dev" => 4,
            "debug" => 3,
            "release" => 2,
            "test" => 1,
            "bench" | "unknown" => 0,
            _ => 0,
        }
    }
}
