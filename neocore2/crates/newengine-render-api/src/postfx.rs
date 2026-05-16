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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ToneMapDisplayParams {
    /// Linear exposure multiplier. 1.0 means authored lighting is used as-is.
    pub exposure: f32,
    /// Gamma/display transfer exponent. 2.2 is the non-HDR-monitor display baseline.
    pub gamma: f32,
    /// Optional black floor lift before display encoding. Keep at 0 for physically based output.
    pub black_lift: f32,
    pub operator: ToneMapOperator,
}

impl Default for ToneMapDisplayParams {
    #[inline]
    fn default() -> Self {
        Self {
            exposure: 1.12,
            gamma: 2.2,
            black_lift: 0.0,
            operator: ToneMapOperator::AcesApprox,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SunPostFxParams {
    /// Sun screen position in normalized viewport coordinates. [0,0] is lower-left.
    pub screen_position: [f32; 2],
    /// Linear RGB sun color from the active gameplay directional light.
    pub color: [f32; 3],
    /// Scalar intensity from the active gameplay directional light.
    pub intensity: f32,
    /// 0..1 visibility. The runtime computes this from sun direction and camera projection.
    pub visibility: f32,
    /// Normalized screen radius of the visible solar disk.
    pub disk_radius: f32,
    /// Screen-space flare strength. No ray tracing; pure post-process optics.
    pub flare_strength: f32,
    /// Screen-space god-ray/radial streak strength. No ray tracing.
    pub ray_strength: f32,
}

impl Default for SunPostFxParams {
    #[inline]
    fn default() -> Self {
        Self {
            screen_position: [0.5, 0.5],
            color: [1.0, 0.94, 0.82],
            intensity: 0.0,
            visibility: 0.0,
            disk_radius: 0.018,
            flare_strength: 0.18,
            ray_strength: 0.16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PostFxFrameParams {
    pub display: ToneMapDisplayParams,
    pub sun: SunPostFxParams,
}

impl Default for PostFxFrameParams {
    #[inline]
    fn default() -> Self {
        Self {
            display: ToneMapDisplayParams::default(),
            sun: SunPostFxParams::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostFxPassKind {
    Exposure,
    Bloom,
    ColorGrade,
    Tonemap,
    DisplayEncode,
    UiComposite,
    SunDisk,
    SunLensFlare,
    SunRays,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostFxPipelineDesc {
    pub label: String,
    pub hdr_scene_color: bool,
    pub display: ToneMapDisplayParams,
    pub passes: Vec<PostFxPassKind>,
}

impl Default for PostFxPipelineDesc {
    #[inline]
    fn default() -> Self {
        Self {
            label: "runtime.hdr_to_display".to_owned(),
            hdr_scene_color: true,
            display: ToneMapDisplayParams::default(),
            passes: vec![
                PostFxPassKind::Exposure,
                PostFxPassKind::Tonemap,
                PostFxPassKind::DisplayEncode,
                PostFxPassKind::SunDisk,
                PostFxPassKind::SunLensFlare,
                PostFxPassKind::SunRays,
                PostFxPassKind::UiComposite,
            ],
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PostFxPassStats {
    pub executed_passes: u32,
    pub last_postfx_ms: f32,
    pub hdr_scene_color: bool,
}
