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
            exposure: 1.0,
            gamma: 2.2,
            black_lift: 0.0,
            operator: ToneMapOperator::AcesApprox,
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
