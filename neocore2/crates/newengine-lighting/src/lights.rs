#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec3;

/// Global ambient lighting parameters.
///
/// Scene resource only. Render backends translate this into their own native
/// constant/light buffers during extraction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmbientLight {
    /// Linear RGB color.
    pub color: [f32; 3],
    /// Scalar intensity multiplier.
    pub intensity: f32,
}

impl Default for AmbientLight {
    #[inline]
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0],
            intensity: 0.08,
        }
    }
}

/// Infinite directional light such as a sun.
///
/// Scene component only. It carries physical-ish authoring parameters but no
/// renderer execution policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DirectionalLight {
    /// Direction of incoming light rays in world space.
    ///
    /// Convention: points *from the light towards the scene*.
    pub direction_ws: [f32; 3],
    /// Linear RGB.
    pub color: [f32; 3],
    pub intensity: f32,
}

impl Default for DirectionalLight {
    #[inline]
    fn default() -> Self {
        let d = Vec3::new(-0.35, -1.0, -0.25).normalize_or_zero();
        Self {
            direction_ws: [d.x, d.y, d.z],
            color: [1.0, 1.0, 1.0],
            intensity: 2.0,
        }
    }
}

/// Local point light.
///
/// Position is taken from the entity's `GlobalTransform` translation. Native
/// tiled/clustered list construction is renderer-owned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointLight {
    /// Linear RGB.
    pub color: [f32; 3],
    pub intensity: f32,
    /// Effective radius in world units.
    pub range: f32,
}

impl Default for PointLight {
    #[inline]
    fn default() -> Self {
        Self {
            color: [1.0, 0.95, 0.85],
            intensity: 10.0,
            range: 6.0,
        }
    }
}

/// Local spot light.
///
/// Position is taken from the entity's `GlobalTransform` translation. Direction
/// points from the light towards the scene in world space. This is data only;
/// shadow atlas allocation and tiled/clustered classification are renderer-owned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpotLight {
    /// Direction of emitted light in world space.
    pub direction_ws: [f32; 3],
    /// Linear RGB.
    pub color: [f32; 3],
    pub intensity: f32,
    /// Effective radius in world units.
    pub range: f32,
    /// Cone angle in radians.
    pub outer_angle_rad: f32,
    /// Inner cone angle in radians.
    pub inner_angle_rad: f32,
}

impl Default for SpotLight {
    #[inline]
    fn default() -> Self {
        let d = Vec3::new(0.0, -1.0, 0.0).normalize_or_zero();
        Self {
            direction_ws: [d.x, d.y, d.z],
            color: [1.0, 0.92, 0.78],
            intensity: 12.0,
            range: 9.0,
            outer_angle_rad: 0.78,
            inner_angle_rad: 0.52,
        }
    }
}
