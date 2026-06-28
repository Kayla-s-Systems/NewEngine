#![forbid(unsafe_op_in_unsafe_fn)]

/// Scene-declared shadow map strategy.
///
/// There is intentionally no `Auto` variant here: backend selection must be an
/// explicit render-provider capability decision, not hidden scene-data policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowMethod {
    None,
    /// Single orthographic/depth shadow map for a directional light.
    DirectionalDepthMap,
    /// Cascaded directional shadow atlas for large outdoor scenes.
    CascadedShadowMaps,
    /// Six-face cube shadow map for an omnidirectional point light.
    PointCubeMap,
    /// Single perspective depth map for a cone/spot light.
    SpotDepthMap,
}

impl Default for ShadowMethod {
    #[inline]
    fn default() -> Self {
        Self::DirectionalDepthMap
    }
}

/// Declarative scene/editor shadow settings.
///
/// Backends consume this resource to allocate render graph targets/passes. Scene
/// and editor code never own Vulkan/WGPU resources directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowSettings {
    pub enabled: bool,
    pub method: ShadowMethod,
    pub resolution: u32,
    pub cascade_count: u32,
    pub max_distance: f32,
    pub softness: f32,
    pub bias: f32,
    pub normal_bias: f32,
    pub contact_strength: f32,
}

impl ShadowSettings {
    #[inline]
    pub fn sanitized(mut self) -> Self {
        if !self.enabled || matches!(self.method, ShadowMethod::None) {
            self.enabled = false;
            self.method = ShadowMethod::None;
        }
        self.resolution = self.resolution.clamp(256, 8192);
        self.cascade_count = self.cascade_count.clamp(1, 4);
        self.max_distance = finite_or(self.max_distance, 80.0).clamp(4.0, 2048.0);
        self.softness = finite_or(self.softness, 1.0).clamp(0.0, 8.0);
        self.bias = finite_or(self.bias, 0.0025).clamp(0.0, 0.1);
        self.normal_bias = finite_or(self.normal_bias, 0.015).clamp(0.0, 0.5);
        self.contact_strength = finite_or(self.contact_strength, 0.25).clamp(0.0, 1.0);
        self
    }
}

impl Default for ShadowSettings {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: true,
            method: ShadowMethod::DirectionalDepthMap,
            resolution: 2048,
            cascade_count: 1,
            max_distance: 80.0,
            softness: 1.0,
            bias: 0.0025,
            normal_bias: 0.015,
            contact_strength: 0.25,
        }
    }
}

#[inline]
fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}
