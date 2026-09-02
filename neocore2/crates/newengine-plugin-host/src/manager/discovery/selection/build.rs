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
    let mut host_policy = HostPluginPolicySnapshot::capture();

    for item in &graph.items {
        let ScannedDynlibKind::Plugin { id, phase, .. } = &item.kind else {
            continue;
        };
        if loaded_ids.contains(id)
            || !filter.allows(*phase)
            || host_policy.is_excluded(id)
            || !host_policy.is_enabled(id)
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
                } else if host_policy.is_excluded(id) {
                    SelectionDecision::Unsupported {
                        reason: "plugin id excluded by host composition policy",
                    }
                } else if !host_policy.is_enabled(id) {
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
