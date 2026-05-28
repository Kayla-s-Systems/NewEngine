#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::Path;

use newengine_plugin_api::PluginDescriptor;
use newengine_service_api::{
    engine_gateway_domain, engine_gateway_matches_service_kind, system_tag,
};
#[cfg(test)]
use newengine_service_api::EngineServiceKind;

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
pub(crate) struct GatewayProviderRouteFact {
    pub(crate) gateway_id: String,
    pub(crate) service_kind: String,
    pub(crate) provider_service_id: String,
    pub(crate) provider_route_id: String,
    pub(crate) provider_owner_id: String,
    pub(crate) backend_capability_id: String,
    pub(crate) backend_priority: i32,
    pub(crate) system_tags: Vec<String>,
}

impl GatewayProviderRouteFact {
    #[cfg(test)]
    #[inline]
    pub(crate) fn new(
        gateway_id: String,
        service_kind: EngineServiceKind,
        provider_service_id: String,
        provider_route_id: String,
        provider_owner_id: String,
        backend_capability_id: String,
        backend_priority: i32,
    ) -> Self {
        Self::new_dynamic(
            gateway_id,
            service_kind.as_str().to_owned(),
            provider_service_id,
            provider_route_id,
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
        provider_route_id: String,
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
            provider_route_id,
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
    EngineRuntime,
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
            Self::EngineRuntime => "engine-runtime",
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
            Self::EngineRuntime => 10_000,
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
        GatewayOverrideMode::Locked => matches!(origin, GatewayProviderOrigin::EngineRuntime),
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

#[inline]
fn route_gateway_matches_declared_kind(
    gateway_id: &str,
    service_kind: &str,
    _system_tags: &[String],
) -> bool {
    // Provider implementation names are metadata, not API domains.
    // A render provider may publish `provider_route = engine.render.vulkan`,
    // but the service route consumed by the engine remains `engine.render`.
    engine_gateway_matches_service_kind(gateway_id, service_kind)
}

#[inline]
fn route_matches_query(route: &ActiveGatewayRoute, requested_gateway_id: &str) -> bool {
    route.gateway_id == requested_gateway_id
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveGatewayRoute {
    pub(crate) gateway_id: String,
    pub(crate) service_kind: String,
    pub(crate) provider_service_id: String,
    pub(crate) provider_route_id: Option<String>,
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
        provider_route_id: Option<String>,
        provider_owner_id: String,
        backend_capability_id: String,
        backend_priority: i32,
        origin: GatewayProviderOrigin,
        route_tags: Vec<String>,
        policy: Option<&GatewayPolicyFact>,
    ) -> Option<Self> {
        if !route_gateway_matches_declared_kind(&gateway_id, &service_kind, &route_tags) {
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
            provider_route_id,
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
        gateway_provider_routes: &[GatewayProviderRouteFact],
    ) -> Self {
        Self::from_facts_with_policy(descriptors, services, gateway_provider_routes, &[])
    }

    pub(crate) fn from_facts_with_policy(
        descriptors: &[PluginDescriptorFact],
        services: &[RegisteredServiceFact],
        gateway_provider_routes: &[GatewayProviderRouteFact],
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
                    gateway.provider_route_id,
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

        for gateway in gateway_provider_routes {
            let registered = services.iter().any(|service| {
                service.service_id == gateway.provider_service_id && service.owner_plugin_id.is_none()
            });
            if !registered {
                skipped_unregistered += 1;
                log::trace!(
                    "gateways: engine-runtime route skipped because service is not registered gateway='{}' service='{}' owner='{}'",
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
                Some(gateway.provider_route_id.clone()),
                gateway.provider_owner_id.clone(),
                gateway.backend_capability_id.clone(),
                gateway.backend_priority,
                GatewayProviderOrigin::EngineRuntime,
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
            "gateways: registry rebuilt descriptors={} services={} host_routes={} policy_facts={} routes={} skipped_unregistered={}",
            descriptors.len(),
            services.len(),
            gateway_provider_routes.len(),
            policy_facts.len(),
            registry.routes.len(),
            skipped_unregistered
        );
        for gateway_id in registry.gateway_ids() {
            if let Some(route) = registry.resolve_route(&gateway_id) {
                log::trace!(
                    "gateways: active route gateway='{}' service='{}' provider_route='{}' owner='{}' kind='{}' origin='{}' mode='{}' prio={} score={} tags='{}'",
                    route.gateway_id,
                    route.provider_service_id,
                    route.provider_route_id.as_deref().unwrap_or("<provider-route-unset>"),
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
            .filter(|route| route_matches_query(route, gateway_id))
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
            route_matches_query(route, gateway_id) && route.backend_capability_id == capability_id
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
mod tests;
