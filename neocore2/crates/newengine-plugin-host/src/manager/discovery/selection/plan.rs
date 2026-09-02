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
    let mut host_policy = HostPluginPolicySnapshot::capture();

    for (graph, load_origin) in inventories {
        for item in &graph.items {
            let ScannedDynlibKind::Plugin { id, .. } = &item.kind else {
                continue;
            };
            if host_policy.is_excluded(id) || !host_policy.is_enabled(id) {
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

