#![forbid(unsafe_op_in_unsafe_fn)]

use super::super::state::{bump_services_generation, ctx};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EngineGatewaySelectionPolicy {
    pub gateway_id: String,
    pub preferred_system_tags: Vec<String>,
    pub forbidden_system_tags: Vec<String>,
    pub preference_bonus: i32,
    pub owner_id: String,
}

impl EngineGatewaySelectionPolicy {
    pub fn new(gateway_id: impl Into<String>, owner_id: impl Into<String>) -> Self {
        Self {
            gateway_id: gateway_id.into(),
            owner_id: owner_id.into(),
            ..Default::default()
        }
    }

    pub fn prefer_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.preferred_system_tags
            .extend(tags.into_iter().map(Into::into));
        normalize(&mut self.preferred_system_tags);
        self
    }

    pub fn forbid_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.forbidden_system_tags
            .extend(tags.into_iter().map(Into::into));
        normalize(&mut self.forbidden_system_tags);
        self
    }

    pub fn preference_bonus(mut self, value: i32) -> Self {
        self.preference_bonus = value.max(0);
        self
    }
}

pub fn install_engine_gateway_selection_policy(
    mut policy: EngineGatewaySelectionPolicy,
) -> Result<(), String> {
    crate::host_context::reject_topology_mutation_from_host_callback(
        "install_gateway_selection_policy",
    )?;
    let gateway_id = policy.gateway_id.trim().to_owned();
    if !newengine_service_api::is_engine_service_gateway_id(&gateway_id) {
        return Err(format!(
            "selection policy gateway id is invalid: '{}'",
            policy.gateway_id
        ));
    }
    let owner_id = policy.owner_id.trim().to_owned();
    if owner_id.is_empty() {
        return Err("selection policy owner_id must not be empty".to_owned());
    }
    normalize(&mut policy.preferred_system_tags);
    normalize(&mut policy.forbidden_system_tags);
    policy.gateway_id = gateway_id.clone();
    policy.owner_id = owner_id;
    policy.preference_bonus = policy.preference_bonus.max(0);

    let c = ctx();
    let mut policies = match c.gateway_selection_policies.lock() {
        Ok(value) => value,
        Err(poisoned) => poisoned.into_inner(),
    };
    policies.insert(gateway_id, policy);
    drop(policies);
    bump_services_generation();
    Ok(())
}

pub fn clear_engine_gateway_selection_policies() {
    if let Err(error) = crate::host_context::reject_topology_mutation_from_host_callback(
        "clear_gateway_selection_policies",
    ) {
        newengine_ulog_api::ulog::warn!("{}", error);
        return;
    }
    let c = ctx();
    let mut policies = match c.gateway_selection_policies.lock() {
        Ok(value) => value,
        Err(poisoned) => poisoned.into_inner(),
    };
    if policies.is_empty() {
        return;
    }
    policies.clear();
    drop(policies);
    bump_services_generation();
}

fn normalize(tags: &mut Vec<String>) {
    *tags = tags
        .drain(..)
        .filter_map(|tag| newengine_service_api::normalize_system_tag(&tag))
        .collect();
    tags.sort();
    tags.dedup();
}
