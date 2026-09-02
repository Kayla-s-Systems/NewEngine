    use super::*;
    use abi_stable::sabi_trait::TD_Opaque;
    use abi_stable::std_types::RResult;
    use abi_stable::std_types::{ROption, RString, RVec};
    use newengine_plugin_api::{
        BackendRouteDescriptor, BackendRouteDescriptorV2, Blob, CapabilityDesc, CapabilityDescV2,
        CapabilityId, CapabilityKind, CapabilityRequirementDescV2, CapabilityRole, MethodName,
        PluginDescriptor, PluginDescriptorV2, PluginKind, ServiceV1, ServiceV1Dyn,
    };

    fn descriptor_v2(id: &str, capabilities: Vec<CapabilityDescV2>) -> PluginDescriptorV2 {
        PluginDescriptorV2 {
            id: RString::from(id),
            name: RString::from(id),
            version: RString::from("1.0.0"),
            kind: PluginKind::Runtime,
            capabilities: RVec::from(capabilities),
            extension_json: RString::new(),
        }
    }

    #[test]
    fn host_events_requirement_uses_events_capability_kind() {
        let consumer = descriptor_v2(
            "consumer.host-events",
            vec![CapabilityDescV2::new(
                "host.events.v1",
                CapabilityRole::Requires,
                CapabilityKind::EventsV1,
                1,
            )],
        );
        let candidates = capability_provider_candidates();
        assert!(
            missing_typed_descriptor_requirements(&consumer, &candidates).is_empty(),
            "host.events.v1 must satisfy EventsV1 requirements"
        );
        assert!(candidates.iter().any(|candidate| {
            candidate.gateway_id
                == capability_composition_key("host.events.v1", CapabilityKind::EventsV1)
                && candidate
                    .capability_ids
                    .iter()
                    .any(|capability| capability == "host.events.v1")
        }));
    }

    #[test]
    fn typed_requirement_range_and_tags_are_solver_constraints() {
        let provider = CapabilityDescV2::new(
            "engine.test.capability",
            CapabilityRole::Provides,
            CapabilityKind::Other,
            3,
        )
        .with_tag("feature.required")
        .with_tag("feature.fast");
        let provider_descriptor = descriptor_v2("provider", vec![provider]);
        let candidates = provider_descriptor
            .capabilities
            .iter()
            .filter_map(|capability| candidate_from_typed_capability("provider", capability))
            .collect::<Vec<_>>();

        let requirement = CapabilityDescV2::new(
            "engine.test.capability",
            CapabilityRole::Requires,
            CapabilityKind::Other,
            2,
        )
        .with_requirement(
            CapabilityRequirementDescV2::between(2, 3)
                .with_required_tag("feature.required")
                .with_preferred_tag("feature.fast")
                .with_forbidden_tag("backend.software"),
        );
        let consumer = descriptor_v2("consumer", vec![requirement]);
        assert!(missing_typed_descriptor_requirements(&consumer, &candidates).is_empty());

        let too_new = CapabilityDescV2::new(
            "engine.test.capability",
            CapabilityRole::Provides,
            CapabilityKind::Other,
            4,
        )
        .with_tag("feature.required")
        .with_tag("feature.fast");
        let too_new_candidates = [too_new]
            .iter()
            .filter_map(|capability| candidate_from_typed_capability("provider", capability))
            .collect::<Vec<_>>();
        let missing = missing_typed_descriptor_requirements(&consumer, &too_new_candidates);
        assert_eq!(missing.len(), 1);
        assert!(missing[0].contains("version=2..3"));

        let forbidden = CapabilityDescV2::new(
            "engine.test.capability",
            CapabilityRole::Provides,
            CapabilityKind::Other,
            3,
        )
        .with_tag("feature.required")
        .with_tag("backend.software");
        let forbidden_candidates = [forbidden]
            .iter()
            .filter_map(|capability| candidate_from_typed_capability("provider", capability))
            .collect::<Vec<_>>();
        let missing = missing_typed_descriptor_requirements(&consumer, &forbidden_candidates);
        assert_eq!(missing.len(), 1);
        assert!(missing[0].contains("forbidden_tags=backend.software"));
    }

    #[test]
    fn backend_route_alias_satisfies_generic_gateway_service_requirement() {
        let provider = CapabilityDescV2::backend_route(
            "test.semantic.backend",
            1,
            BackendRouteDescriptor::new(newengine_service_api::BackendServiceSpec::new(
                "test.semantic",
                "engine.test.semantic",
                "test.semantic.api",
                "test.semantic.backend",
            ))
            .provider_route("engine.test.semantic.runtime")
            .backend("test_semantic_runtime")
            .contract("test.semantic.api"),
        );
        let provider_descriptor = descriptor_v2("engine.test.semantic.runtime", vec![provider]);
        let mut candidates = Vec::new();
        for capability in provider_descriptor.capabilities.iter() {
            if let Some(candidate) =
                candidate_from_typed_capability(provider_descriptor.id.as_str(), capability)
            {
                candidates.push(candidate);
            }
            if let Some(candidate) = route_alias_candidate_from_typed_capability(
                provider_descriptor.id.as_str(),
                capability,
            ) {
                candidates.push(candidate);
            }
        }

        let consumer = descriptor_v2(
            "newengine.composition.game-ready",
            vec![CapabilityDescV2::new(
                "engine.test.semantic",
                CapabilityRole::Requires,
                CapabilityKind::ServiceV1,
                1,
            )],
        );

        assert!(
            missing_typed_descriptor_requirements(&consumer, &candidates).is_empty(),
            "provider backend route must alias its declared engine gateway"
        );
        assert!(candidates.iter().any(|candidate| {
            candidate
                .capability_ids
                .iter()
                .any(|id| id == "engine.test.semantic")
        }));
    }

    #[test]
    fn native_v2_route_is_authoritative_for_provider_origin() {
        let legacy = PluginDescriptor::builder(
            "engine.render.provider-test",
            "Provider Test",
            "1.0.0",
            PluginKind::Runtime,
        )
        .push(
            CapabilityDesc::new(
                "render.backend.test",
                CapabilityRole::Provides,
                CapabilityKind::Other,
                1,
            )
            .with_json(
                r#"{"service_kind":"render","engine_gateway":"engine.render","contract":"render.api","backend":"null","mode":"headless"}"#,
            ),
        )
        .build();

        let mut typed = PluginDescriptorV2::from_legacy(&legacy);
        let capability = typed.capabilities.get_mut(0).expect("typed capability");
        capability.route = ROption::RSome(BackendRouteDescriptorV2 {
            service_kind: RString::from("render"),
            engine_gateway: RString::from("engine.render"),
            provider_service_id: RString::from("render.api"),
            provider_abi: ROption::RNone,
            provider_route: ROption::RNone,
            backend_priority: 1,
            backend: ROption::RSome(RString::from("vulkan")),
            mode: ROption::RSome(RString::from("windowed")),
            features: RVec::new(),
        });

        let default_origin = crate::service_gateway::GatewayProviderOrigin::GamePlugin;
        assert_eq!(
            effective_provider_origin(&legacy, Some(&typed), default_origin),
            default_origin
        );

        let capability = typed.capabilities.get_mut(0).expect("typed capability");
        let ROption::RSome(route) = &mut capability.route else {
            panic!("typed route missing");
        };
        route.mode = ROption::RSome(RString::from("headless"));
        assert_eq!(
            effective_provider_origin(&legacy, Some(&typed), default_origin),
            crate::service_gateway::GatewayProviderOrigin::NullProvider
        );
    }

    struct HostGatewayTestService;

    impl ServiceV1 for HostGatewayTestService {
        fn id(&self) -> CapabilityId {
            CapabilityId::from("test.host-gateway.service")
        }

        fn describe(&self) -> RString {
            RString::from("{\"test\":true}")
        }

        fn call(&self, _method: MethodName, payload: Blob) -> RResult<Blob, RString> {
            RResult::ROk(payload)
        }
    }

    #[test]
    fn active_host_gateway_route_satisfies_service_requirement() {
        let context = crate::host_context::create_host_context();
        crate::host_context::with_host_context(&context, || {
            let service = ServiceV1Dyn::from_value(HostGatewayTestService, TD_Opaque);
            crate::host_api::host_register_service_impl(service)
                .into_result()
                .expect("register host gateway test service");
            crate::host_context::register_engine_gateway_provider_route(
                "engine.test-host-gateway",
                "test-host-gateway",
                "test.host-gateway.service",
                "engine.test-host-gateway.core",
                "test.host-gateway.backend",
                0,
                "test.host-gateway",
            )
            .expect("register host gateway test route");

            let consumer = descriptor_v2(
                "consumer.host-gateway",
                vec![CapabilityDescV2::new(
                    "engine.test-host-gateway",
                    CapabilityRole::Requires,
                    CapabilityKind::ServiceV1,
                    1,
                )],
            );
            let candidates = capability_provider_candidates();
            assert!(
                missing_typed_descriptor_requirements(&consumer, &candidates).is_empty(),
                "active host-owned engine gateway must satisfy ServiceV1 requirement by gateway id"
            );
            assert!(candidates.iter().any(|candidate| {
                candidate.gateway_id
                    == capability_composition_key(
                        "engine.test-host-gateway",
                        CapabilityKind::ServiceV1,
                    )
                    && candidate
                        .capability_ids
                        .iter()
                        .any(|capability| capability == "engine.test-host-gateway")
            }));
        });
    }
