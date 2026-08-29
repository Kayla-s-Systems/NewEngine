use newengine_ui_api::{UiDrawList, UiLayerCompositionPlan, UiLayerDomain};

/// Host-side retained draw cache for one logical UI presentation domain.
///
/// The cache is deliberately provider/backend neutral. It remembers the logical
/// composition plan and an atlas-free draw packet; texture deltas are transport
/// events and therefore are never replayed from the retained cache.
#[derive(Debug)]
pub(crate) struct RetainedUiLayerCache {
    domain: UiLayerDomain,
    plan: Option<UiLayerCompositionPlan>,
    draw: Option<UiDrawList>,
}

impl RetainedUiLayerCache {
    #[inline]
    pub(crate) fn new(domain: UiLayerDomain) -> Self {
        Self {
            domain,
            plan: None,
            draw: None,
        }
    }

    #[inline]
    pub(crate) fn draw(&self) -> Option<&UiDrawList> {
        self.draw.as_ref()
    }

    #[inline]
    pub(crate) fn cloned_draw(&self) -> Option<UiDrawList> {
        self.draw.clone()
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.draw.is_none()
    }

    #[inline]
    pub(crate) fn plan_matches(&self, plan: &UiLayerCompositionPlan) -> bool {
        self.plan
            .as_ref()
            .is_some_and(|cached| cached.cache_identity_matches(plan))
    }

    #[inline]
    pub(crate) fn invalidation_matches(&self, plan: &UiLayerCompositionPlan) -> bool {
        self.plan
            .as_ref()
            .is_some_and(|cached| cached.invalidation_revision == plan.invalidation_revision)
    }

    #[inline]
    pub(crate) fn needs_refresh(
        &self,
        plan: &UiLayerCompositionPlan,
        force_refresh: bool,
        active_animation: bool,
    ) -> bool {
        debug_assert_eq!(plan.domain, self.domain);
        force_refresh
            || active_animation
            || self.is_empty()
            || !self.plan_matches(plan)
            || !self.invalidation_matches(plan)
    }

    pub(crate) fn store(&mut self, plan: UiLayerCompositionPlan, draw: &UiDrawList) {
        debug_assert_eq!(plan.domain, self.domain);
        let mut cached = draw.clone();
        cached.texture_delta.clear();
        self.plan = Some(plan);
        self.draw = Some(cached);
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.plan = None;
        self.draw = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_plan(revision: u64) -> UiLayerCompositionPlan {
        let mut plan = UiLayerCompositionPlan::disabled(
            UiLayerDomain::GameViewport,
            "engine.render.viewport.primary",
            1,
        );
        plan.surface_ids = vec!["game.hud".to_owned()];
        plan.invalidation_revision = revision;
        plan
    }

    #[test]
    fn retained_cache_refreshes_on_domain_content_invalidation() {
        let mut cache = RetainedUiLayerCache::new(UiLayerDomain::GameViewport);
        let plan = game_plan(3);
        assert!(cache.needs_refresh(&plan, false, false));
        cache.store(plan.clone(), &UiDrawList::new());
        assert!(!cache.needs_refresh(&plan, false, false));

        let invalidated = game_plan(4);
        assert!(cache.needs_refresh(&invalidated, false, false));
    }

    #[test]
    fn retained_cache_refreshes_when_surface_topology_changes() {
        let mut cache = RetainedUiLayerCache::new(UiLayerDomain::GameViewport);
        let plan = game_plan(1);
        cache.store(plan.clone(), &UiDrawList::new());

        let mut changed = plan;
        changed.surface_ids.push("game.pause".to_owned());
        assert!(cache.needs_refresh(&changed, false, false));
    }

    #[test]
    fn retained_cache_never_replays_texture_delta() {
        let mut cache = RetainedUiLayerCache::new(UiLayerDomain::GameViewport);
        let plan = game_plan(1);
        let mut draw = UiDrawList::new();
        draw.texture_delta
            .free
            .push(newengine_ui_api::UiTexId::new(99));
        cache.store(plan, &draw);
        assert!(cache
            .draw()
            .expect("cached draw")
            .texture_delta
            .free
            .is_empty());
    }
}
