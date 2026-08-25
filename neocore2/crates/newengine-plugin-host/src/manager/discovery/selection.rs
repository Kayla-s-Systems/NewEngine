#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections_prelude::NeHashMap as HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use newengine_math::collections::prelude::NeHashSet;
use newengine_service_api::{CompositionCandidate, CompositionPlan, CompositionSolver};

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
        || crate::host_context::ctx()
            .headless_provider_skip_message_printed
            .swap(true, Ordering::AcqRel)
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

/// Converts discovery inventory into a load set.
///
/// Discovery itself never chooses a gateway provider. Duplicate binary identity
/// is resolved here as an artifact concern; gateway provider winner selection is
/// delegated exclusively to CompositionSolver.
pub(super) fn build_load_selection(
    graph: &DiscoveryGraph,
    filter: LoadPhaseFilter,
    loaded_ids: &NeHashSet<String>,
) -> LoadSelection {
    emit_headless_skip_summary_once(graph);

    let mut out = LoadSelection::default();
    let mut winners_by_id: HashMap<&str, &super::graph::ScannedDynlib> = HashMap::default();

    // First pass only deduplicates physical artifacts for one logical plugin id.
    // Provider priority is intentionally NOT part of this rank; that belongs to
    // CompositionSolver and nowhere else.
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
            || plugin_excluded_by_host_policy(id)
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

    let composition_plan = resolve_preload_composition_plan(&winners_by_id);

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
                } else if preload_provider_is_shadowed(&composition_plan, item, service_gateways) {
                    SelectionDecision::Filtered {
                        filter_label: "composition-plan",
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

/// Resolve provider hints captured by discovery inventory through the same
/// `CompositionSolver` that produces the live registry's immutable plan. Signature-only discovery
/// legitimately produces no gateway candidates; in that case the plan is empty
/// and the loader does not guess a winner from filenames or plugin ids.
fn resolve_preload_composition_plan(
    winners_by_id: &HashMap<&str, &super::graph::ScannedDynlib>,
) -> CompositionPlan {
    let candidates = winners_by_id.values().copied().flat_map(|item| {
        let ScannedDynlibKind::Plugin {
            id,
            service_gateways,
            backend_priority,
            ..
        } = &item.kind
        else {
            return Vec::new().into_iter();
        };

        let origin = crate::service_gateway::GatewayProviderOrigin::from_plugin_path(&item.path);
        let candidate_id = item.path.to_string_lossy().into_owned();
        service_gateways
            .iter()
            .map(|gateway| {
                CompositionCandidate::new(
                    gateway.clone(),
                    candidate_id.clone(),
                    id.clone(),
                    *backend_priority,
                    origin.origin_bias(),
                    0,
                )
            })
            .collect::<Vec<_>>()
            .into_iter()
    });

    CompositionSolver::resolve(candidates)
}

#[inline]
fn preload_provider_is_shadowed(
    plan: &CompositionPlan,
    item: &super::graph::ScannedDynlib,
    service_gateways: &[String],
) -> bool {
    if service_gateways.is_empty() {
        return false;
    }
    let candidate_id = item.path.to_string_lossy();
    // A DLL can publish more than one gateway. It must remain loadable when it
    // wins at least one of them; otherwise pre-load filtering could remove a
    // provider that the immutable plan actually selected for another gateway.
    !service_gateways.iter().any(|gateway| {
        plan.selected(gateway)
            .is_some_and(|selected| selected.candidate_id.as_str() == candidate_id.as_ref())
    })
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
) -> ((u64, u64, u64, u64), usize, String) {
    let (version, declared_capabilities) = match &item.kind {
        ScannedDynlibKind::Plugin {
            version,
            declared_capabilities,
            ..
        } => (semver_rank(version), declared_capabilities.unwrap_or(0)),
        _ => ((0, 0, 0, 0), 0),
    };

    // The final path string is only a deterministic tie-breaker for two artifacts
    // with equal id/version/capability metadata. It is not provider selection.
    (
        version,
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

    fn plugin_item(
        path: &str,
        id: &str,
        gateway: Option<&str>,
        priority: i32,
    ) -> super::super::graph::ScannedDynlib {
        super::super::graph::ScannedDynlib {
            path: PathBuf::from(path),
            file_name: path.to_owned(),
            kind: ScannedDynlibKind::Plugin {
                id: id.to_owned(),
                version: "1.0.0".to_owned(),
                phase: newengine_plugin_api::PluginBootstrapPhase::Engine,
                descriptor_kind: Some(newengine_plugin_api::PluginKind::Runtime),
                declared_capabilities: gateway.map(|_| 1),
                service_gateways: gateway.into_iter().map(str::to_owned).collect(),
                backend_priority: priority,
            },
        }
    }

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
    fn preload_gateway_provider_selection_uses_composition_solver() {
        let low = plugin_item(
            "plugins/engine.render.low-1.0.0.dll",
            "engine.render.low",
            Some("engine.render"),
            10,
        );
        let high = plugin_item(
            "plugins/engine.render.high-1.0.0.dll",
            "engine.render.high",
            Some("engine.render"),
            20,
        );
        let high_path = high.path.clone();
        let low_path = low.path.clone();
        let graph = DiscoveryGraph {
            dir: PathBuf::from("pluginsRuntime"),
            entries_total: 2,
            skipped_non_dynlib: 0,
            items: vec![low, high],
            scan_errors: Vec::new(),
            platform_runtime_count: 0,
            bootstrap_total: 0,
            engine_total: 2,
            unknown_dynlibs: Vec::new(),
        };

        let selection = build_load_selection(
            &graph,
            LoadPhaseFilter::BootstrapAndEngine,
            &NeHashSet::default(),
        );

        assert!(selection.engine_candidates.contains(&high_path));
        assert!(!selection.engine_candidates.contains(&low_path));
        assert!(matches!(
            selection.decisions.get(&low_path),
            Some(SelectionDecision::Filtered {
                filter_label: "composition-plan"
            })
        ));
    }

    #[test]
    fn multi_gateway_plugin_stays_loadable_when_plan_selects_any_route() {
        let multi_path = PathBuf::from("plugins/multi-provider.dll");
        let render_path = PathBuf::from("plugins/render-specialist.dll");
        let multi = super::super::graph::ScannedDynlib {
            path: multi_path.clone(),
            file_name: "multi-provider.dll".to_owned(),
            kind: ScannedDynlibKind::Plugin {
                id: "engine.multi.provider".to_owned(),
                version: "1.0.0".to_owned(),
                phase: newengine_plugin_api::PluginBootstrapPhase::Engine,
                descriptor_kind: Some(newengine_plugin_api::PluginKind::Runtime),
                declared_capabilities: Some(2),
                service_gateways: vec!["engine.render".to_owned(), "engine.audio".to_owned()],
                backend_priority: 20,
            },
        };
        let render = plugin_item(
            "plugins/render-specialist.dll",
            "engine.render.specialist",
            Some("engine.render"),
            30,
        );
        let graph = DiscoveryGraph {
            dir: PathBuf::from("pluginsRuntime"),
            entries_total: 2,
            skipped_non_dynlib: 0,
            items: vec![multi, render],
            scan_errors: Vec::new(),
            platform_runtime_count: 0,
            bootstrap_total: 0,
            engine_total: 2,
            unknown_dynlibs: Vec::new(),
        };

        let selection = build_load_selection(
            &graph,
            LoadPhaseFilter::BootstrapAndEngine,
            &NeHashSet::default(),
        );

        assert!(selection.engine_candidates.contains(&multi_path));
        assert!(selection.engine_candidates.contains(&render_path));
    }

    #[test]
    fn duplicate_artifact_rank_does_not_use_provider_priority() {
        let low_priority_newer = super::super::graph::ScannedDynlib {
            path: PathBuf::from("provider-2.0.0.dll"),
            file_name: "provider-2.0.0.dll".to_owned(),
            kind: ScannedDynlibKind::Plugin {
                id: "engine.render.provider".to_owned(),
                version: "2.0.0".to_owned(),
                phase: newengine_plugin_api::PluginBootstrapPhase::Engine,
                descriptor_kind: Some(newengine_plugin_api::PluginKind::Runtime),
                declared_capabilities: Some(1),
                service_gateways: vec!["engine.render".to_owned()],
                backend_priority: -100,
            },
        };
        let high_priority_older = super::super::graph::ScannedDynlib {
            path: PathBuf::from("provider-1.0.0.dll"),
            file_name: "provider-1.0.0.dll".to_owned(),
            kind: ScannedDynlibKind::Plugin {
                id: "engine.render.provider".to_owned(),
                version: "1.0.0".to_owned(),
                phase: newengine_plugin_api::PluginBootstrapPhase::Engine,
                descriptor_kind: Some(newengine_plugin_api::PluginKind::Runtime),
                declared_capabilities: Some(1),
                service_gateways: vec!["engine.render".to_owned()],
                backend_priority: 10_000,
            },
        };

        assert!(is_better_plugin_candidate(
            &low_priority_newer,
            &high_priority_older
        ));
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
