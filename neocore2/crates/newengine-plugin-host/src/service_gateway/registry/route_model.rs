use super::*;
use super::facts::{GatewayOverrideMode, GatewayPolicyFact, GatewayProviderOrigin};

#[inline]
pub(super) fn route_allowed_by_policy(
    override_mode: GatewayOverrideMode,
    origin: GatewayProviderOrigin,
) -> bool {
    match override_mode {
        GatewayOverrideMode::Open | GatewayOverrideMode::ProfileControlled => true,
        GatewayOverrideMode::Locked => matches!(origin, GatewayProviderOrigin::EngineRuntime),
    }
}

pub(super) fn merge_system_tags(
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
pub(super) fn route_blocked_by_selection_policy(
    route_tags: &[String],
    policy: Option<&GatewayPolicyFact>,
) -> bool {
    policy.is_some_and(|policy| {
        policy
            .forbidden_system_tags
            .iter()
            .any(|forbidden| route_tags.iter().any(|tag| tag == forbidden))
    })
}

#[inline]
pub(super) fn selection_policy_score_bonus(
    route_tags: &[String],
    policy: Option<&GatewayPolicyFact>,
) -> i64 {
    let Some(policy) = policy else { return 0; };
    if policy.preference_bonus <= 0 {
        return 0;
    }
    let matched = policy
        .preferred_system_tags
        .iter()
        .filter(|preferred| route_tags.iter().any(|tag| tag == *preferred))
        .count() as i64;
    matched * i64::from(policy.preference_bonus)
}

#[inline]
pub(super) fn route_gateway_matches_declared_kind(
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
pub(super) fn route_matches_query(route: &ActiveGatewayRoute, requested_gateway_id: &str) -> bool {
    route.gateway_id == requested_gateway_id
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveGatewayRoute {
    pub(crate) gateway_id: String,
    pub(crate) service_kind: String,
    pub(crate) provider_service_id: String,
    pub(crate) provider_route_id: Option<String>,
    pub(crate) provider_abi: Option<String>,
    pub(crate) provider_owner_id: String,
    pub(crate) backend_capability_id: String,
    pub(crate) backend_priority: i32,
    pub(crate) origin: GatewayProviderOrigin,
    pub(crate) override_mode: GatewayOverrideMode,
    pub(crate) active_score: i64,
    pub(crate) system_tags: Vec<String>,
}

impl ActiveGatewayRoute {
    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub(super) fn new(
        gateway_id: String,
        service_kind: String,
        provider_service_id: String,
        provider_route_id: Option<String>,
        provider_abi: Option<String>,
        provider_owner_id: String,
        backend_capability_id: String,
        backend_priority: i32,
        origin: GatewayProviderOrigin,
        route_tags: Vec<String>,
        policy: Option<&GatewayPolicyFact>,
    ) -> Option<Self> {
        if !route_gateway_matches_declared_kind(&gateway_id, &service_kind, &route_tags) {
            newengine_ulog_api::ulog::warn!(
                "gateways: ignoring route with mixed domain levels gateway='{}' service_kind='{}' gateway_domain='{}' service='{}' owner='{}'",
                gateway_id,
                service_kind,
                engine_gateway_domain(&gateway_id).unwrap_or_else(|| "<invalid>".to_owned()),
                provider_service_id,
                provider_owner_id,
            );
            return None;
        }

        if route_blocked_by_selection_policy(&route_tags, policy) {
            newengine_ulog_api::ulog::info!(
                "gateways: route blocked by host capability policy gateway='{}' service='{}' owner='{}' tags='{}' policy_owner='{}'",
                gateway_id,
                provider_service_id,
                provider_owner_id,
                route_tags.join(","),
                policy.map(|policy| policy.owner_id.as_str()).unwrap_or("<none>"),
            );
            return None;
        }
        let selection_bonus = selection_policy_score_bonus(&route_tags, policy);
        let system_tags = merge_system_tags(route_tags, policy);
        let override_mode = policy
            .and_then(|policy| policy.override_mode)
            .or_else(|| GatewayOverrideMode::from_system_tags(&system_tags))
            .unwrap_or(GatewayOverrideMode::Open);

        if !route_allowed_by_policy(override_mode, origin) {
            newengine_ulog_api::ulog::warn!(
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

        let active_score = origin.origin_bias() + i64::from(backend_priority) + selection_bonus;
        Some(Self {
            gateway_id,
            service_kind,
            provider_service_id,
            provider_route_id,
            provider_abi,
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
