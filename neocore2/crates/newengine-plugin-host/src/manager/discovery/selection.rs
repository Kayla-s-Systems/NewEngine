#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections_prelude::NeHashMap as HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

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
    DisabledByConfig,
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
            | Self::DisabledByConfig
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
            Self::DisabledByConfig => "disabled by config".to_owned(),
            Self::Filtered { filter_label } => format!("filtered by {filter_label}"),
            Self::DuplicateId { winner_file } => {
                format!("duplicate plugin id, winner='{winner_file}'")
            }
        }
    }
}

#[inline]
fn plugin_excluded_by_host_policy(id: &str) -> bool {
    std::env::var("NEWENGINE_PLUGIN_EXCLUDE_IDS")
        .ok()
        .map(|value| {
            value
                .split(|ch: char| ch == ',' || ch == ';' || ch.is_ascii_whitespace())
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .any(|entry| entry == id)
        })
        .unwrap_or(false)
}

fn headless_mode_enabled() -> bool {
    std::env::var("NEWENGINE_HEADLESS")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

static HEADLESS_PROVIDER_SKIP_MESSAGE_PRINTED: AtomicBool = AtomicBool::new(false);

#[inline]
fn is_concrete_provider_id(id: &str) -> bool {
    let id = id.to_ascii_lowercase();
    !(id.contains(".null") || id.contains("null"))
}

#[inline]
fn headless_native_provider_domain(id: &str) -> Option<&'static str> {
    let id = id.trim().to_ascii_lowercase();
    [
        ("engine.platform.", "platform"),
        ("engine.render.", "render"),
        ("engine.ui.", "ui"),
        ("engine.input.", "input"),
        ("engine.logging.", "logging"),
    ]
    .into_iter()
    .find_map(|(prefix, domain)| id.starts_with(prefix).then_some(domain))
}

#[inline]
fn gateway_is_native_headless_domain(gateway: &str) -> bool {
    matches!(
        gateway,
        "engine.platform"
            | "engine.render"
            | "engine.ui"
            | "engine.ui.text"
            | "engine.input"
            | "engine.logging"
    )
}

#[inline]
fn headless_skips_native_provider_for_mode(
    headless: bool,
    id: &str,
    service_gateways: &[String],
) -> bool {
    headless
        && is_concrete_provider_id(id)
        && (headless_native_provider_domain(id).is_some()
            || service_gateways
                .iter()
                .any(|gateway| gateway_is_native_headless_domain(gateway)))
}

fn emit_headless_skip_summary_once(graph: &DiscoveryGraph) {
    if !headless_mode_enabled()
        || HEADLESS_PROVIDER_SKIP_MESSAGE_PRINTED.swap(true, Ordering::AcqRel)
    {
        return;
    }

    let mut skipped = graph
        .items
        .iter()
        .filter_map(|item| {
            let ScannedDynlibKind::Plugin {
                id,
                service_gateways,
                ..
            } = &item.kind
            else {
                return None;
            };
            headless_skips_native_provider_for_mode(true, id, service_gateways).then(|| {
                format!(
                    "{}:{}",
                    headless_native_provider_domain(id).unwrap_or("gateway"),
                    id
                )
            })
        })
        .collect::<Vec<_>>();
    skipped.sort();
    skipped.dedup();

    if !skipped.is_empty() {
        eprintln!(
            "[HEADLESS] Native device providers skipped before initialization: {}",
            skipped.join(", ")
        );
    }
}

fn headless_skips_native_provider(id: &str, service_gateways: &[String]) -> bool {
    headless_skips_native_provider_for_mode(headless_mode_enabled(), id, service_gateways)
}

