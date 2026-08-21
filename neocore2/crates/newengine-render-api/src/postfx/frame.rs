use serde::{Deserialize, Serialize};
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewDepthOfFieldFrameParams {
    #[serde(default)]
    pub near_start: f32,
    #[serde(default)]
    pub near_end: f32,
    #[serde(default = "default_dof_far_plane")]
    pub far_start: f32,
    #[serde(default = "default_dof_far_plane")]
    pub far_end: f32,
    #[serde(default)]
    pub blend_level: f32,
    #[serde(default)]
    pub high_quality: bool,
}

impl Default for ViewDepthOfFieldFrameParams {
    #[inline]
    fn default() -> Self {
        Self {
            near_start: 0.0,
            near_end: 0.0,
            far_start: default_dof_far_plane(),
            far_end: default_dof_far_plane(),
            blend_level: 0.0,
            high_quality: false,
        }
    }
}

#[inline]
pub(super) fn default_dof_far_plane() -> f32 {
    10_000.0
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewMotionBlurFrameParams {
    #[serde(default)]
    pub strength: f32,
    #[serde(default = "default_motion_blur_decay_rate")]
    pub decay_rate: f32,
}

impl Default for ViewMotionBlurFrameParams {
    #[inline]
    fn default() -> Self {
        Self {
            strength: 0.0,
            decay_rate: default_motion_blur_decay_rate(),
        }
    }
}

#[inline]
fn default_motion_blur_decay_rate() -> f32 {
    0.5
}

/// Renderer-facing, source-agnostic frame post-process intent.
///
/// This is deliberately not tied to any view producer implementation. Cutscene, replay,
/// editor, gameplay or photo-mode systems can provide the same normalized
/// frame intent without coupling render API to producer-specific state.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewPostFxFrameParams {
    #[serde(default)]
    pub dof: ViewDepthOfFieldFrameParams,
    #[serde(default)]
    pub motion_blur: ViewMotionBlurFrameParams,
    #[serde(default)]
    pub shake_amplitude: f32,
    #[serde(default)]
    pub exposure_bias: f32,
    #[serde(default)]
    pub jitter_px: [f32; 2],
}

impl Default for ViewPostFxFrameParams {
    #[inline]
    fn default() -> Self {
        Self {
            dof: ViewDepthOfFieldFrameParams::default(),
            motion_blur: ViewMotionBlurFrameParams::default(),
            shake_amplitude: 0.0,
            exposure_bias: 0.0,
            jitter_px: [0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UiBackdropPostFxParams {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub alpha: f32,
    #[serde(default)]
    pub dim_opacity: f32,
    #[serde(default)]
    pub blur_radius_px: f32,
}

impl Default for UiBackdropPostFxParams {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: false,
            alpha: 0.0,
            dim_opacity: 0.0,
            blur_radius_px: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PostFxFrameParams {
    #[serde(default)]
    pub display: ToneMapDisplayParams,
    #[serde(default)]
    pub sun: SunPostFxParams,
    #[serde(default)]
    pub quality: PostFxQualityParams,
    #[serde(default)]
    pub view: ViewPostFxFrameParams,
    #[serde(default)]
    pub ui_backdrop: UiBackdropPostFxParams,
}

impl Default for PostFxFrameParams {
    #[inline]
    fn default() -> Self {
        Self {
            display: ToneMapDisplayParams::default(),
            sun: SunPostFxParams::default(),
            quality: PostFxQualityParams::default(),
            view: ViewPostFxFrameParams::default(),
            ui_backdrop: UiBackdropPostFxParams::default(),
        }
    }
}

