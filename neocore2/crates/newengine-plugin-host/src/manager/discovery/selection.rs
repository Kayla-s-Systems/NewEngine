#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::collections_prelude::NeHashMap as HashMap;
use std::path::{Path, PathBuf};

use newengine_math::collections::prelude::NeHashSet;
use newengine_service_api::{CompositionPlan, CompositionSolver, CompositionSolverInput};

use super::graph::{DiscoveryGraph, LoadPhaseFilter, ScannedDynlibKind};
use crate::manager::types::PluginLoadOrigin;

#[derive(Debug, Clone)]
pub struct FrozenPluginCompositionPlan {
    pub(super) plan: CompositionPlan,
    artifact_winners: HashMap<String, PathBuf>,
    /// Verified metadata snapshot keyed by the exact winning artifact path.
    /// Loader/materializer must use this snapshot rather than re-reading a mutable sidecar.
    artifact_manifests: HashMap<PathBuf, super::sidecar::VerifiedPluginDiscoveryManifest>,
    provider_paths: NeHashSet<PathBuf>,
    selected_provider_paths: NeHashSet<PathBuf>,
    forbidden_system_tags: Vec<String>,
}

impl FrozenPluginCompositionPlan {
    #[inline]
    fn artifact_winner(&self, plugin_id: &str) -> Option<&PathBuf> {
        self.artifact_winners.get(plugin_id)
    }

    #[inline]
    pub(crate) fn artifact_manifest(
        &self,
        path: &Path,
    ) -> Option<&super::sidecar::VerifiedPluginDiscoveryManifest> {
        self.artifact_manifests.get(path)
    }

    #[inline]
    fn provider_is_shadowed(&self, path: &PathBuf) -> bool {
        self.provider_paths.contains(path) && !self.selected_provider_paths.contains(path)
    }

    #[inline]
    fn system_tags_allowed(&self, tags: &[String]) -> bool {
        !self
            .forbidden_system_tags
            .iter()
            .any(|forbidden| tags.iter().any(|tag| tag == forbidden))
    }
}

pub fn build_frozen_composition_plan(
    inventories: &[(DiscoveryGraph, PluginLoadOrigin)],
    planning: &crate::host_context::CompositionPlanningSnapshot,
) -> FrozenPluginCompositionPlan {
    let mut artifact_winners: HashMap<String, (super::graph::ScannedDynlib, PluginLoadOrigin)> =
        HashMap::default();

    for (graph, load_origin) in inventories {
        for item in &graph.items {
            let ScannedDynlibKind::Plugin { id, .. } = &item.kind else {
                continue;
            };
            if plugin_excluded_by_host_policy(id)
                || !crate::plugin_config_service::plugin_enabled_by_config(id)
            {
                continue;
            }
            match artifact_winners.get(id) {
                Some((current, _)) if is_better_plugin_candidate(item, current) => {
                    artifact_winners.insert(id.clone(), (item.clone(), *load_origin));
                }
                None => {
                    artifact_winners.insert(id.clone(), (item.clone(), *load_origin));
                }
                _ => {}
            }
        }
    }

    let mut candidates = crate::service_gateway::host_route_composition_candidates(
        &planning.services,
        &planning.gateway_provider_routes,
        &planning.selection_policies,
    );
    let mut provider_paths = NeHashSet::default();
    let mut candidate_paths: HashMap<String, PathBuf> = HashMap::default();
    let mut winner_paths: HashMap<String, PathBuf> = HashMap::default();
    let mut artifact_manifests: HashMap<PathBuf, super::sidecar::VerifiedPluginDiscoveryManifest> =
        HashMap::default();

    for (plugin_id, (item, load_origin)) in &artifact_winners {
        winner_paths.insert(plugin_id.clone(), item.path.clone());
        if let Some(manifest) = item.discovery_manifest.as_ref() {
            artifact_manifests.insert(item.path.clone(), manifest.clone());
        }
        let ScannedDynlibKind::Plugin {
            descriptor,
            descriptor_v2,
            service_gateways,
            ..
        } = &item.kind
        else {
            continue;
        };
        let origin = load_origin.gateway_origin(&item.path);
        let descriptor_candidates = if let Some(descriptor_v2) = descriptor_v2.as_ref() {
            crate::service_gateway::descriptor_v2_composition_candidates(
                descriptor_v2,
                origin,
                &planning.selection_policies,
            )
        } else if let Some(descriptor) = descriptor.as_ref() {
            crate::service_gateway::descriptor_composition_candidates(
                descriptor,
                origin,
                &planning.selection_policies,
            )
        } else {
            Vec::new()
        };
        if !descriptor_candidates.is_empty() || !service_gateways.is_empty() {
            provider_paths.insert(item.path.clone());
        }
        for candidate in descriptor_candidates {
            candidate_paths.insert(candidate.candidate_id.clone(), item.path.clone());
            candidates.push(candidate);
        }
    }

    let plan = CompositionSolver::resolve_input(CompositionSolverInput {
        candidates,
        capability_matrix: planning.capability_matrix.clone(),
    });
    let mut selected_provider_paths = NeHashSet::default();
    for gateway_id in plan.gateway_ids() {
        for selected in plan.selected_all(&gateway_id) {
            if let Some(path) = candidate_paths.get(&selected.candidate_id) {
                selected_provider_paths.insert(path.clone());
            }
        }
    }

    FrozenPluginCompositionPlan {
        plan,
        artifact_winners: winner_paths,
        artifact_manifests,
        provider_paths,
        selected_provider_paths,
        forbidden_system_tags: planning.capability_matrix.conflict_tags().to_vec(),
    }
}

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
    crate::host_context::environment_var("NEWENGINE_PLUGIN_EXCLUDE_IDS")
        .map(|value| {
            value
                .split(|ch: char| ch == ',' || ch == ';' || ch.is_ascii_whitespace())
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .any(|entry| entry == id)
        })
        .unwrap_or(false)
}

