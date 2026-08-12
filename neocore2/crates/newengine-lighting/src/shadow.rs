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

/// Visibility reconstruction applied after a shadow map has been produced.
///
/// Keeping filtering independent from `ShadowMethod` is intentional: CSM, spot
/// maps and future cached/local maps may all choose their own reconstruction
/// policy without changing the map-generation provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowFilter {
    Hard,
    Pcf,
    Pcss,
}

impl Default for ShadowFilter {
    #[inline]
    fn default() -> Self {
        Self::Pcf
    }
}

/// Directional-light PCSS controls expressed in stable physical/sample-space
/// units rather than frame-dependent normalized depth constants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowPcssSettings {
    /// Apparent half-angle of the emitter. The Sun is approximately 0.266 deg.
    pub light_angular_radius_degrees: f32,
    /// Radius of the blocker-search disk in shadow texels.
    pub blocker_search_radius_texels: f32,
    /// Maximum final PCF/PCSS penumbra radius in shadow texels.
    pub max_filter_radius_texels: f32,
    /// Number of Poisson samples used during blocker search (runtime clamps 4..16).
    pub blocker_samples: u32,
    /// Number of Poisson samples used during final filtering (runtime clamps 4..16).
    pub filter_samples: u32,
    /// Minimum radius of the final filter. Keeps contact edges antialiased without
    /// turning contact-hardening into a permanently blurred shadow.
    pub min_filter_radius_texels: f32,
    /// World-space kernel orientation is quantized in cells measured in shadow
    /// texels so camera motion cannot introduce temporal sampling jitter.
    pub stable_kernel_cell_texels: f32,
}

impl ShadowPcssSettings {
    #[inline]
    pub fn sanitized(mut self) -> Self {
        self.light_angular_radius_degrees =
            finite_or(self.light_angular_radius_degrees, 0.266).clamp(0.001, 5.0);
        self.blocker_search_radius_texels =
            finite_or(self.blocker_search_radius_texels, 5.0).clamp(0.5, 32.0);
        self.max_filter_radius_texels =
            finite_or(self.max_filter_radius_texels, 12.0).clamp(0.5, 64.0);
        self.blocker_samples = self.blocker_samples.clamp(4, 16);
        self.filter_samples = self.filter_samples.clamp(4, 16);
        self.min_filter_radius_texels = finite_or(self.min_filter_radius_texels, 0.55)
            .clamp(0.0, self.max_filter_radius_texels);
        self.stable_kernel_cell_texels =
            finite_or(self.stable_kernel_cell_texels, 4.0).clamp(1.0, 32.0);
        self
    }

    /// Tangent of the emitter half-angle. For a directional light the projected
    /// penumbra width is approximately `receiver_blocker_distance * tan(theta)`.
    #[inline]
    pub fn light_angular_radius_tangent(self) -> f32 {
        self.sanitized()
            .light_angular_radius_degrees
            .to_radians()
            .tan()
    }
}

impl Default for ShadowPcssSettings {
    #[inline]
    fn default() -> Self {
        Self {
            light_angular_radius_degrees: 0.266,
            blocker_search_radius_texels: 3.0,
            max_filter_radius_texels: 5.0,
            blocker_samples: 10,
            filter_samples: 12,
            min_filter_radius_texels: 0.18,
            stable_kernel_cell_texels: 8.0,
        }
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
    pub filter: ShadowFilter,
    pub resolution: u32,
    pub cascade_count: u32,
    pub max_distance: f32,
    /// Legacy/quality multiplier. For PCSS it scales the physical source radius;
    /// for PCF it remains the fixed filter-radius control.
    pub softness: f32,
    pub bias: f32,
    pub normal_bias: f32,
    pub contact_strength: f32,
    pub pcss: ShadowPcssSettings,
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
        self.pcss = self.pcss.sanitized();
        self
    }
}

impl Default for ShadowSettings {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: true,
            method: ShadowMethod::DirectionalDepthMap,
            filter: ShadowFilter::Pcf,
            resolution: 2048,
            cascade_count: 1,
            max_distance: 80.0,
            softness: 1.0,
            bias: 0.0025,
            normal_bias: 0.015,
            contact_strength: 0.25,
            pcss: ShadowPcssSettings::default(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_sun_radius_has_small_stable_tangent() {
        let pcss = ShadowPcssSettings::default();
        let tangent = pcss.light_angular_radius_tangent();
        assert!(tangent > 0.004 && tangent < 0.0055, "tangent={tangent}");
    }

    #[test]
    fn pcss_settings_sanitize_sample_and_radius_limits() {
        let pcss = ShadowPcssSettings {
            light_angular_radius_degrees: f32::NAN,
            blocker_search_radius_texels: 999.0,
            max_filter_radius_texels: -4.0,
            blocker_samples: 1,
            filter_samples: 100,
            min_filter_radius_texels: 9.0,
            stable_kernel_cell_texels: 0.0,
        }
        .sanitized();
        assert_eq!(pcss.blocker_samples, 4);
        assert_eq!(pcss.filter_samples, 16);
        assert!(pcss.blocker_search_radius_texels <= 32.0);
        assert!(pcss.min_filter_radius_texels <= pcss.max_filter_radius_texels);
        assert!(pcss.stable_kernel_cell_texels >= 1.0);
    }
}
