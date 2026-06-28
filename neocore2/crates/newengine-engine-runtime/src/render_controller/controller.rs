#![forbid(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use serde::Deserialize;

use crate::plugin_manager::PluginManagerBridge;
use crate::scene_bridge::SceneBridge;
use crate::viewport_bridge::ViewportBridge;

use super::error_policy::RenderBackendFailureState;
use super::gpu::{MaterialGpuPipelineKey, MaterialGpuPipelineProvider};
use super::state::{
    RenderBridgeState, RenderDiagnosticsRuntimeState, RenderFeatureProviderState,
    RenderFrameRuntimeState, RenderGpuSceneState, RenderRuntimeProfileState,
    RenderShadowRuntimeState, RenderUiSurfaceRuntimeState, RenderViewportState,
};
use newengine_core::render::{RenderBackendCapabilities, RenderBackendStatus};
use newengine_render_feature_api::{LightExtractionProvider, RenderDrawListProvider};

/// Engine-side render composition root.
///
/// This type is intentionally a composition root, not a renderer backend. It owns
/// runtime-facing state, delegates frame orchestration to module_impl, and submits
/// a typed RenderFrameEnvelope into the backend RenderApi adapter.

#[derive(Clone, Debug, Default)]
pub(super) struct RenderRuntimeAppPolicy {
    pub(super) ui_only: bool,
    pub(super) viewport_pass: Option<bool>,
}

impl RenderRuntimeAppPolicy {
    pub(super) fn from_startup_config() -> Self {
        let mut policy = Self::default();
        let Some(startup) = newengine_core::startup::last_startup_config() else {
            return policy;
        };
        if let Some(value) = startup.plugins.get("engine.render") {
            if let Ok(config) = serde_json::from_value::<RenderPolicyConfig>(value.clone()) {
                policy.merge(config);
            }
        }
        if let Some(value) = startup
            .plugins
            .get("engine.runtime")
            .and_then(|value| value.get("render"))
        {
            if let Ok(config) = serde_json::from_value::<RenderPolicyConfig>(value.clone()) {
                policy.merge(config);
            }
        }
        if policy.ui_only {
            policy.viewport_pass = Some(false);
        }
        policy
    }

    fn merge(&mut self, config: RenderPolicyConfig) {
        if let Some(mode) = config.mode.as_deref() {
            if mode.eq_ignore_ascii_case("ui_only") || mode.eq_ignore_ascii_case("ui-only") {
                self.ui_only = true;
            }
        }
        if let Some(ui_only) = config.ui_only {
            self.ui_only = ui_only;
        }
        if let Some(viewport_pass) = config.viewport_pass {
            self.viewport_pass = Some(viewport_pass);
        }
    }
}

#[derive(Debug, Deserialize)]
struct RenderPolicyConfig {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    ui_only: Option<bool>,
    #[serde(default)]
    viewport_pass: Option<bool>,
}

pub struct RuntimeRenderController {
    pub(super) bridges: RenderBridgeState,
    pub(super) viewport: RenderViewportState,
    pub(super) shadows: RenderShadowRuntimeState,
    pub(super) features: RenderFeatureProviderState,
    pub(super) gpu: RenderGpuSceneState,
    pub(super) frame: RenderFrameRuntimeState,
    pub(super) diagnostics: RenderDiagnosticsRuntimeState,
    pub(super) ui: RenderUiSurfaceRuntimeState,
    pub(super) runtime_profile: RenderRuntimeProfileState,
    pub(super) backend_failure: RenderBackendFailureState,
    pub(super) app_policy: RenderRuntimeAppPolicy,
}

impl RuntimeRenderController {
    #[inline]
    pub(crate) fn runtime_profile(&self) -> &super::runtime_profile::RenderRuntimeProfile {
        &self.runtime_profile.profile
    }

    pub(crate) fn apply_backend_capability_profile(
        &mut self,
        capabilities: &RenderBackendCapabilities,
    ) {
        self.runtime_profile
            .apply_hardware_tier_once(capabilities.hardware_tier);
    }

    pub(crate) fn backend_status_snapshot(&self) -> RenderBackendStatus {
        self.backend_failure.snapshot()
    }

    pub(super) fn restore_playable_view_after_ui_close(&mut self) {
        let restore_viewport = self.runtime_profile().ui.restore_viewport_pass_on_close;
        let invalidate_shadow_cache = self.runtime_profile().ui.invalidate_shadow_cache_on_close;
        let restore_input = self.runtime_profile().ui.restore_gameplay_input_on_close;
        if restore_viewport && self.viewport.pass_disabled && !self.backend_render_disabled() {
            newengine_ulog_api::ulog::warn!(
                "render controller: UI restore reopened viewport GPU pass after UI close"
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
                "UI restore contract",
                self.frame.frame_index,
            );
            self.frame.input_systems.set_enabled(
                crate::input_systems::InputRuntimeSystem::GameplayMovement,
                true,
                "UI restore contract",
                self.frame.frame_index,
            );
            self.frame.input_systems.set_enabled(
                crate::input_systems::InputRuntimeSystem::CameraLook,
                true,
                "UI restore contract",
                self.frame.frame_index,
            );
        }
        newengine_core::crash::record_breadcrumb(
            "render controller: UI restore contract applied".to_owned(),
        );
    }

    /// Registers a profile-owned draw-list provider.
    ///
    /// Engine-runtime has no built-in draw-list defaults: the active profile owns
    /// terrain/mesh/UI extraction policy and must register it explicitly.
    #[inline]
    pub fn with_draw_list_provider(mut self, provider: Arc<dyn RenderDrawListProvider>) -> Self {
        self.features
            .draw_list_providers
            .register_provider(provider);
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
        self.features
            .light_extraction_providers
            .register_provider(provider);
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
    pub fn with_primary_lit_material_domain(mut self, key: MaterialGpuPipelineKey) -> Self {
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
        self.frame
            .input_systems
            .set_enabled(system, enabled, reason, self.frame.frame_index);
    }

    #[inline]
    pub fn input_systems_snapshot(&self) -> crate::input_systems::InputRuntimeSystemsSnapshot {
        self.frame.input_systems.snapshot(self.frame.frame_index)
    }

    #[inline]
    pub fn log_input_systems_snapshot(&self, reason: &str) {
        self.frame
            .input_systems
            .log_explicit_snapshot(self.frame.frame_index, reason);
    }

    pub(super) fn disable_viewport_pass(
        &mut self,
        phase: &'static str,
        error: impl std::fmt::Display,
    ) {
        let message = error.to_string();
        if !self.viewport.pass_disabled {
            newengine_ulog_api::ulog::error!(
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
            ui: RenderUiSurfaceRuntimeState::new(),
            runtime_profile: RenderRuntimeProfileState::new(),
            backend_failure: RenderBackendFailureState::new(),
            app_policy: RenderRuntimeAppPolicy::from_startup_config(),
        }
    }
}