/// Converts discovery inventory into a load set.
///
/// Without an authoritative frozen plan this function is deliberately
/// conservative: discovery may deduplicate physical artifacts, but it MUST NOT
/// reject a semantically valid provider as shadowed. Provider selection is only
/// applied from `FrozenPluginCompositionPlan`, which was solved from full inputs.
pub(super) fn build_load_selection(
    graph: &DiscoveryGraph,
    filter: LoadPhaseFilter,
    loaded_ids: &NeHashSet<String>,
    frozen_plan: Option<&FrozenPluginCompositionPlan>,
) -> LoadSelection {
    let mut out = LoadSelection::default();
    let mut winners_by_id: HashMap<&str, &super::graph::ScannedDynlib> = HashMap::default();

    for item in &graph.items {
        let ScannedDynlibKind::Plugin { id, phase, .. } = &item.kind else {
            continue;
        };
        if loaded_ids.contains(id)
            || plugin_excluded_by_host_policy(id)
            || !crate::plugin_config_service::plugin_enabled_by_config(id)
            || !filter.allows(*phase)
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

    for item in &graph.items {
        let decision = match &item.kind {
            ScannedDynlibKind::PlatformRuntime { system_tags, .. } => {
                if frozen_plan.is_some_and(|plan| !plan.system_tags_allowed(system_tags)) {
                    SelectionDecision::Filtered {
                        filter_label: "composition-tags",
                    }
                } else {
                    SelectionDecision::Runtime { label: "platform" }
                }
            }
            ScannedDynlibKind::Unknown => SelectionDecision::Unknown,
            ScannedDynlibKind::Plugin {
                id,
                phase,
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
                } else if !filter.allows(*phase) {
                    SelectionDecision::Filtered {
                        filter_label: filter.label(),
                    }
                } else {
                    let frozen_winner = frozen_plan.and_then(|plan| plan.artifact_winner(id));
                    let local_winner = winners_by_id.get(id.as_str()).map(|winner| &winner.path);
                    let winner_path = frozen_winner.or(local_winner);
                    match winner_path {
                        Some(winner_path) if winner_path != &item.path => {
                            SelectionDecision::DuplicateId {
                                winner_file: winner_path
                                    .file_name()
                                    .and_then(|name| name.to_str())
                                    .unwrap_or("<unknown>")
                                    .to_owned(),
                            }
                        }
                        Some(_)
                            if frozen_plan.is_some_and(|plan| {
                                !service_gateways.is_empty()
                                    && !plan.provider_paths.contains(&item.path)
                            }) =>
                        {
                            SelectionDecision::Filtered {
                                filter_label: "not-in-frozen-composition",
                            }
                        }
                        Some(_)
                            if frozen_plan
                                .is_some_and(|plan| plan.provider_is_shadowed(&item.path)) =>
                        {
                            SelectionDecision::Filtered {
                                filter_label: "composition-plan",
                            }
                        }
                        Some(_) => {
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
                        None => SelectionDecision::Unknown,
                    }
                }
            }
        };
        out.decisions.insert(item.path.clone(), decision);
    }

    out
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
            discovery_manifest: None,
            kind: ScannedDynlibKind::Plugin {
                id: id.to_owned(),
                version: "1.0.0".to_owned(),
                phase: newengine_plugin_api::PluginBootstrapPhase::Engine,
                descriptor_kind: Some(newengine_plugin_api::PluginKind::Runtime),
                declared_capabilities: gateway.map(|_| 1),
                descriptor: None,
                descriptor_v2: None,
                service_gateways: gateway.into_iter().map(str::to_owned).collect(),
                backend_priority: priority,
            },
        }
    }

    #[test]
    fn editor_tooling_plugin_remains_selectable_for_game_runtime_target() {
        let host = crate::host_context::create_host_context();
        host.replace_environment_snapshot([(
            std::ffi::OsString::from("NEWENGINE_PLUGIN_TARGET"),
            std::ffi::OsString::from("game"),
        )]);
        crate::host_context::activate_host_context(&host);

        let path = PathBuf::from("editing-tools-0.3.0-release.dll");
        let graph = DiscoveryGraph {
            dir: PathBuf::from("pluginsRuntime"),
            entries_total: 1,
            skipped_non_dynlib: 0,
            items: vec![super::super::graph::ScannedDynlib {
                path: path.clone(),
                file_name: "editing-tools-0.3.0-release.dll".to_owned(),
                discovery_manifest: None,
                kind: ScannedDynlibKind::Plugin {
                    id: "newengine.editing.tools".to_owned(),
                    version: "0.3.0".to_owned(),
                    phase: newengine_plugin_api::PluginBootstrapPhase::Engine,
                    descriptor_kind: Some(newengine_plugin_api::PluginKind::Editor),
                    declared_capabilities: Some(1),
                    descriptor: None,
                    descriptor_v2: None,
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
            None,
        );

        assert!(selection.engine_candidates.contains(&path));
        assert!(selection
            .decisions
            .get(&path)
            .is_some_and(SelectionDecision::is_selected));
    }

    #[test]
    fn preload_without_frozen_authority_keeps_semantic_alternatives_loadable() {
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
            None,
        );

        assert!(selection.engine_candidates.contains(&high_path));
        assert!(selection.engine_candidates.contains(&low_path));
        assert!(selection
            .decisions
            .get(&low_path)
            .is_some_and(SelectionDecision::is_selected));
    }

    #[test]
    fn only_frozen_authority_may_filter_a_shadowed_provider() {
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
        let low_path = low.path.clone();
        let high_path = high.path.clone();
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

        let mut artifact_winners = HashMap::default();
        artifact_winners.insert("engine.render.low".to_owned(), low_path.clone());
        artifact_winners.insert("engine.render.high".to_owned(), high_path.clone());
        let provider_paths = [low_path.clone(), high_path.clone()].into_iter().collect();
        let selected_provider_paths = [high_path.clone()].into_iter().collect();
        let frozen = FrozenPluginCompositionPlan {
            plan: CompositionPlan::default(),
            artifact_winners,
            artifact_manifests: HashMap::default(),
            provider_paths,
            selected_provider_paths,
            forbidden_system_tags: Vec::new(),
        };

        let selection = build_load_selection(
            &graph,
            LoadPhaseFilter::BootstrapAndEngine,
            &NeHashSet::default(),
            Some(&frozen),
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
    fn frozen_inventory_accepts_legacy_v1_provider_metadata() {
        use newengine_plugin_api::{
            CapabilityDesc, CapabilityKind, CapabilityRole, PluginDescriptor, PluginKind,
        };

        let path = PathBuf::from("plugins/legacy-render.dll");
        let descriptor = PluginDescriptor::builder(
            "engine.render.legacy",
            "Legacy Render",
            "1.0.0",
            PluginKind::Runtime,
        )
        .provides_service(
            "engine.render.legacy.service",
            1,
            r#"{"methods":["info_json"]}"#,
        )
        .push(
            CapabilityDesc::new(
                "render.backend",
                CapabilityRole::Provides,
                CapabilityKind::Other,
                1,
            )
            .with_json(
                r#"{"service_kind":"render","engine_gateway":"engine.render","provider_route":"engine.render.provider","contract":"engine.render.legacy.service","backend_priority":25}"#,
            ),
        )
        .build();
        let graph = DiscoveryGraph {
            dir: PathBuf::from("pluginsRuntime"),
            entries_total: 1,
            skipped_non_dynlib: 0,
            items: vec![super::super::graph::ScannedDynlib {
                path: path.clone(),
                file_name: "legacy-render.dll".to_owned(),
                discovery_manifest: None,
                kind: ScannedDynlibKind::Plugin {
                    id: "engine.render.legacy".to_owned(),
                    version: "1.0.0".to_owned(),
                    phase: newengine_plugin_api::PluginBootstrapPhase::Engine,
                    descriptor_kind: Some(PluginKind::Runtime),
                    declared_capabilities: Some(1),
                    descriptor: Some(descriptor),
                    descriptor_v2: None,
                    service_gateways: vec!["engine.render".to_owned()],
                    backend_priority: 25,
                },
            }],
            scan_errors: Vec::new(),
            platform_runtime_count: 0,
            bootstrap_total: 0,
            engine_total: 1,
            unknown_dynlibs: Vec::new(),
        };
        let planning = crate::host_context::CompositionPlanningSnapshot {
            services: Vec::new(),
            gateway_provider_routes: Vec::new(),
            selection_policies: Vec::new(),
            capability_matrix: newengine_service_api::CapabilityMatrix::default(),
        };

        let frozen = build_frozen_composition_plan(
            &[(graph, PluginLoadOrigin::FirstPartyPlugin)],
            &planning,
        );

        assert!(frozen.provider_paths.contains(&path));
        assert!(frozen.selected_provider_paths.contains(&path));
        assert!(frozen.plan.selected("engine.render").is_some());
    }

    #[test]
    fn provider_discovered_after_freeze_is_rejected_from_load_plan() {
        let frozen_path = PathBuf::from("plugins/frozen-render.dll");
        let late_path = PathBuf::from("plugins/late-render.dll");
        let late = plugin_item(
            "plugins/late-render.dll",
            "engine.render.late",
            Some("engine.render"),
            100,
        );
        let graph = DiscoveryGraph {
            dir: PathBuf::from("pluginsRuntime"),
            entries_total: 1,
            skipped_non_dynlib: 0,
            items: vec![late],
            scan_errors: Vec::new(),
            platform_runtime_count: 0,
            bootstrap_total: 0,
            engine_total: 1,
            unknown_dynlibs: Vec::new(),
        };
        let mut artifact_winners = HashMap::default();
        artifact_winners.insert("engine.render.frozen".to_owned(), frozen_path.clone());
        let frozen = FrozenPluginCompositionPlan {
            plan: CompositionPlan::default(),
            artifact_winners,
            artifact_manifests: HashMap::default(),
            provider_paths: [frozen_path.clone()].into_iter().collect(),
            selected_provider_paths: [frozen_path].into_iter().collect(),
            forbidden_system_tags: Vec::new(),
        };

        let selection = build_load_selection(
            &graph,
            LoadPhaseFilter::BootstrapAndEngine,
            &NeHashSet::default(),
            Some(&frozen),
        );

        assert!(!selection.engine_candidates.contains(&late_path));
        assert!(matches!(
            selection.decisions.get(&late_path),
            Some(SelectionDecision::Filtered {
                filter_label: "not-in-frozen-composition"
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
            discovery_manifest: None,
            kind: ScannedDynlibKind::Plugin {
                id: "engine.multi.provider".to_owned(),
                version: "1.0.0".to_owned(),
                phase: newengine_plugin_api::PluginBootstrapPhase::Engine,
                descriptor_kind: Some(newengine_plugin_api::PluginKind::Runtime),
                declared_capabilities: Some(2),
                descriptor: None,
                descriptor_v2: None,
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
            None,
        );

        assert!(selection.engine_candidates.contains(&multi_path));
        assert!(selection.engine_candidates.contains(&render_path));
    }

    #[test]
    fn frozen_composition_filters_platform_runtime_by_tags_not_provider_name() {
        let path = PathBuf::from("plugins/vendor-neutral-platform.dll");
        let graph = DiscoveryGraph {
            dir: PathBuf::from("pluginsRuntime"),
            entries_total: 1,
            skipped_non_dynlib: 0,
            items: vec![super::super::graph::ScannedDynlib {
                path: path.clone(),
                file_name: "vendor-neutral-platform.dll".to_owned(),
                discovery_manifest: None,
                kind: ScannedDynlibKind::PlatformRuntime {
                    id: "vendor.platform.runtime".to_owned(),
                    version: "1.0.0".to_owned(),
                    system_tags: vec!["windowing".to_owned(), "headful".to_owned()],
                    backend_priority: 100,
                },
            }],
            scan_errors: Vec::new(),
            platform_runtime_count: 1,
            bootstrap_total: 0,
            engine_total: 0,
            unknown_dynlibs: Vec::new(),
        };
        let frozen = FrozenPluginCompositionPlan {
            plan: CompositionPlan::default(),
            artifact_winners: HashMap::default(),
            artifact_manifests: HashMap::default(),
            provider_paths: NeHashSet::default(),
            selected_provider_paths: NeHashSet::default(),
            forbidden_system_tags: vec!["headful".to_owned()],
        };

        let selection = build_load_selection(
            &graph,
            LoadPhaseFilter::BootstrapAndEngine,
            &NeHashSet::default(),
            Some(&frozen),
        );

        assert!(matches!(
            selection.decisions.get(&path),
            Some(SelectionDecision::Filtered {
                filter_label: "composition-tags"
            })
        ));
    }

    #[test]
    fn duplicate_artifact_rank_does_not_use_provider_priority() {
        let low_priority_newer = super::super::graph::ScannedDynlib {
            path: PathBuf::from("provider-2.0.0.dll"),
            file_name: "provider-2.0.0.dll".to_owned(),
            discovery_manifest: None,
            kind: ScannedDynlibKind::Plugin {
                id: "engine.render.provider".to_owned(),
                version: "2.0.0".to_owned(),
                phase: newengine_plugin_api::PluginBootstrapPhase::Engine,
                descriptor_kind: Some(newengine_plugin_api::PluginKind::Runtime),
                declared_capabilities: Some(1),
                descriptor: None,
                descriptor_v2: None,
                service_gateways: vec!["engine.render".to_owned()],
                backend_priority: -100,
            },
        };
        let high_priority_older = super::super::graph::ScannedDynlib {
            path: PathBuf::from("provider-1.0.0.dll"),
            file_name: "provider-1.0.0.dll".to_owned(),
            discovery_manifest: None,
            kind: ScannedDynlibKind::Plugin {
                id: "engine.render.provider".to_owned(),
                version: "1.0.0".to_owned(),
                phase: newengine_plugin_api::PluginBootstrapPhase::Engine,
                descriptor_kind: Some(newengine_plugin_api::PluginKind::Runtime),
                declared_capabilities: Some(1),
                descriptor: None,
                descriptor_v2: None,
                service_gateways: vec!["engine.render".to_owned()],
                backend_priority: 10_000,
            },
        };

        assert!(is_better_plugin_candidate(
            &low_priority_newer,
            &high_priority_older
        ));
    }
}
