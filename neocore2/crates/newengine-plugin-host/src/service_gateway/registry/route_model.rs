use super::facts::{GatewayOverrideMode, GatewayPolicyFact, GatewayProviderOrigin};
use super::*;
use newengine_service_api::{parse_versioned_contract_id, CompositionCandidate, CompositionSolver};

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
    let Some(policy) = policy else {
        return 0;
    };
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
    pub(crate) capability_version: Option<u32>,
    pub(crate) contract_id: Option<String>,
    pub(crate) contract_version: Option<u32>,
    pub(crate) backend_priority: i32,
    pub(crate) origin: GatewayProviderOrigin,
    pub(crate) override_mode: GatewayOverrideMode,
    pub(crate) active_score: i64,
    pub(crate) selection_bonus: i64,
    pub(crate) system_tags: Vec<String>,
    pub(crate) selection_key: String,
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
        capability_version: Option<u32>,
        contract_id: Option<String>,
        contract_version: Option<u32>,
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
            if let Some(policy) = policy {
                newengine_ulog_api::ulog::debug!(
                    "gateways: route blocked by selection policy gateway='{}' service='{}' provider_owner='{}' policy_owner='{}' forbidden_tags={:?}",
                    gateway_id,
                    provider_service_id,
                    provider_owner_id,
                    policy.owner_id,
                    policy.forbidden_system_tags,
                );
            }
            return None;
        }
        let selection_bonus = selection_policy_score_bonus(&route_tags, policy);
        if selection_bonus > 0 {
            if let Some(policy) = policy {
                newengine_ulog_api::ulog::debug!(
                    "gateways: selection policy bonus gateway='{}' service='{}' provider_owner='{}' policy_owner='{}' bonus={} preferred_tags={:?}",
                    gateway_id,
                    provider_service_id,
                    provider_owner_id,
                    policy.owner_id,
                    selection_bonus,
                    policy.preferred_system_tags,
                );
            }
        }
        let system_tags = merge_system_tags(route_tags, policy);
        let override_mode = policy
            .and_then(|policy| policy.override_mode)
            .or_else(|| GatewayOverrideMode::from_system_tags(&system_tags))
            .unwrap_or(GatewayOverrideMode::Open);

        if !route_allowed_by_policy(override_mode, origin) {
            return None;
        }

        let active_score =
            CompositionSolver::score(origin.origin_bias(), backend_priority, selection_bonus);
        let selection_key = format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}",
            gateway_id,
            provider_service_id,
            provider_route_id.as_deref().unwrap_or(""),
            provider_owner_id,
        );
        Some(Self {
            gateway_id,
            service_kind,
            provider_service_id,
            provider_route_id,
            provider_abi,
            provider_owner_id,
            backend_capability_id,
            capability_version,
            contract_id,
            contract_version,
            backend_priority,
            origin,
            override_mode,
            active_score,
            selection_bonus,
            system_tags,
            selection_key,
        })
    }

    pub(super) fn composition_candidate(&self) -> CompositionCandidate {
        let mut candidate = CompositionCandidate::new(
            self.gateway_id.clone(),
            self.selection_key.clone(),
            self.provider_owner_id.clone(),
            self.backend_priority,
            self.origin.origin_bias(),
            self.selection_bonus,
        )
        .with_capability(self.backend_capability_id.clone())
        .with_tags(self.system_tags.clone());

        if let Some(version) = self.capability_version {
            candidate = candidate.with_capability_version(version);
        }

        match (self.contract_id.as_deref(), self.contract_version) {
            (Some(contract_id), Some(version)) => {
                candidate = candidate.with_contract(contract_id.to_owned(), version);
            }
            (Some(contract_id), None) => {
                candidate = candidate.with_contract_id(contract_id.to_owned());
            }
            _ => {
                if let Some((contract_id, version)) = self
                    .provider_abi
                    .as_deref()
                    .and_then(parse_versioned_contract_id)
                {
                    candidate = candidate.with_contract(contract_id, version);
                }
            }
        }
        candidate
    }
}