pub(super) fn build_load_selection(
    graph: &DiscoveryGraph,
    filter: LoadPhaseFilter,
    loaded_ids: &NeHashSet<String>,
) -> LoadSelection {
    emit_headless_skip_summary_once(graph);

    let mut out = LoadSelection::default();
    let mut winners_by_id: HashMap<&str, &super::graph::ScannedDynlib> = HashMap::default();

    // First pass: choose one deterministic winner per plugin id.
    // The scan order is filesystem-dependent, so duplicate descriptors must be
    // resolved deterministically by descriptor version and build profile.
    for item in &graph.items {
        let ScannedDynlibKind::Plugin {
            id,
            phase,
            descriptor_kind: _,
            service_gateways,
            ..
        } = &item.kind
        else {
            continue;
        };
        if loaded_ids.contains(id)
            || !crate::plugin_config_service::plugin_enabled_by_config(id)
            || !filter.allows(*phase)
            || headless_skips_native_provider(id, service_gateways)
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
            ScannedDynlibKind::PlatformRuntime { .. } => {
                SelectionDecision::Runtime { label: "platform" }
            }
            ScannedDynlibKind::Unknown => SelectionDecision::Unknown,
            ScannedDynlibKind::Plugin {
                id,
                phase,
                descriptor_kind: _,
                service_gateways,
                ..
            } => {
                if loaded_ids.contains(id) {
                    SelectionDecision::AlreadyLoaded
                } else if plugin_excluded_by_host_policy(id) {
                    SelectionDecision::Unsupported {
                        reason: "plugin id excluded by host composition policy",
                    }
                } else if !crate::plugin_config_service::plugin_enabled_by_config(id) {
                    SelectionDecision::DisabledByConfig
                } else if headless_skips_native_provider(id, service_gateways) {
                    SelectionDecision::Unsupported {
                        reason: "headless mode owns platform/render/ui/input/logging through host or null routes",
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
        let ScannedDynlibKind::Plugin {
            service_gateways, ..
        } = &item.kind
        else {
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
        ScannedDynlibKind::Plugin {
            backend_priority, ..
        } => *backend_priority,
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

fn plugin_candidate_rank(
    item: &super::graph::ScannedDynlib,
) -> ((u64, u64, u64, u64), i32, usize, String) {
    let (version, backend_priority, declared_capabilities) = match &item.kind {
        ScannedDynlibKind::Plugin {
            version,
            backend_priority,
            declared_capabilities,
            ..
        } => (
            semver_rank(version),
            *backend_priority,
            declared_capabilities.unwrap_or(0),
        ),
        _ => ((0, 0, 0, 0), 0, 0),
    };

    // The final path string is only a deterministic tie-breaker for two descriptors
    // with equal id/version/capability metadata. It is not used for plugin identity.
    (
        version,
        backend_priority,
        declared_capabilities,
        item.path.to_string_lossy().to_string(),
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_tooling_plugin_remains_selectable_for_game_runtime_target() {
        static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = ENV_LOCK.lock().unwrap();
        let previous_target = std::env::var_os("NEWENGINE_PLUGIN_TARGET");
        let previous_headless = std::env::var_os("NEWENGINE_HEADLESS");
        std::env::set_var("NEWENGINE_PLUGIN_TARGET", "game");
        std::env::remove_var("NEWENGINE_HEADLESS");

        let path = PathBuf::from("editing-tools-0.3.0-release.dll");
        let graph = DiscoveryGraph {
            dir: PathBuf::from("pluginsRuntime"),
            entries_total: 1,
            skipped_non_dynlib: 0,
            items: vec![super::super::graph::ScannedDynlib {
                path: path.clone(),
                file_name: "editing-tools-0.3.0-release.dll".to_owned(),
                kind: ScannedDynlibKind::Plugin {
                    id: "newengine.editing.tools".to_owned(),
                    version: "0.3.0".to_owned(),
                    phase: newengine_plugin_api::PluginBootstrapPhase::Engine,
                    descriptor_kind: Some(newengine_plugin_api::PluginKind::Editor),
                    declared_capabilities: Some(1),
                    service_gateways: Vec::new(),
                    backend_priority: 0,
                },
            }],
            scan_errors: Vec::new(),
            platform_runtime_count: 0,
            bootstrap_total: 0,
            engine_total: 1,
            unknown_dynlibs: Vec::new(),
        };
        let selection = build_load_selection(
            &graph,
            LoadPhaseFilter::BootstrapAndEngine,
            &NeHashSet::default(),
        );

        if let Some(value) = previous_target {
            std::env::set_var("NEWENGINE_PLUGIN_TARGET", value);
        } else {
            std::env::remove_var("NEWENGINE_PLUGIN_TARGET");
        }
        if let Some(value) = previous_headless {
            std::env::set_var("NEWENGINE_HEADLESS", value);
        } else {
            std::env::remove_var("NEWENGINE_HEADLESS");
        }

        assert!(selection.engine_candidates.contains(&path));
        assert!(selection
            .decisions
            .get(&path)
            .is_some_and(SelectionDecision::is_selected));
    }

    #[test]
    fn signature_only_native_provider_ids_are_skipped_in_headless_mode() {
        let no_gateways = Vec::<String>::new();
        for id in [
            "engine.platform.winit",
            "engine.render.vulkan",
            "engine.ui.aurelia",
            "engine.input.compass",
            "engine.logging.chronicle",
        ] {
            assert!(
                headless_skips_native_provider_for_mode(true, id, &no_gateways),
                "expected headless skip for {id}"
            );
        }
    }

    #[test]
    fn simulation_and_asset_plugins_remain_available_in_headless_mode() {
        let no_gateways = Vec::<String>::new();
        for id in [
            "engine.assets.starvault",
            "engine.ecs.constellation",
            "engine.physics.gravitas",
            "engine.profiler.starprofiler",
        ] {
            assert!(
                !headless_skips_native_provider_for_mode(true, id, &no_gateways),
                "unexpected headless skip for {id}"
            );
        }
    }

    #[test]
    fn gateway_metadata_still_skips_custom_native_providers() {
        assert!(headless_skips_native_provider_for_mode(
            true,
            "vendor.custom-backend",
            &["engine.render".to_owned()]
        ));
        assert!(!headless_skips_native_provider_for_mode(
            false,
            "engine.render.vulkan",
            &[]
        ));
        assert!(!headless_skips_native_provider_for_mode(
            true,
            "engine.render.null",
            &["engine.render".to_owned()]
        ));
    }
}
