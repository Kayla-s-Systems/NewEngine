// Provider-neutral retained UI presentation-layer contracts.
// Included flat from lib.rs so runtime, editor and gameplay code share one engine-level DTO surface.

/// Logical native presentation target used by system-level UI domains.
pub const UI_PRESENTATION_TARGET_PRIMARY: &str = "engine.render.surface.primary";

/// Provider-neutral render/composition plan for one retained UI domain.
///
/// The plan intentionally carries only logical surface identities. It must never expose
/// framebuffer/texture/descriptor handles; mapping to physical resources remains downstream
/// in the renderer/RenderGraph compiler.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiLayerCompositionPlan {
    pub version: u32,
    pub frame_index: u64,
    pub domain: UiLayerDomain,
    /// Logical target/viewport identity for the domain.
    pub target_surface_id: String,
    /// Ordered visible provider surface ids for this domain.
    pub surface_ids: Vec<String>,
    /// Domain-scoped retained-state invalidation epoch.
    pub invalidation_revision: u64,
    /// Surface currently owning UI focus/input, if any.
    pub input_owner_surface_id: Option<String>,
    /// Top modal surface, if any.
    pub modal_surface_id: Option<String>,
}

impl UiLayerCompositionPlan {
    #[inline]
    pub fn disabled(
        domain: UiLayerDomain,
        target_surface_id: impl Into<String>,
        frame_index: u64,
    ) -> Self {
        Self {
            version: 1,
            frame_index,
            domain,
            target_surface_id: target_surface_id.into(),
            ..Self::default()
        }
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        !self.surface_ids.is_empty()
    }

    #[inline]
    pub fn cache_identity_matches(&self, other: &Self) -> bool {
        self.domain == other.domain
            && self.target_surface_id == other.target_surface_id
            && self.surface_ids == other.surface_ids
    }

    /// Materialize the resolved composition plan into the renderer-neutral draw packet.
    /// Provider layout has already happened; the renderer receives only logical routing data
    /// plus the immutable draw stream for this domain.
    #[inline]
    pub fn draw_packet(&self, draw_list: UiDrawList) -> UiLayerDrawPacket {
        UiLayerDrawPacket::new(self.domain, self.frame_index, draw_list)
            .with_target(self.target_surface_id.clone())
            .with_surfaces(self.surface_ids.clone())
            .with_invalidation_revision(self.invalidation_revision)
    }

    /// Add an engine-owned overlay surface without destroying authored stack order.
    /// Duplicate ids are ignored so diagnostics can safely opt into a game/editor lane.
    pub fn with_overlay_surface(mut self, surface_id: impl Into<String>) -> Self {
        let surface_id = surface_id.into();
        let surface_id = surface_id.trim();
        if !surface_id.is_empty() && !self.surface_ids.iter().any(|id| id == surface_id) {
            self.surface_ids.push(surface_id.to_owned());
        }
        self
    }
}

#[cfg(test)]
mod ui_layer_contract_tests {
    use super::*;

    #[test]
    fn layer_domain_is_provider_and_backend_neutral() {
        assert_eq!(UiLayerDomain::System.as_str(), "system");
        assert_eq!(UiLayerDomain::GameViewport.as_str(), "game_viewport");
        assert_eq!(UiLayerDomain::Editor.as_str(), "editor");
        assert_eq!(UiLayerDomain::Debug.as_str(), "debug");
    }

    #[test]
    fn composition_plan_cache_identity_ignores_frame_and_content_revision() {
        let mut a = UiLayerCompositionPlan::disabled(
            UiLayerDomain::GameViewport,
            "engine.render.viewport.primary",
            10,
        );
        a.surface_ids = vec!["game.hud".to_owned()];
        let mut b = a.clone();
        b.frame_index = 11;
        b.invalidation_revision = 7;
        assert!(a.cache_identity_matches(&b));

        b.surface_ids.push("game.pause".to_owned());
        assert!(!a.cache_identity_matches(&b));
    }

    #[test]
    fn overlay_injection_is_ordered_and_deduplicated() {
        let mut plan = UiLayerCompositionPlan::disabled(
            UiLayerDomain::GameViewport,
            "engine.render.viewport.primary",
            1,
        );
        plan.surface_ids.push("game.hud".to_owned());
        plan = plan
            .with_overlay_surface("runtime.debug_overlay")
            .with_overlay_surface("runtime.debug_overlay");
        assert_eq!(
            plan.surface_ids,
            vec!["game.hud".to_owned(), "runtime.debug_overlay".to_owned()]
        );
    }
}
