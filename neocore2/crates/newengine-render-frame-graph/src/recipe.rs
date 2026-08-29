use serde::{Deserialize, Serialize};

use crate::StandardRenderPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeFrameFeatureSet {
    pub shadows: bool,
    pub local_shadows: bool,
    pub deferred: bool,
    pub postfx: bool,
    pub ui_composite: bool,
    pub ui_backdrop_blur: bool,
    pub debug_overlay: bool,
}

impl RuntimeFrameFeatureSet {
    #[inline]
    pub const fn forward(
        shadows: bool,
        postfx: bool,
        ui_composite: bool,
        debug_overlay: bool,
    ) -> Self {
        Self {
            shadows,
            local_shadows: false,
            deferred: false,
            postfx,
            ui_composite,
            ui_backdrop_blur: false,
            debug_overlay,
        }
    }

    #[inline]
    pub const fn deferred(
        shadows: bool,
        postfx: bool,
        ui_composite: bool,
        debug_overlay: bool,
    ) -> Self {
        Self {
            shadows,
            local_shadows: false,
            deferred: true,
            postfx,
            ui_composite,
            ui_backdrop_blur: false,
            debug_overlay,
        }
    }
    #[inline]
    pub const fn with_ui_backdrop_blur(mut self, enabled: bool) -> Self {
        self.ui_backdrop_blur = enabled;
        self
    }

    #[inline]
    pub const fn with_local_shadows(mut self, enabled: bool) -> Self {
        self.local_shadows = enabled;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderPhaseRecipeStep {
    pub phase: StandardRenderPhase,
    pub enabled: bool,
}

impl RenderPhaseRecipeStep {
    #[inline]
    pub const fn enabled(phase: StandardRenderPhase) -> Self {
        Self {
            phase,
            enabled: true,
        }
    }

    #[inline]
    pub const fn optional(phase: StandardRenderPhase, enabled: bool) -> Self {
        Self { phase, enabled }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderFrameRecipe {
    pub label: String,
    pub features: RuntimeFrameFeatureSet,
    #[serde(default)]
    pub steps: Vec<RenderPhaseRecipeStep>,
}

impl RenderFrameRecipe {
    pub fn standard_runtime(features: RuntimeFrameFeatureSet) -> Self {
        Self::standard_runtime_with_shadow_mode(features, false)
    }

    pub fn standard_runtime_with_shadow_mode(
        features: RuntimeFrameFeatureSet,
        cascaded_shadows: bool,
    ) -> Self {
        let mut steps = Vec::with_capacity(14);
        steps.push(RenderPhaseRecipeStep::enabled(
            StandardRenderPhase::BeginFrame,
        ));
        if cascaded_shadows {
            steps.push(RenderPhaseRecipeStep::optional(
                StandardRenderPhase::ShadowCascadeMap,
                features.shadows,
            ));
        } else {
            steps.push(RenderPhaseRecipeStep::optional(
                StandardRenderPhase::ShadowMap,
                features.shadows,
            ));
        }
        steps.push(RenderPhaseRecipeStep::optional(
            StandardRenderPhase::LocalShadowMap,
            features.local_shadows,
        ));
        // Particle simulation is compute work consumed by the transparent draw list.
        // Schedule it before the scene raster chain: Vulkan compute execution ends an
        // active legacy render pass, so placing it between ForwardOpaque and Transparent
        // would force a direct-surface continuation to reopen the swapchain CLEAR pass
        // and erase the opaque scene that was just rendered.
        steps.push(RenderPhaseRecipeStep::enabled(
            StandardRenderPhase::ParticleSimulation,
        ));
        if features.deferred {
            steps.push(RenderPhaseRecipeStep::enabled(
                StandardRenderPhase::DepthPrepass,
            ));
            steps.push(RenderPhaseRecipeStep::enabled(
                StandardRenderPhase::ViewportGBuffer,
            ));
            steps.push(RenderPhaseRecipeStep::enabled(
                StandardRenderPhase::DeferredLighting,
            ));
        } else {
            steps.push(RenderPhaseRecipeStep::enabled(
                StandardRenderPhase::ViewportForward,
            ));
        }
        steps.push(RenderPhaseRecipeStep::enabled(
            StandardRenderPhase::Transparent,
        ));
        steps.push(RenderPhaseRecipeStep::optional(
            StandardRenderPhase::PostFx,
            features.postfx,
        ));
        steps.push(RenderPhaseRecipeStep::optional(
            StandardRenderPhase::UiBackdropBlur,
            features.ui_composite && features.ui_backdrop_blur,
        ));
        steps.push(RenderPhaseRecipeStep::optional(
            StandardRenderPhase::UiComposite,
            features.ui_composite,
        ));
        steps.push(RenderPhaseRecipeStep::optional(
            StandardRenderPhase::DebugOverlay,
            features.debug_overlay,
        ));
        steps.push(RenderPhaseRecipeStep::enabled(
            StandardRenderPhase::EndFrame,
        ));

        Self {
            label: "runtime.standard_frame".to_owned(),
            features,
            steps,
        }
    }

    #[inline]
    pub fn enabled_phases(&self) -> impl Iterator<Item = StandardRenderPhase> + '_ {
        self.steps
            .iter()
            .filter(|step| step.enabled)
            .map(|step| step.phase)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeRecipeBuildParams {
    pub shadow_resolution: u32,
    pub shadow_cascade_count: u32,
}

impl RuntimeRecipeBuildParams {
    #[inline]
    pub const fn new(shadow_resolution: u32) -> Self {
        Self {
            shadow_resolution,
            shadow_cascade_count: 1,
        }
    }

    #[inline]
    pub const fn with_shadow_cascade_count(mut self, cascade_count: u32) -> Self {
        self.shadow_cascade_count = cascade_count;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_compute_precedes_forward_surface_raster_chain() {
        let recipe = RenderFrameRecipe::standard_runtime(RuntimeFrameFeatureSet::forward(
            true, false, false, false,
        ));
        let phases = recipe.enabled_phases().collect::<Vec<_>>();
        let particle = phases
            .iter()
            .position(|phase| *phase == StandardRenderPhase::ParticleSimulation)
            .expect("particle simulation phase");
        let forward = phases
            .iter()
            .position(|phase| *phase == StandardRenderPhase::ViewportForward)
            .expect("forward phase");
        let transparent = phases
            .iter()
            .position(|phase| *phase == StandardRenderPhase::Transparent)
            .expect("transparent phase");

        assert!(
            particle < forward,
            "compute must finish before surface raster begins"
        );
        assert!(
            forward < transparent,
            "transparent must continue the opaque surface scope"
        );
        assert_eq!(
            transparent,
            forward + 1,
            "no phase may split the direct-surface raster chain"
        );
    }
}
