use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PostFxPassKind {
    Exposure,
    BloomDownsample,
    BloomUpsample,
    ColorGrade,
    Tonemap,
    Sharpen,
    UiComposite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostFxPipelineDesc {
    pub enabled: bool,
    pub hdr_scene_color: bool,
    pub passes: Vec<PostFxPassKind>,
}

impl Default for PostFxPipelineDesc {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: true,
            hdr_scene_color: true,
            passes: vec![
                PostFxPassKind::Exposure,
                PostFxPassKind::Tonemap,
                PostFxPassKind::UiComposite,
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct PostFxPassStats {
    pub enabled_passes: u32,
    pub transient_targets: u32,
    pub last_postfx_ms: f32,
}
