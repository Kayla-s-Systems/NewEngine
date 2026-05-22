#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use crate::plugin_manager::PluginManagerBridge;
use crate::scene_bridge::SceneBridge;
use crate::viewport_bridge::ViewportBridge;

use super::gpu::{MaterialGpuPipelineKey, MaterialGpuPipelineProvider};
use newengine_render_feature_api::{LightExtractionProvider, RenderDrawListProvider};
use super::state::{
    RenderBridgeState, RenderDiagnosticsRuntimeState, RenderFeatureProviderState,
    RenderFrameRuntimeState, RenderGpuSceneState, RenderMenuRuntimeState, RenderRuntimeProfileState, RenderShadowRuntimeState, RenderViewportState,
};

/// Engine-side render composition root.
///
/// This type is intentionally a composition root, not a renderer backend. It owns
/// runtime-facing state, delegates frame orchestration to module_impl, and submits
/// a typed RenderFrameEnvelope into the backend RenderApi adapter.
pub struct RuntimeRenderController {
    pub(super) bridges: RenderBridgeState,
    pub(super) viewport: RenderViewportState,
    pub(super) shadows: RenderShadowRuntimeState,
    pub(super) features: RenderFeatureProviderState,
    pub(super) gpu: RenderGpuSceneState,
    pub(super) frame: RenderFrameRuntimeState,
    pub(super) diagnostics: RenderDiagnosticsRuntimeState,
    pub(super) menu: RenderMenuRuntimeState,
    pub(super) runtime_profile: RenderRuntimeProfileState,
}

impl RuntimeRenderController {

    #[inline]
    pub(crate) fn runtime_profile(&self) -> &super::runtime_profile::RenderRuntimeProfile {
        &self.runtime_profile.profile
    }

    pub(super) fn restore_playable_view_after_menu_close(&mut self) {
        let restore_viewport = self.runtime_profile().menu.restore_viewport_pass_on_close;
        let invalidate_shadow_cache = self.runtime_profile().menu.invalidate_shadow_cache_on_close;
        let restore_input = self.runtime_profile().menu.restore_gameplay_input_on_close;
        if restore_viewport && self.viewport.pass_disabled {
            log::warn!(
                "render controller: menu restore reopened viewport GPU pass after pause/settings close"
            );
            self.viewport.pass_disabled = false;
        }
        if invalidate_shadow_cache {
            self.shadows.cache_valid = false;
        }
        if restore_input {
            self.frame.input_systems.set_enabled(
                crate::input_systems::InputRuntimeSystem::Actions,
                true,
                "menu restore contract",
                self.frame.frame_index,
            );
            self.frame.input_systems.set_enabled(
                crate::input_systems::InputRuntimeSystem::GameplayMovement,
                true,
                "menu restore contract",
                self.frame.frame_index,
            );
            self.frame.input_systems.set_enabled(
                crate::input_systems::InputRuntimeSystem::CameraLook,
                true,
                "menu restore contract",
                self.frame.frame_index,
            );
        }
        newengine_core::crash::record_breadcrumb(
            "render controller: menu restore contract applied".to_owned(),
        );
    }

    /// Registers a profile-owned draw-list provider.
    ///
    /// Engine-runtime has no built-in draw-list defaults: the active profile owns
    /// terrain/mesh/UI extraction policy and must register it explicitly.
    #[inline]
    pub fn with_draw_list_provider(
        mut self,
        provider: Arc<dyn RenderDrawListProvider>,
    ) -> Self {
        self.features.draw_list_providers.register_provider(provider);
        self
    }

    /// Registers a profile-owned light extraction provider.
    ///
    /// Shadow planning policy belongs to the profile/render feature pack, not to
    /// the renderer backend adapter or reusable engine runtime.
    #[inline]
    pub fn with_light_extraction_provider(
        mut self,
        provider: Arc<dyn LightExtractionProvider>,
    ) -> Self {
        self.features.light_extraction_providers.register_provider(provider);
        self
    }

    /// Registers a host-side material-domain provider used by the reusable render
    /// controller. Profiles/features own these providers; engine-runtime only
    /// stores the trait object.
    #[inline]
    pub fn with_material_pipeline_provider(
        mut self,
        provider: Box<dyn MaterialGpuPipelineProvider>,
    ) -> Self {
        self.gpu.material.registry.register_provider(provider);
        self
    }

    /// Selects the material domain used by the current lit mesh/shadow passes.
    ///
    /// This keeps the reusable render controller free from GameReady shader path
    /// knowledge: the profile chooses a domain key and registers the provider.
    #[inline]
    pub fn with_primary_lit_material_domain(
        mut self,
        key: MaterialGpuPipelineKey,
    ) -> Self {
        self.gpu.material.primary_lit_pipeline_key = Some(key);
        self
    }


    /// Enables or disables one semantic input system at runtime.
    ///
    /// This is the in-process control point used by scripted states such as cutscenes,
    /// dialogue or loading gates. Disabling a higher-level system does not stop raw
    /// device polling; it only suppresses the matching semantic effects.
    #[inline]
    pub fn set_input_system_enabled(
        &mut self,
        system: crate::input_systems::InputRuntimeSystem,
        enabled: bool,
        reason: impl Into<String>,
    ) {
        self.frame.input_systems.set_enabled(system, enabled, reason, self.frame.frame_index);
    }

    #[inline]
    pub fn input_systems_snapshot(&self) -> crate::input_systems::InputRuntimeSystemsSnapshot {
        self.frame.input_systems.snapshot(self.frame.frame_index)
    }

    #[inline]
    pub fn log_input_systems_snapshot(&self, reason: &str) {
        self.frame.input_systems.log_explicit_snapshot(self.frame.frame_index, reason);
    }

    pub(super) fn disable_viewport_pass(
        &mut self,
        phase: &'static str,
        error: impl std::fmt::Display,
    ) {
        let message = error.to_string();
        if !self.viewport.pass_disabled {
            log::error!(
                "render controller: viewport GPU pass disabled phase='{}' err='{}'",
                phase,
                message
            );
            newengine_core::crash::record_breadcrumb(format!(
                "render controller: viewport GPU pass disabled phase='{}' err='{}'",
                phase, message
            ));
        }
        self.viewport.pass_disabled = true;
    }

    #[inline]
    pub fn new(
        viewport_bridge: Arc<ViewportBridge>,
        plugins_bridge: Arc<PluginManagerBridge>,
        scene_bridge: Arc<SceneBridge>,
    ) -> Self {
        Self {
            bridges: RenderBridgeState::new(viewport_bridge, plugins_bridge, scene_bridge),
            viewport: RenderViewportState::new(),
            shadows: RenderShadowRuntimeState::new(),
            features: RenderFeatureProviderState::new(),
            gpu: RenderGpuSceneState::new(),
            frame: RenderFrameRuntimeState::new(),
            diagnostics: RenderDiagnosticsRuntimeState::new(),
            menu: RenderMenuRuntimeState::new(),
            runtime_profile: RenderRuntimeProfileState::new(),
        }
    }
}
