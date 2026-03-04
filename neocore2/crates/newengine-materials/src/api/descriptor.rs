use crate::api::{MaterialDomain, MaterialPermutationKey, ShadingModel};

/// Material feature flags.
///
/// Keep these renderer-agnostic; backends map them to pipeline state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct MaterialFlags(pub u32);

impl MaterialFlags {
    /// No special material features enabled.
    pub const NONE: Self = Self(0);

    /// Render both triangle winding orders.
    pub const DOUBLE_SIDED: Self = Self(1 << 0);
    /// Enable alpha blending.
    pub const ALPHA_BLEND: Self = Self(1 << 1);
    /// Enable alpha testing / masking.
    pub const ALPHA_TEST: Self = Self(1 << 2);
    /// Allow the material to cast shadows.
    pub const CAST_SHADOWS: Self = Self(1 << 3);

    /// Returns `true` when all bits from `other` are present in `self`.
    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns the union of two flag sets.
    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns the intersection of two flag sets.
    #[inline]
    pub const fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

/// Minimal, forward-compatible material descriptor.
///
/// Design goals:
/// - renderer-agnostic (no backend-specific pipeline state);
/// - compact, `Copy`, editor-friendly;
/// - deterministic defaults;
/// - extensible without leaking renderer details into gameplay or scene code.
///
/// Notes:
/// - texture binding is intentionally not part of the base contract yet;
/// - texture references should be layered via asset-driven material instances once the
///   asset system provides stable texture handles or ids.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialDescriptor {
    /// High-level routing into the render graph.
    pub domain: MaterialDomain,

    /// BRDF / shading family.
    pub shading_model: ShadingModel,

    /// Base albedo in linear space.
    pub base_color: [f32; 4],

    /// Emissive color in linear space.
    ///
    /// The final emissive radiance is `emissive * emissive_strength`.
    pub emissive: [f32; 3],

    /// Emissive intensity multiplier in linear space.
    ///
    /// Typical ranges:
    /// - `0.0`: no emission
    /// - `1.0`: subtle self-illumination
    /// - `5.0..50.0`: strong emission suitable for bloom in HDR pipelines
    pub emissive_strength: f32,

    /// PBR metallic factor.
    pub metallic: f32,
    /// PBR roughness factor.
    pub roughness: f32,

    /// Normal map scale (`1.0` = unchanged).
    pub normal_scale: f32,

    /// Ambient occlusion strength (`1.0` = full effect).
    pub occlusion_strength: f32,

    /// Alpha test cutoff for masked materials.
    ///
    /// Used when [`MaterialFlags::ALPHA_TEST`] is set.
    pub alpha_cutoff: f32,

    /// Renderer-agnostic material feature flags.
    pub flags: MaterialFlags,

    /// Reserved for future extensions.
    pub reserved: [u32; 2],
}

impl MaterialDescriptor {
    /// Returns emissive radiance in linear HDR space: `emissive * emissive_strength`.
    #[inline]
    pub fn emissive_radiance(&self) -> [f32; 3] {
        [
            self.emissive[0] * self.emissive_strength,
            self.emissive[1] * self.emissive_strength,
            self.emissive[2] * self.emissive_strength,
        ]
    }

    /// Sanitizes descriptor values for runtime and render backends.
    ///
    /// This clamps numeric ranges and replaces NaN or infinite values with deterministic
    /// fallbacks, preventing invalid data from leaking into GPU code or caches.
    #[inline]
    pub fn sanitize_in_place(&mut self) {
        sanitize_slice_clamped(&mut self.base_color, 1.0, 0.0, 1.0);
        sanitize_slice_clamped(&mut self.emissive, 0.0, 0.0, 1.0);

        self.emissive_strength = sanitize_scalar(self.emissive_strength, 1.0, 0.0, 10_000.0);
        self.metallic = sanitize_scalar(self.metallic, 0.0, 0.0, 1.0);
        self.roughness = sanitize_scalar(self.roughness, 0.75, 0.02, 1.0);
        self.normal_scale = sanitize_scalar(self.normal_scale, 1.0, 0.0, 8.0);
        self.occlusion_strength = sanitize_scalar(self.occlusion_strength, 1.0, 0.0, 1.0);
        self.alpha_cutoff = sanitize_scalar(self.alpha_cutoff, 0.5, 0.0, 1.0);
    }

    /// Returns a sanitized copy of the descriptor.
    #[inline]
    pub fn sanitized(mut self) -> Self {
        self.sanitize_in_place();
        self
    }

    /// Creates a deterministic permutation key used by render backends.
    ///
    /// This key is intentionally a pure function of descriptor routing and feature fields.
    #[inline]
    pub fn permutation_key(&self) -> MaterialPermutationKey {
        // Layout (little-endian, deterministic across platforms):
        // [ domain:8 | shading:8 | _pad:16 | flags:32 ]
        let d = (self.domain as u64) & 0xFF;
        let s = (self.shading_model as u64) & 0xFF;
        let f = self.flags.0 as u64;

        let v = d | (s << 8) | (f << 32);
        MaterialPermutationKey(if v == 0 { 1 } else { v })
    }
}

impl Default for MaterialDescriptor {
    #[inline]
    fn default() -> Self {
        Self {
            domain: MaterialDomain::Surface,
            shading_model: ShadingModel::PbrMetallicRoughness,
            base_color: [0.85, 0.85, 0.90, 1.0],
            emissive: [0.0, 0.0, 0.0],
            emissive_strength: 1.0,
            metallic: 0.0,
            roughness: 0.75,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            alpha_cutoff: 0.5,
            flags: MaterialFlags::NONE,
            reserved: [0; 2],
        }
    }
}

#[inline]
fn sanitize_scalar(v: f32, fallback: f32, lo: f32, hi: f32) -> f32 {
    clamp(finite_or(v, fallback), lo, hi)
}

#[inline]
fn sanitize_slice_clamped<const N: usize>(slice: &mut [f32; N], fallback: f32, lo: f32, hi: f32) {
    for value in slice {
        *value = sanitize_scalar(*value, fallback, lo, hi);
    }
}

#[inline]
fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() { v } else { fallback }
}

#[inline]
fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}
