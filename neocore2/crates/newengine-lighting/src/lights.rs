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

/// Renderer-agnostic shadow configuration resource.
///
/// This is intentionally declarative: scene/editor/gameplay code can configure
/// shadow intent without depending on Vulkan/WGPU-specific render targets,
/// samplers, pipelines, or cascade allocations. Backends consume this resource
/// when their shadow pass is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowMethod {
    None,
    DirectionalDepthMap,
}

impl Default for ShadowMethod {
    #[inline]
    fn default() -> Self { Self::DirectionalDepthMap }
}

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
