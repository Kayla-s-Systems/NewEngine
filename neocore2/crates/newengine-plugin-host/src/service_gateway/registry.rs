#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;

use newengine_plugin_api::PluginDescriptor;
use newengine_service_api::{engine_gateway_domain, system_tag, EngineServiceKind};

use super::metadata::descriptor_gateway_capabilities;
use super::provider::gateway_provider_service_id;

#[derive(Debug, Clone)]
pub(crate) struct RegisteredServiceFact {
    pub(crate) service_id: String,
    pub(crate) owner_plugin_id: Option<String>,
}

impl RegisteredServiceFact {
    #[inline]
    pub(crate) fn new(service_id: String, owner_plugin_id: Option<String>) -> Self {
        Self { service_id, owner_plugin_id }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PluginDescriptorFact {
    pub(crate) plugin_id: String,
    pub(crate) descriptor: PluginDescriptor,
    pub(crate) origin: GatewayProviderOrigin,
}

impl PluginDescriptorFact {
    #[inline]
    pub(crate) fn new(
        plugin_id: String,
        descriptor: PluginDescriptor,
        origin: GatewayProviderOrigin,
    ) -> Self {
        Self { plugin_id, descriptor, origin }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EngineOwnedGatewayFact {
    pub(crate) gateway_id: String,
    pub(crate) service_kind: String,
    pub(crate) provider_service_id: String,
    pub(crate) provider_owner_id: String,
    pub(crate) backend_capability_id: String,
    pub(crate) backend_priority: i32,
    pub(crate) system_tags: Vec<String>,
}

impl EngineOwnedGatewayFact {
    #[inline]
    pub(crate) fn new(
        gateway_id: String,
        service_kind: EngineServiceKind,
        provider_service_id: String,
        provider_owner_id: String,
        backend_capability_id: String,
        backend_priority: i32,
    ) -> Self {
        Self::new_dynamic(
            gateway_id,
            service_kind.as_str().to_owned(),
            provider_service_id,
            provider_owner_id,
            backend_capability_id,
            backend_priority,
            [system_tag::ENGINE_DOMAIN, system_tag::PROVIDER_BACKEND],
        )
    }

    #[inline]
    pub(crate) fn new_dynamic<I, S>(
        gateway_id: String,
        service_kind: String,
        provider_service_id: String,
        provider_owner_id: String,
        backend_capability_id: String,
        backend_priority: i32,
        system_tags: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut system_tags = system_tags
            .into_iter()
            .filter_map(|tag| newengine_service_api::normalize_system_tag(tag.as_ref()))
            .collect::<Vec<_>>();
        system_tags.sort();
        system_tags.dedup();
        Self {
            gateway_id,
            service_kind,
            provider_service_id,
            provider_owner_id,
            backend_capability_id,
            backend_priority,
            system_tags,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GatewayPolicyFact {
    pub(crate) gateway_id: String,
    pub(crate) override_mode: GatewayOverrideMode,
    pub(crate) system_tags: Vec<String>,
    pub(crate) owner_id: String,
}

#[cfg(test)]
impl GatewayPolicyFact {
    #[inline]
    pub(crate) fn new<I, S>(
        gateway_id: String,
        override_mode: GatewayOverrideMode,
        system_tags: I,
        owner_id: String,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut system_tags = system_tags
            .into_iter()
            .filter_map(|tag| newengine_service_api::normalize_system_tag(tag.as_ref()))
            .collect::<Vec<_>>();
        system_tags.sort();
        system_tags.dedup();
        Self { gateway_id, override_mode, system_tags, owner_id }
    }
}

/// Host-assigned trust/origin tier used by gateway provider selection.
///
/// Plugins must not be trusted to self-declare this value in descriptor JSON.
/// The host/loader assigns it from the load source, profile metadata or dev
/// override policy before descriptor facts enter `ActiveGatewayRegistry`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum GatewayProviderOrigin {
    NullProvider,
    EngineOwned,
    FirstPartyPlugin,
    GamePlugin,
    UserMod,
    DevOverride,
}

impl GatewayProviderOrigin {
    #[inline]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::NullProvider => "null-provider",
            Self::EngineOwned => "engine-owned",
            Self::FirstPartyPlugin => "first-party-plugin",
            Self::GamePlugin => "game-plugin",
            Self::UserMod => "user-mod",
            Self::DevOverride => "dev-override",
        }
    }

    #[inline]
    pub(crate) const fn origin_bias(self) -> i64 {
        match self {
            Self::DevOverride => 50_000,
            Self::UserMod => 40_000,
            Self::GamePlugin => 30_000,
            Self::FirstPartyPlugin => 20_000,
            Self::EngineOwned => 10_000,
            Self::NullProvider => 0,
        }
    }

    /// Conservative host-side classification used when no profile policy has
    /// supplied an explicit origin. This is intentionally path-derived and
    /// best-effort only; the descriptor JSON is never trusted for origin.
    pub(crate) fn from_plugin_path(path: &Path) -> Self {
        let normalized = path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .map(|s| s.to_ascii_lowercase())
            .collect::<Vec<_>>();

        if normalized.iter().any(|part| {
            matches!(
                part.as_str(),
                "devoverrides" | "dev-overrides" | "dev_override"
            )
        }) {
            return Self::DevOverride;
        }

        if normalized.iter().any(|part| matches!(part.as_str(), "mods" | "mod" | "user_mods")) {
            return Self::UserMod;
        }

        if normalized.iter().any(|part| {
            matches!(part.as_str(), "gameplugins" | "game-plugins" | "profileplugins")
        }) {
            return Self::GamePlugin;
        }

        if normalized.iter().any(|part| matches!(part.as_str(), "plugins" | "plugin")) {
            return Self::FirstPartyPlugin;
        }

        Self::GamePlugin
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GatewayOverrideMode {
    Open,
    ProfileControlled,
    Locked,
}

impl GatewayOverrideMode {
    #[inline]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::ProfileControlled => "profile-controlled",
            Self::Locked => "locked",
        }
    }

    fn from_system_tags(tags: &[String]) -> Option<Self> {
        if tags.iter().any(|tag| tag == system_tag::OVERRIDE_LOCKED || tag == system_tag::TRUST_ROOT) {
            return Some(Self::Locked);
        }
        if tags.iter().any(|tag| tag == system_tag::OVERRIDE_PROFILE_CONTROLLED) {
            return Some(Self::ProfileControlled);
        }
        if tags.iter().any(|tag| tag == system_tag::OVERRIDE_OPEN) {
            return Some(Self::Open);
        }
        None
    }
}

#[inline]
fn route_allowed_by_policy(override_mode: GatewayOverrideMode, origin: GatewayProviderOrigin) -> bool {
    match override_mode {
        GatewayOverrideMode::Open | GatewayOverrideMode::ProfileControlled => true,
        GatewayOverrideMode::Locked => matches!(origin, GatewayProviderOrigin::EngineOwned),
    }
}

fn merge_system_tags(
    mut route_tags: Vec<String>,
    policy: Option<&GatewayPolicyFact>,
) -> Vec<String> {
    if let Some(policy) = policy {
        route_tags.extend(policy.system_tags.iter().cloned());
    }
    route_tags.sort();
    route_tags.dedup();
    route_tags
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveGatewayRoute {
    pub(crate) gateway_id: String,
    pub(crate) service_kind: String,
    pub(crate) provider_service_id: String,
    pub(crate) provider_owner_id: String,
    pub(crate) backend_capability_id: String,
    pub(crate) backend_priority: i32,
    pub(crate) origin: GatewayProviderOrigin,
    pub(crate) override_mode: GatewayOverrideMode,
    pub(crate) active_score: i64,
    pub(crate) system_tags: Vec<String>,
}

impl ActiveGatewayRoute {
    #[inline]
    fn new(
        gateway_id: String,
        service_kind: String,
        provider_service_id: String,
        provider_owner_id: String,
        backend_capability_id: String,
        backend_priority: i32,
        origin: GatewayProviderOrigin,
        route_tags: Vec<String>,
        policy: Option<&GatewayPolicyFact>,
    ) -> Option<Self> {
        if engine_gateway_domain(&gateway_id).as_deref() != Some(service_kind.as_str()) {
            log::warn!(
                "gateways: ignoring route with mixed domain levels gateway='{}' service_kind='{}' gateway_domain='{}' service='{}' owner='{}'",
                gateway_id,
                service_kind,
                engine_gateway_domain(&gateway_id).unwrap_or_else(|| "<invalid>".to_owned()),
                provider_service_id,
                provider_owner_id,
            );
            return None;
        }

        let system_tags = merge_system_tags(route_tags, policy);
        let override_mode = policy
            .map(|policy| policy.override_mode)
            .or_else(|| GatewayOverrideMode::from_system_tags(&system_tags))
            .unwrap_or(GatewayOverrideMode::Open);

        if !route_allowed_by_policy(override_mode, origin) {
            log::warn!(
                "gateways: ignoring route blocked by override policy gateway='{}' service='{}' owner='{}' origin='{}' mode='{}' policy_owner='{}' tags='{}'",
                gateway_id,
                provider_service_id,
                provider_owner_id,
                origin.as_str(),
                override_mode.as_str(),
                policy.map(|policy| policy.owner_id.as_str()).unwrap_or("<route-tags>"),
                system_tags.join(","),
            );
            return None;
        }

        let active_score = origin.origin_bias() + i64::from(backend_priority);
        Some(Self {
            gateway_id,
            service_kind,
            provider_service_id,
            provider_owner_id,
            backend_capability_id,
            backend_priority,
            origin,
            override_mode,
            active_score,
            system_tags,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ActiveGatewayRegistry {
    routes: Vec<ActiveGatewayRoute>,
}

impl ActiveGatewayRegistry {
    pub(crate) fn from_facts(
        descriptors: &[PluginDescriptorFact],
        services: &[RegisteredServiceFact],
        engine_owned_gateways: &[EngineOwnedGatewayFact],
    ) -> Self {
        Self::from_facts_with_policy(descriptors, services, engine_owned_gateways, &[])
    }

    pub(crate) fn from_facts_with_policy(
        descriptors: &[PluginDescriptorFact],
        services: &[RegisteredServiceFact],
        engine_owned_gateways: &[EngineOwnedGatewayFact],
        policy_facts: &[GatewayPolicyFact],
    ) -> Self {
        let mut routes = Vec::new();
        let mut skipped_unregistered = 0usize;

        for descriptor_fact in descriptors {
            for gateway in descriptor_gateway_capabilities(&descriptor_fact.descriptor) {
                let Some(provider_service_id) =
                    gateway_provider_service_id(&descriptor_fact.descriptor, &gateway)
                else {
                    continue;
                };

                let registered = services.iter().any(|service| {
                    service.service_id == provider_service_id
                        && service.owner_plugin_id.as_deref()
                            == Some(descriptor_fact.plugin_id.as_str())
                });
                if !registered {
                    skipped_unregistered += 1;
                    log::trace!(
                        "gateways: plugin route skipped because service is not registered plugin='{}' gateway='{}' service='{}' capability='{}'",
                        descriptor_fact.plugin_id,
                        gateway.gateway_id,
                        provider_service_id,
                        gateway.backend_capability_id
                    );
                    continue;
                }

                let policy = policy_facts.iter().find(|policy| policy.gateway_id == gateway.gateway_id);
                if let Some(route) = ActiveGatewayRoute::new(
                    gateway.gateway_id,
                    gateway.service_kind,
                    provider_service_id,
                    descriptor_fact.plugin_id.clone(),
                    gateway.backend_capability_id,
                    gateway.backend_priority,
                    descriptor_fact.origin,
                    gateway.system_tags,
                    policy,
                ) {
                    routes.push(route);
                }
            }
        }

        for gateway in engine_owned_gateways {
            let registered = services.iter().any(|service| {
                service.service_id == gateway.provider_service_id && service.owner_plugin_id.is_none()
            });
            if !registered {
                skipped_unregistered += 1;
                log::trace!(
                    "gateways: engine-owned route skipped because service is not registered gateway='{}' service='{}' owner='{}'",
                    gateway.gateway_id,
                    gateway.provider_service_id,
                    gateway.provider_owner_id
                );
                continue;
            }

            let policy = policy_facts.iter().find(|policy| policy.gateway_id == gateway.gateway_id);
            if let Some(route) = ActiveGatewayRoute::new(
                gateway.gateway_id.clone(),
                gateway.service_kind.clone(),
                gateway.provider_service_id.clone(),
                gateway.provider_owner_id.clone(),
                gateway.backend_capability_id.clone(),
                gateway.backend_priority,
                GatewayProviderOrigin::EngineOwned,
                gateway.system_tags.clone(),
                policy,
            ) {
                routes.push(route);
            }
        }

        routes.sort_by(|a, b| {
            a.gateway_id
                .cmp(&b.gateway_id)
                .then_with(|| b.active_score.cmp(&a.active_score))
                .then_with(|| b.backend_priority.cmp(&a.backend_priority))
                .then_with(|| b.origin.origin_bias().cmp(&a.origin.origin_bias()))
                .then_with(|| a.service_kind.cmp(&b.service_kind))
                .then_with(|| a.provider_service_id.cmp(&b.provider_service_id))
                .then_with(|| a.provider_owner_id.cmp(&b.provider_owner_id))
        });

        let registry = Self { routes };
        log::debug!(
            "gateways: registry rebuilt descriptors={} services={} engine_owned={} policy_facts={} routes={} skipped_unregistered={}",
            descriptors.len(),
            services.len(),
            engine_owned_gateways.len(),
            policy_facts.len(),
            registry.routes.len(),
            skipped_unregistered
        );
        for gateway_id in registry.gateway_ids() {
            if let Some(route) = registry.resolve_route(&gateway_id) {
                log::trace!(
                    "gateways: active route gateway='{}' service='{}' owner='{}' kind='{}' origin='{}' mode='{}' prio={} score={} tags='{}'",
                    route.gateway_id,
                    route.provider_service_id,
                    route.provider_owner_id,
                    route.service_kind,
                    route.origin.as_str(),
                    route.override_mode.as_str(),
                    route.backend_priority,
                    route.active_score,
                    route.system_tags.join(",")
                );
            }
        }

        registry
    }

    pub(crate) fn routes(&self) -> &[ActiveGatewayRoute] {
        &self.routes
    }

    pub(crate) fn gateway_ids(&self) -> Vec<String> {
        let mut out = self.routes.iter().map(|route| route.gateway_id.clone()).collect::<Vec<_>>();
        out.sort();
        out.dedup();
        out
    }

    pub(crate) fn resolve_gateway(&self, gateway_id: &str) -> Option<String> {
        self.resolve_route(gateway_id)
            .map(|route| route.provider_service_id.clone())
    }

    pub(crate) fn resolve_route(&self, gateway_id: &str) -> Option<&ActiveGatewayRoute> {
        self.routes
            .iter()
            .filter(|route| route.gateway_id == gateway_id)
            .max_by(|a, b| {
                a.active_score
                    .cmp(&b.active_score)
                    .then_with(|| a.backend_priority.cmp(&b.backend_priority))
                    .then_with(|| a.origin.origin_bias().cmp(&b.origin.origin_bias()))
                    .then_with(|| b.provider_service_id.cmp(&a.provider_service_id))
                    .then_with(|| b.provider_owner_id.cmp(&a.provider_owner_id))
            })
    }

    pub(crate) fn has_gateway_capability(&self, gateway_id: &str, capability_id: &str) -> bool {
        self.routes.iter().any(|route| {
            route.gateway_id == gateway_id && route.backend_capability_id == capability_id
        })
    }
}

pub(crate) fn descriptor_engine_gateways(descriptor: &PluginDescriptor) -> Vec<String> {
    let mut out: Vec<String> = descriptor_gateway_capabilities(descriptor)
        .into_iter()
        .filter_map(|gateway| gateway_provider_service_id(descriptor, &gateway).map(|_| gateway.gateway_id))
        .collect();
    out.sort();
    out.dedup();
    out
}

pub(crate) fn descriptor_max_gateway_priority(descriptor: &PluginDescriptor) -> i32 {
    descriptor_gateway_capabilities(descriptor)
        .into_iter()
        .filter(|gateway| gateway_provider_service_id(descriptor, gateway).is_some())
        .map(|gateway| gateway.backend_priority)
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use newengine_plugin_api::{CapabilityDesc, CapabilityKind, CapabilityRole, PluginDescriptor, PluginKind};

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
}
