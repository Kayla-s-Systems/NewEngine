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
