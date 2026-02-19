#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec3;

/// Global ambient lighting parameters.
///
/// This is a world resource (not a component).
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

/// Infinite directional light (e.g. sun).
///
/// Stored as a component on a dedicated entity.
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
/// Position is taken from the entity's `GlobalTransform` translation.
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
