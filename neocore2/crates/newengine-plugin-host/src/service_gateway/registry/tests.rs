use newengine_plugin_api::{CapabilityDesc, CapabilityKind, CapabilityRole, PluginDescriptor, PluginKind};
use newengine_service_api::EngineServiceKind;

use super::*;

fn descriptor(
    plugin_id: &str,
    provider_service_id: &str,
    gateway_id: &str,
    backend_capability_id: &str,
    service_kind: &str,
    backend_priority: i32,
) -> PluginDescriptor {
    PluginDescriptor::builder(plugin_id, plugin_id, "1.0.0", PluginKind::Runtime)
        .provides_service(provider_service_id, 1, r#"{"methods":["info_json"]}"#)
        .push(
            CapabilityDesc::new(
                backend_capability_id,
                CapabilityRole::Provides,
                CapabilityKind::Other,
                1,
            )
            .with_json(format!(
                r#"{{"service_kind":"{}","engine_gateway":"{}","contract":"{}","backend_priority":{}}}"#,
                service_kind, gateway_id, provider_service_id, backend_priority
            )),
        )
        .build()
}

fn service(service_id: &str, owner: Option<&str>) -> RegisteredServiceFact {
    RegisteredServiceFact::new(
        service_id.to_owned(),
        owner.map(std::borrow::ToOwned::to_owned),
    )
}

#[test]
fn plugin_origin_tier_overrides_engine_owned_even_with_lower_backend_priority() {
    let descriptors = vec![PluginDescriptorFact::new(
        "mod.camera".to_owned(),
        descriptor(
            "mod.camera",
            "mod.camera.api",
            "engine.camera",
            "camera.backend",
            "camera",
            0,
        ),
        GatewayProviderOrigin::UserMod,
    )];
    let services = vec![
        service("mod.camera.api", Some("mod.camera")),
        service("engine.camera", None),
    ];
    let engine_owned = vec![EngineOwnedGatewayFact::new(
        "engine.camera".to_owned(),
        EngineServiceKind::Camera,
        "engine.camera".to_owned(),
        "newengine-engine-runtime.camera-gateway".to_owned(),
        "camera.backend".to_owned(),
        5_000,
    )];

    let registry = ActiveGatewayRegistry::from_facts(&descriptors, &services, &engine_owned);
    let route = registry.resolve_route("engine.camera").expect("engine.camera route");

    assert_eq!(route.provider_service_id, "mod.camera.api");
    assert_eq!(route.origin, GatewayProviderOrigin::UserMod);
    assert_eq!(route.active_score, 40_000);
}

#[test]
fn one_plugin_can_override_multiple_authority_gateways() {
    let descriptor = PluginDescriptor::builder("newengine.ecs.flecs", "FlecsECS", "1.0.0", PluginKind::Runtime)
        .provides_service("ecs.api", 1, r#"{"methods":["summary_json_v1","snapshot_json_v1","command_json_v1"]}"#)
        .provides_service("entity.api", 1, r#"{"methods":["list_json_v1","spawn_json_v1","despawn_json_v1"]}"#)
        .push(
            CapabilityDesc::new(
                "ecs.backend",
                CapabilityRole::Provides,
                CapabilityKind::Other,
                1,
            )
            .with_json(r#"{"service_kind":"ecs","engine_gateway":"engine.ecs","contract":"ecs.api","backend_priority":500}"#),
        )
        .push(
            CapabilityDesc::new(
                "entity.backend",
                CapabilityRole::Provides,
                CapabilityKind::Other,
                1,
            )
            .with_json(r#"{"service_kind":"entity","engine_gateway":"engine.entity","contract":"entity.api","backend_priority":500}"#),
        )
        .build();
    let descriptors = vec![PluginDescriptorFact::new(
        "newengine.ecs.flecs".to_owned(),
        descriptor,
        GatewayProviderOrigin::FirstPartyPlugin,
    )];
    let services = vec![
        service("ecs.api", Some("newengine.ecs.flecs")),
        service("entity.api", Some("newengine.ecs.flecs")),
        service("engine.ecs", None),
        service("engine.entity", None),
    ];
    let engine_owned = vec![
        EngineOwnedGatewayFact::new(
            "engine.ecs".to_owned(),
            EngineServiceKind::Ecs,
            "engine.ecs".to_owned(),
            "newengine-ecs-runtime.ecs-gateway".to_owned(),
            "ecs.backend".to_owned(),
            0,
        ),
        EngineOwnedGatewayFact::new(
            "engine.entity".to_owned(),
            EngineServiceKind::Entity,
            "engine.entity".to_owned(),
            "newengine-entity-runtime.entity-gateway".to_owned(),
            "entity.backend".to_owned(),
            0,
        ),
    ];

    let registry = ActiveGatewayRegistry::from_facts(&descriptors, &services, &engine_owned);
    let ecs_route = registry.resolve_route("engine.ecs").expect("engine.ecs route");
    let entity_route = registry.resolve_route("engine.entity").expect("engine.entity route");

    assert_eq!(ecs_route.provider_service_id, "ecs.api");
    assert_eq!(entity_route.provider_service_id, "entity.api");
    assert_eq!(ecs_route.provider_owner_id, "newengine.ecs.flecs");
    assert_eq!(entity_route.provider_owner_id, "newengine.ecs.flecs");
    assert_eq!(ecs_route.origin, GatewayProviderOrigin::FirstPartyPlugin);
    assert_eq!(entity_route.origin, GatewayProviderOrigin::FirstPartyPlugin);
    assert_eq!(ecs_route.active_score, 20_500);
    assert_eq!(entity_route.active_score, 20_500);
}

#[test]
fn dynamic_engine_owned_gateway_does_not_require_central_service_kind_enum() {
    let services = vec![service("render.draw_lists.api", None)];
    let engine_owned = vec![EngineOwnedGatewayFact::new_dynamic(
        "engine.render.draw_lists".to_owned(),
        "render.draw_lists".to_owned(),
        "render.draw_lists.api".to_owned(),
        "newengine-render-runtime.draw-lists".to_owned(),
        "render.draw_list_provider".to_owned(),
        0,
        [system_tag::ENGINE_DOMAIN, system_tag::PROVIDER_BACKEND],
    )];

    let registry = ActiveGatewayRegistry::from_facts(&[], &services, &engine_owned);
    let route = registry.resolve_route("engine.render.draw_lists").expect("dynamic draw-list route");

    assert_eq!(route.service_kind, "render.draw_lists");
    assert_eq!(route.provider_service_id, "render.draw_lists.api");
}

#[test]
fn engine_owned_is_used_when_no_plugin_provider_exists() {
    let services = vec![service("engine.camera", None)];
    let engine_owned = vec![EngineOwnedGatewayFact::new(
        "engine.camera".to_owned(),
        EngineServiceKind::Camera,
        "engine.camera".to_owned(),
        "newengine-engine-runtime.camera-gateway".to_owned(),
        "camera.backend".to_owned(),
        0,
    )];

    let registry = ActiveGatewayRegistry::from_facts(&[], &services, &engine_owned);
    let route = registry.resolve_route("engine.camera").expect("engine.camera route");

    assert_eq!(route.provider_service_id, "engine.camera");
    assert_eq!(route.origin, GatewayProviderOrigin::EngineOwned);
    assert_eq!(route.active_score, 10_000);
}

#[test]
fn higher_priority_wins_inside_same_origin_tier() {
    let descriptors = vec![
        PluginDescriptorFact::new(
            "mod.camera.low".to_owned(),
            descriptor(
                "mod.camera.low",
                "mod.camera.low.api",
                "engine.camera",
                "camera.backend",
                "camera",
                10,
            ),
            GatewayProviderOrigin::UserMod,
        ),
        PluginDescriptorFact::new(
            "mod.camera.high".to_owned(),
            descriptor(
                "mod.camera.high",
                "mod.camera.high.api",
                "engine.camera",
                "camera.backend",
                "camera",
                20,
            ),
            GatewayProviderOrigin::UserMod,
        ),
    ];
    let services = vec![
        service("mod.camera.low.api", Some("mod.camera.low")),
        service("mod.camera.high.api", Some("mod.camera.high")),
    ];

    let registry = ActiveGatewayRegistry::from_facts(&descriptors, &services, &[]);
    let route = registry.resolve_route("engine.camera").expect("engine.camera route");

    assert_eq!(route.provider_service_id, "mod.camera.high.api");
    assert_eq!(route.active_score, 40_020);
}

#[test]
fn locked_gateway_rejects_plugin_route() {
    let descriptors = vec![PluginDescriptorFact::new(
        "mod.security".to_owned(),
        descriptor(
            "mod.security",
            "mod.security.api",
            "engine.security",
            "security.backend",
            "security",
            99_999,
        ),
        GatewayProviderOrigin::DevOverride,
    )];
    let services = vec![
        service("mod.security.api", Some("mod.security")),
        service("engine.security", None),
    ];
    let engine_owned = vec![EngineOwnedGatewayFact::new(
        "engine.security".to_owned(),
        EngineServiceKind::Security,
        "engine.security".to_owned(),
        "newengine.security".to_owned(),
        "security.backend".to_owned(),
        0,
    )];

    let policies = vec![GatewayPolicyFact::new(
        "engine.security".to_owned(),
        GatewayOverrideMode::Locked,
        [system_tag::TRUST_ROOT, system_tag::OVERRIDE_LOCKED],
        "newengine.security.policy".to_owned(),
    )];
    let registry = ActiveGatewayRegistry::from_facts_with_policy(
        &descriptors,
        &services,
        &engine_owned,
        &policies,
    );
    let route = registry.resolve_route("engine.security").expect("engine.security route");

    assert_eq!(route.provider_service_id, "engine.security");
    assert_eq!(route.origin, GatewayProviderOrigin::EngineOwned);
}

#[test]
fn tie_breakers_are_deterministic() {
    let descriptors = vec![
        PluginDescriptorFact::new(
            "mod.b".to_owned(),
            descriptor("mod.b", "b.camera.api", "engine.camera", "camera.backend", "camera", 1),
            GatewayProviderOrigin::UserMod,
        ),
        PluginDescriptorFact::new(
            "mod.a".to_owned(),
            descriptor("mod.a", "a.camera.api", "engine.camera", "camera.backend", "camera", 1),
            GatewayProviderOrigin::UserMod,
        ),
    ];
    let services = vec![
        service("b.camera.api", Some("mod.b")),
        service("a.camera.api", Some("mod.a")),
    ];

    let registry = ActiveGatewayRegistry::from_facts(&descriptors, &services, &[]);
    let route = registry.resolve_route("engine.camera").expect("engine.camera route");

    assert_eq!(route.provider_service_id, "a.camera.api");
}
#[test]
fn child_domain_route_is_selected_when_kind_and_gateway_match() {
    let descriptors = vec![PluginDescriptorFact::new(
        "mod.input.bindings".to_owned(),
        descriptor(
            "mod.input.bindings",
            "mod.input.bindings.api",
            "engine.input.bindings",
            "input.bindings.backend",
            "input.bindings",
            7,
        ),
        GatewayProviderOrigin::UserMod,
    )];
    let services = vec![service("mod.input.bindings.api", Some("mod.input.bindings"))];

    let registry = ActiveGatewayRegistry::from_facts(&descriptors, &services, &[]);
    let route = registry.resolve_route("engine.input.bindings").expect("engine.input.bindings route");

    assert_eq!(route.service_kind, EngineServiceKind::InputBindings.as_str());
    assert_eq!(route.provider_service_id, "mod.input.bindings.api");
}



#[test]
fn dynamic_gateway_kind_does_not_require_engine_enum_entry() {
    let descriptors = vec![PluginDescriptorFact::new(
        "mod.weather".to_owned(),
        descriptor(
            "mod.weather",
            "mod.weather.api",
            "engine.weather",
            "weather.backend",
            "weather",
            42,
        ),
        GatewayProviderOrigin::UserMod,
    )];
    let services = vec![service("mod.weather.api", Some("mod.weather"))];

    let registry = ActiveGatewayRegistry::from_facts(&descriptors, &services, &[]);
    let route = registry.resolve_route("engine.weather").expect("engine.weather route");

    assert_eq!(route.service_kind, "weather");
    assert_eq!(route.provider_service_id, "mod.weather.api");
    assert_eq!(route.override_mode, GatewayOverrideMode::Open);
}

#[test]
fn system_tags_can_drive_policy_without_gateway_match_lists() {
    let descriptors = vec![PluginDescriptorFact::new(
        "mod.render".to_owned(),
        descriptor(
            "mod.render",
            "mod.render.api",
            "engine.render",
            "render.backend",
            "render",
            10,
        ),
        GatewayProviderOrigin::GamePlugin,
    )];
    let services = vec![service("mod.render.api", Some("mod.render"))];
    let policies = vec![GatewayPolicyFact::new(
        "engine.render".to_owned(),
        GatewayOverrideMode::ProfileControlled,
        [system_tag::OVERRIDE_PROFILE_CONTROLLED],
        "profile.gateway-policy".to_owned(),
    )];

    let registry = ActiveGatewayRegistry::from_facts_with_policy(
        &descriptors,
        &services,
        &[],
        &policies,
    );
    let route = registry.resolve_route("engine.render").expect("engine.render route");

    assert_eq!(route.override_mode, GatewayOverrideMode::ProfileControlled);
    assert!(route.system_tags.iter().any(|tag| tag == system_tag::OVERRIDE_PROFILE_CONTROLLED));
}
#[test]
fn mixed_parent_and_child_domain_route_is_ignored() {
    let descriptors = vec![PluginDescriptorFact::new(
        "bad.input".to_owned(),
        descriptor(
            "bad.input",
            "bad.input.api",
            "engine.input.bindings",
            "input.bindings.backend",
            "input",
            100,
        ),
        GatewayProviderOrigin::UserMod,
    )];
    let services = vec![service("bad.input.api", Some("bad.input"))];

    let registry = ActiveGatewayRegistry::from_facts(&descriptors, &services, &[]);

    assert!(registry.resolve_route("engine.input.bindings").is_none());
}
