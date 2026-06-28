use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToneMapOperator {
    /// Lightweight default for the first production HDR path. Stable, cheap and predictable.
    AcesApprox,
    Reinhard,
    None,
}

impl Default for ToneMapOperator {
    #[inline]
    fn default() -> Self {
        Self::AcesApprox
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AntiAliasingMode {
    None,
    /// Single-pass post-tonemap/HDR-compatible edge filter. This is the stable default until
    /// graph-level MSAA resolve resources are available on every provider.
    Fxaa,
    /// Temporal anti-aliasing. Requires provider-owned history color and velocity/depth inputs when available.
    Taa,
    /// Provider advertises the mode, but execution is valid only when the frame graph allocates
    /// multisampled scene/depth targets and explicit resolves.
    Msaa2x,
    Msaa4x,
    Msaa8x,
}

impl Default for AntiAliasingMode {
    #[inline]
    fn default() -> Self {
        Self::Fxaa
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ToneMapDisplayParams {
    /// Linear exposure multiplier. 1.0 means authored lighting is used as-is.
    #[serde(default = "default_exposure")]
    pub exposure: f32,
    /// Gamma/display transfer exponent. 2.2 is the non-HDR-monitor display baseline.
    #[serde(default = "default_gamma")]
    pub gamma: f32,
    /// Optional black floor lift before display encoding. Keep at 0 for physically based output.
    #[serde(default)]
    pub black_lift: f32,
    #[serde(default)]
    pub operator: ToneMapOperator,
}

impl Default for ToneMapDisplayParams {
    #[inline]
    fn default() -> Self {
        Self {
            exposure: default_exposure(),
            gamma: default_gamma(),
            black_lift: 0.0,
            operator: ToneMapOperator::AcesApprox,
        }
    }
}

#[inline]
fn default_exposure() -> f32 {
    1.12
}
#[inline]
fn default_gamma() -> f32 {
    2.2
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BloomParams {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// HDR threshold where bloom starts extracting highlights.
    #[serde(default = "default_bloom_threshold")]
    pub threshold: f32,
    /// Soft knee width around the threshold. Prevents harsh popping around bright edges.
    #[serde(default = "default_bloom_knee")]
    pub knee: f32,
    /// Final bloom contribution in linear HDR before tone mapping.
    #[serde(default = "default_bloom_intensity")]
    pub intensity: f32,
    /// Sampling radius multiplier. 1.0 keeps the fast production kernel.
    #[serde(default = "default_bloom_radius")]
    pub radius: f32,
}

impl Default for BloomParams {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: default_bloom_threshold(),
            knee: default_bloom_knee(),
            intensity: default_bloom_intensity(),
            radius: default_bloom_radius(),
        }
    }
}

#[inline]
fn default_bloom_threshold() -> f32 {
    0.85
}
#[inline]
fn default_bloom_knee() -> f32 {
    0.35
}
#[inline]
fn default_bloom_intensity() -> f32 {
    0.085
}
#[inline]
fn default_bloom_radius() -> f32 {
    1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FxaaParams {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Standard FXAA edge threshold. Lower values catch more edges and cost the same.
    #[serde(default = "default_fxaa_edge_threshold")]
    pub edge_threshold: f32,
    /// Minimum luma range before filtering begins.
    #[serde(default = "default_fxaa_edge_threshold_min")]
    pub edge_threshold_min: f32,
    /// Sub-pixel blending strength. 0 disables softening, 1 uses full local blend.
    #[serde(default = "default_fxaa_subpixel_quality")]
    pub subpixel_quality: f32,
}

impl Default for FxaaParams {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: true,
            edge_threshold: default_fxaa_edge_threshold(),
            edge_threshold_min: default_fxaa_edge_threshold_min(),
            subpixel_quality: default_fxaa_subpixel_quality(),
        }
    }
}

#[inline]
fn default_fxaa_edge_threshold() -> f32 {
    0.125
}
#[inline]
fn default_fxaa_edge_threshold_min() -> f32 {
    0.0312
}
#[inline]
fn default_fxaa_subpixel_quality() -> f32 {
    0.75
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TaaParams {
    #[serde(default)]
    pub enabled: bool,
    /// History blend factor. Higher values are smoother but can ghost more.
    #[serde(default = "default_taa_feedback")]
    pub feedback: f32,
    /// Neighborhood clamp strength for anti-ghosting.
    #[serde(default = "default_taa_neighborhood_clamping")]
    pub neighborhood_clamping: f32,
    /// Jitter scale applied to view jitter from the camera/view gateway.
    #[serde(default = "default_taa_jitter_scale")]
    pub jitter_scale: f32,
}

impl Default for TaaParams {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: false,
            feedback: default_taa_feedback(),
            neighborhood_clamping: default_taa_neighborhood_clamping(),
            jitter_scale: default_taa_jitter_scale(),
        }
    }
}

#[inline]
fn default_taa_feedback() -> f32 {
    0.92
}
#[inline]
fn default_taa_neighborhood_clamping() -> f32 {
    1.0
}
#[inline]
fn default_taa_jitter_scale() -> f32 {
    1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColorGradeParams {
    #[serde(default = "default_saturation")]
    pub saturation: f32,
    #[serde(default = "default_contrast")]
    pub contrast: f32,
    /// Warm/cool offset. Positive warms highlights slightly, negative cools them.
    #[serde(default)]
    pub temperature: f32,
    #[serde(default = "default_vignette_strength")]
    pub vignette_strength: f32,
    #[serde(default = "default_local_contrast_strength")]
    pub local_contrast_strength: f32,
    #[serde(default = "default_dither_strength")]
    pub dither_strength: f32,
}

impl Default for ColorGradeParams {
    #[inline]
    fn default() -> Self {
        Self {
            saturation: default_saturation(),
            contrast: default_contrast(),
            temperature: 0.0,
            vignette_strength: default_vignette_strength(),
            local_contrast_strength: default_local_contrast_strength(),
            dither_strength: default_dither_strength(),
        }
    }
}

#[inline]
fn default_saturation() -> f32 {
    1.06
}
#[inline]
fn default_contrast() -> f32 {
    1.03
}
#[inline]
fn default_vignette_strength() -> f32 {
    0.10
}
#[inline]
fn default_local_contrast_strength() -> f32 {
    0.055
}
#[inline]
fn default_dither_strength() -> f32 {
    1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SsaoParams {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ssao_radius_ws")]
    pub radius_ws: f32,
    #[serde(default = "default_ssao_intensity")]
    pub intensity: f32,
    #[serde(default = "default_ssao_quality_steps")]
    pub quality_steps: u32,
    #[serde(default = "default_true")]
    pub half_resolution: bool,
}

impl Default for SsaoParams {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: false,
            radius_ws: default_ssao_radius_ws(),
            intensity: default_ssao_intensity(),
            quality_steps: default_ssao_quality_steps(),
            half_resolution: true,
        }
    }
}

#[inline]
fn default_ssao_radius_ws() -> f32 {
    0.75
}
#[inline]
fn default_ssao_intensity() -> f32 {
    0.82
}
#[inline]
fn default_ssao_quality_steps() -> u32 {
    16
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PostFxQualityParams {
    #[serde(default)]
    pub bloom: BloomParams,
    #[serde(default)]
    pub fxaa: FxaaParams,
    #[serde(default)]
    pub taa: TaaParams,
    #[serde(default)]
    pub ssao: SsaoParams,
    #[serde(default)]
    pub color: ColorGradeParams,
    #[serde(default)]
    pub anti_aliasing: AntiAliasingMode,
}

impl Default for PostFxQualityParams {
    #[inline]
    fn default() -> Self {
        Self {
            bloom: BloomParams::default(),
            fxaa: FxaaParams::default(),
            taa: TaaParams::default(),
            ssao: SsaoParams::default(),
            color: ColorGradeParams::default(),
            anti_aliasing: AntiAliasingMode::Fxaa,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SunPostFxParams {
    /// Sun screen position in normalized viewport coordinates. [0,0] is lower-left.
    #[serde(default = "default_sun_screen_position")]
    pub screen_position: [f32; 2],
    /// Linear RGB sun color from the active gameplay directional light.
    #[serde(default = "default_sun_color")]
    pub color: [f32; 3],
    /// World-space light direction used by lighting/deferred passes. Must be normalized by the provider.
    #[serde(default = "default_sun_direction")]
    pub direction: [f32; 3],
    /// Scalar intensity from the active gameplay directional light.
    #[serde(default)]
    pub intensity: f32,
    /// 0..1 visibility. The runtime computes this from sun direction and the active view projection.
    #[serde(default)]
    pub visibility: f32,
    /// Normalized screen radius of the visible solar disk.
    #[serde(default = "default_sun_disk_radius")]
    pub disk_radius: f32,
    /// Screen-space flare strength. No ray tracing; pure post-process optics.
    #[serde(default = "default_sun_flare_strength")]
    pub flare_strength: f32,
    /// Screen-space god-ray/radial streak strength. No ray tracing.
    #[serde(default = "default_sun_ray_strength")]
    pub ray_strength: f32,
}

impl Default for SunPostFxParams {
    #[inline]
    fn default() -> Self {
        Self {
            screen_position: default_sun_screen_position(),
            color: default_sun_color(),
            direction: default_sun_direction(),
            intensity: 0.0,
            visibility: 0.0,
            disk_radius: default_sun_disk_radius(),
            flare_strength: default_sun_flare_strength(),
            ray_strength: default_sun_ray_strength(),
        }
    }
}

#[inline]
fn default_true() -> bool {
    true
}
#[inline]
fn default_sun_screen_position() -> [f32; 2] {
    [0.5, 0.5]
}
#[inline]
fn default_sun_color() -> [f32; 3] {
    [1.0, 0.94, 0.82]
}
#[inline]
fn default_sun_direction() -> [f32; 3] {
    [-0.53590363, -0.7989835, -0.27282366]
}
#[inline]
fn default_sun_disk_radius() -> f32 {
    0.018
}
#[inline]
fn default_sun_flare_strength() -> f32 {
    0.18
}
#[inline]
fn default_sun_ray_strength() -> f32 {
    0.16
}

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
fn default_dof_far_plane() -> f32 {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostFxPassKind {
    Exposure,
    ExposureAdaptation,
    Ssao,
    DepthReduction,
    AdaptiveDof,
    LensArtefacts,
    PostScan,
    Bloom,
    ColorGrade,
    Tonemap,
    Fxaa,
    TaaResolve,
    MsaaResolve,
    DisplayEncode,
    UiBackdropBlur,
    UiComposite,
    SunDisk,
    SunLensFlare,
    SunRays,
    Dither,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostFxPipelineDesc {
    pub label: String,
    pub hdr_scene_color: bool,
    #[serde(default)]
    pub display: ToneMapDisplayParams,
    #[serde(default)]
    pub quality: PostFxQualityParams,
    #[serde(default = "default_postfx_passes")]
    pub passes: Vec<PostFxPassKind>,
}

impl Default for PostFxPipelineDesc {
    #[inline]
    fn default() -> Self {
        Self {
            label: "runtime.hdr_to_display".to_owned(),
            hdr_scene_color: true,
            display: ToneMapDisplayParams::default(),
            quality: PostFxQualityParams::default(),
            passes: default_postfx_passes(),
        }
    }
}

#[inline]
fn default_postfx_passes() -> Vec<PostFxPassKind> {
    vec![
        PostFxPassKind::Exposure,
        PostFxPassKind::ExposureAdaptation,
        PostFxPassKind::Ssao,
        PostFxPassKind::DepthReduction,
        PostFxPassKind::AdaptiveDof,
        PostFxPassKind::LensArtefacts,
        PostFxPassKind::PostScan,
        PostFxPassKind::Bloom,
        PostFxPassKind::SunDisk,
        PostFxPassKind::SunLensFlare,
        PostFxPassKind::SunRays,
        PostFxPassKind::ColorGrade,
        PostFxPassKind::Tonemap,
        PostFxPassKind::Fxaa,
        PostFxPassKind::TaaResolve,
        PostFxPassKind::MsaaResolve,
        PostFxPassKind::DisplayEncode,
        PostFxPassKind::Dither,
        PostFxPassKind::UiBackdropBlur,
        PostFxPassKind::UiComposite,
    ]
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PostFxPassStats {
    pub executed_passes: u32,
    pub last_postfx_ms: f32,
    pub hdr_scene_color: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postfx_frame_params_accept_old_payload_without_quality() {
        let json = r#"{
            "display":{"exposure":1.0,"gamma":2.2,"black_lift":0.0,"operator":"AcesApprox"},
            "sun":{"screen_position":[0.5,0.5],"color":[1.0,0.94,0.82],"intensity":3.2,"visibility":1.0,"disk_radius":0.018,"flare_strength":0.2,"ray_strength":0.16}
        }"#;
        let decoded: PostFxFrameParams =
            serde_json::from_str(json).expect("old postfx payload must remain valid");
        assert!(decoded.quality.bloom.enabled);
        assert!(decoded.quality.fxaa.enabled);
        assert_eq!(decoded.quality.anti_aliasing, AntiAliasingMode::Fxaa);
        assert!(!decoded.quality.ssao.enabled);
    }

    #[test]
    fn postfx_frame_params_accept_old_payload_without_view_intent() {
        let json = r#"{
            "display":{"exposure":1.0,"gamma":2.2,"black_lift":0.0,"operator":"AcesApprox"},
            "sun":{"screen_position":[0.5,0.5],"color":[1.0,0.94,0.82],"intensity":3.2,"visibility":1.0,"disk_radius":0.018,"flare_strength":0.2,"ray_strength":0.16},
            "quality":{"anti_aliasing":"Fxaa"}
        }"#;
        let decoded: PostFxFrameParams =
            serde_json::from_str(json).expect("old postfx payload must remain valid");
        assert_eq!(decoded.view.motion_blur.strength, 0.0);
        assert_eq!(decoded.view.dof.far_end, default_dof_far_plane());
    }

    #[test]
    fn postfx_pipeline_defaults_include_aaa_pass_order() {
        let desc = PostFxPipelineDesc::default();
        assert!(desc.passes.contains(&PostFxPassKind::ExposureAdaptation));
        assert!(desc.passes.contains(&PostFxPassKind::Ssao));
        assert!(desc.passes.contains(&PostFxPassKind::AdaptiveDof));
        assert!(desc.passes.contains(&PostFxPassKind::LensArtefacts));
        assert!(desc.passes.contains(&PostFxPassKind::PostScan));
        assert!(desc.passes.contains(&PostFxPassKind::Bloom));
        assert!(desc.passes.contains(&PostFxPassKind::Fxaa));
        assert!(desc.passes.contains(&PostFxPassKind::TaaResolve));
        assert!(desc.passes.contains(&PostFxPassKind::MsaaResolve));
        assert!(desc.passes.contains(&PostFxPassKind::Dither));
    }
}
