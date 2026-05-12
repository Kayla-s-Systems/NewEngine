use serde::{Deserialize, Serialize};

use crate::StandardRenderPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeFrameFeatureSet {
    pub shadows: bool,
    pub deferred: bool,
    pub postfx: bool,
    pub ui_composite: bool,
    pub debug_overlay: bool,
}

impl RuntimeFrameFeatureSet {
    #[inline]
    pub const fn forward(shadows: bool, postfx: bool, ui_composite: bool, debug_overlay: bool) -> Self {
        Self {
            shadows,
            deferred: false,
            postfx,
            ui_composite,
            debug_overlay,
        }
    }

    #[inline]
    pub const fn deferred(shadows: bool, postfx: bool, ui_composite: bool, debug_overlay: bool) -> Self {
        Self {
            shadows,
            deferred: true,
            postfx,
            ui_composite,
            debug_overlay,
        }
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
        Self { phase, enabled: true }
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
        let mut steps = Vec::with_capacity(12);
        steps.push(RenderPhaseRecipeStep::enabled(StandardRenderPhase::BeginFrame));
        steps.push(RenderPhaseRecipeStep::optional(StandardRenderPhase::ShadowMap, features.shadows));
        if features.deferred {
            steps.push(RenderPhaseRecipeStep::enabled(StandardRenderPhase::DepthPrepass));
            steps.push(RenderPhaseRecipeStep::enabled(StandardRenderPhase::ViewportGBuffer));
            steps.push(RenderPhaseRecipeStep::enabled(StandardRenderPhase::DeferredLighting));
        } else {
            steps.push(RenderPhaseRecipeStep::enabled(StandardRenderPhase::ViewportForward));
        }
        steps.push(RenderPhaseRecipeStep::optional(StandardRenderPhase::PostFx, features.postfx));
        steps.push(RenderPhaseRecipeStep::optional(StandardRenderPhase::UiComposite, features.ui_composite));
        steps.push(RenderPhaseRecipeStep::optional(StandardRenderPhase::DebugOverlay, features.debug_overlay));
        steps.push(RenderPhaseRecipeStep::enabled(StandardRenderPhase::EndFrame));

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
}

impl RuntimeRecipeBuildParams {
    #[inline]
    pub const fn new(shadow_resolution: u32) -> Self {
        Self { shadow_resolution }
    }
}
