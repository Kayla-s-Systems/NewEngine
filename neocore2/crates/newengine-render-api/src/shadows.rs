use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShadowTechnique {
    None,
    DirectionalDepthMap,
    CascadedShadowMaps,
}

impl Default for ShadowTechnique {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ShadowQualityDesc {
    pub technique: ShadowTechnique,
    pub resolution: u32,
    pub cascade_count: u32,
    pub max_distance: f32,
    pub bias: f32,
    pub normal_bias: f32,
    pub softness: f32,
}

impl Default for ShadowQualityDesc {
    #[inline]
    fn default() -> Self {
        Self {
            technique: ShadowTechnique::DirectionalDepthMap,
            resolution: 2048,
            cascade_count: 1,
            max_distance: 96.0,
            bias: 0.0015,
            normal_bias: 0.02,
            softness: 0.35,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ShadowPassStats {
    pub enabled: bool,
    pub cascades: u32,
    pub shadow_draws: u32,
    pub shadow_map_resolution: u32,
    pub last_shadow_pass_ms: f32,
}
