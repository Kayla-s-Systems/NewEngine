#![forbid(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;

/// Pure provider candidate consumed by [`CompositionSolver`].
///
/// The solver deliberately knows nothing about plugin loading, host globals,
/// descriptor parsing or service registration. Callers materialize inventory
/// facts into candidates and receive one immutable [`CompositionPlan`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompositionCandidate {
    pub(crate) gateway_id: String,
    pub(crate) candidate_id: String,
    pub(crate) provider_owner_id: String,
    pub(crate) backend_priority: i32,
    pub(crate) origin_bias: i64,
    pub(crate) score: i64,
}

impl CompositionCandidate {
    #[inline]
    pub(crate) fn new(
        gateway_id: impl Into<String>,
        candidate_id: impl Into<String>,
        provider_owner_id: impl Into<String>,
        backend_priority: i32,
        origin_bias: i64,
        preference_bonus: i64,
    ) -> Self {
        Self {
            gateway_id: gateway_id.into(),
            candidate_id: candidate_id.into(),
            provider_owner_id: provider_owner_id.into(),
            backend_priority,
            origin_bias,
            score: CompositionSolver::score(origin_bias, backend_priority, preference_bonus),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompositionSelection {
    pub(crate) gateway_id: String,
    pub(crate) candidate_id: String,
    pub(crate) provider_owner_id: String,
    pub(crate) backend_priority: i32,
    pub(crate) origin_bias: i64,
    pub(crate) score: i64,
}

impl From<CompositionCandidate> for CompositionSelection {
    fn from(candidate: CompositionCandidate) -> Self {
        Self {
            gateway_id: candidate.gateway_id,
            candidate_id: candidate.candidate_id,
            provider_owner_id: candidate.provider_owner_id,
            backend_priority: candidate.backend_priority,
            origin_bias: candidate.origin_bias,
            score: candidate.score,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GatewayCompositionPlan {
    gateway_id: String,
    active: CompositionSelection,
    shadowed: Vec<CompositionSelection>,
}

impl GatewayCompositionPlan {
    #[inline]
    pub(crate) fn active(&self) -> &CompositionSelection {
        &self.active
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn shadowed(&self) -> &[CompositionSelection] {
        &self.shadowed
    }
}

/// Immutable output of provider composition.
///
/// There is intentionally no mutation API. A new inventory/policy generation
/// must be resolved into a new plan and atomically replace the previous snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CompositionPlan {
    gateways: Vec<GatewayCompositionPlan>,
}

impl CompositionPlan {
    #[inline]
    pub(crate) fn selected(&self, gateway_id: &str) -> Option<&CompositionSelection> {
        self.gateways
            .binary_search_by(|entry| entry.gateway_id.as_str().cmp(gateway_id))
            .ok()
            .map(|index| self.gateways[index].active())
    }

    pub(crate) fn gateway_ids(&self) -> Vec<String> {
        self.gateways
            .iter()
            .map(|entry| entry.gateway_id.clone())
            .collect()
    }
}

/// Single authority for provider winner selection.
///
/// All callers use the same score and deterministic tie-break policy:
/// 1. highest total score;
/// 2. highest backend priority;
/// 3. highest host-assigned origin bias;
/// 4. lexicographically smallest stable candidate id;
/// 5. lexicographically smallest owner id.
pub(crate) struct CompositionSolver;

impl CompositionSolver {
    #[inline]
    pub(crate) const fn score(
        origin_bias: i64,
        backend_priority: i32,
        preference_bonus: i64,
    ) -> i64 {
        origin_bias + backend_priority as i64 + preference_bonus
    }

    pub(crate) fn resolve<I>(candidates: I) -> CompositionPlan
    where
        I: IntoIterator<Item = CompositionCandidate>,
    {
        let mut by_gateway: BTreeMap<String, Vec<CompositionCandidate>> = BTreeMap::new();
        for candidate in candidates {
            if candidate.gateway_id.trim().is_empty() || candidate.candidate_id.trim().is_empty() {
                continue;
            }
            by_gateway
                .entry(candidate.gateway_id.clone())
                .or_default()
                .push(candidate);
        }

        let mut gateways = Vec::with_capacity(by_gateway.len());
        for (gateway_id, mut candidates) in by_gateway {
            candidates.sort_by(|a, b| {
                b.score
                    .cmp(&a.score)
                    .then_with(|| b.backend_priority.cmp(&a.backend_priority))
                    .then_with(|| b.origin_bias.cmp(&a.origin_bias))
                    .then_with(|| a.candidate_id.cmp(&b.candidate_id))
                    .then_with(|| a.provider_owner_id.cmp(&b.provider_owner_id))
            });

            let Some(active) = candidates.first().cloned() else {
                continue;
            };
            let shadowed = candidates
                .into_iter()
                .skip(1)
                .map(CompositionSelection::from)
                .collect();
            gateways.push(GatewayCompositionPlan {
                gateway_id,
                active: active.into(),
                shadowed,
            });
        }

        CompositionPlan { gateways }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, priority: i32) -> CompositionCandidate {
        CompositionCandidate::new("engine.render", id, id, priority, 20_000, 0)
    }

    #[test]
    fn higher_score_wins() {
        let plan = CompositionSolver::resolve([candidate("low", 10), candidate("high", 20)]);
        let selected = plan.selected("engine.render").expect("render selection");
        assert_eq!(selected.candidate_id, "high");
        assert_eq!(selected.score, 20_020);
    }

    #[test]
    fn tie_break_is_independent_of_inventory_order() {
        let a = candidate("a", 10);
        let b = candidate("b", 10);
        let forward = CompositionSolver::resolve([b.clone(), a.clone()]);
        let reverse = CompositionSolver::resolve([a, b]);

        assert_eq!(
            forward
                .selected("engine.render")
                .map(|v| v.candidate_id.as_str()),
            Some("a")
        );
        assert_eq!(forward, reverse);
    }

    #[test]
    fn plan_is_sorted_for_binary_lookup() {
        let plan = CompositionSolver::resolve([
            CompositionCandidate::new("engine.ui", "ui", "ui", 0, 20_000, 0),
            CompositionCandidate::new("engine.audio", "audio", "audio", 0, 20_000, 0),
        ]);
        assert_eq!(plan.gateway_ids(), vec!["engine.audio", "engine.ui"]);
        assert_eq!(
            plan.selected("engine.ui").map(|v| v.candidate_id.as_str()),
            Some("ui")
        );
    }
}
