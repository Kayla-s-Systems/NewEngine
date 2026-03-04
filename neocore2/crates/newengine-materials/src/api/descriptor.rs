use crate::api::{MaterialDomain, MaterialPermutationKey, ShadingModel};

/// Material feature flags.
///
/// Keep these renderer-agnostic; backends map them to pipeline state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct MaterialFlags(pub u32);

impl MaterialFlags {
    pub const NONE: Self = Self(0);

    pub const DOUBLE_SIDED: Self = Self(1 << 0);
    pub const ALPHA_BLEND: Self = Self(1 << 1);
    pub const ALPHA_TEST: Self = Self(1 << 2);
    pub const CAST_SHADOWS: Self = Self(1 << 3);

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[inline]
    pub const fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

/// Minimal, forward-compatible material descriptor.
///
/// Design goals:
/// - Renderer-agnostic (no backend-specific pipeline state).
/// - Compact, `Copy`, editor-friendly.
/// - Deterministic defaults.
/// - Extensible without leaking renderer details into gameplay/scene.
///
/// Notes:
/// - Texture binding is intentionally not part of the base contract yet.
///   It should be layered via asset-driven material instances once the
///   asset system provides stable texture handles/ids.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialDescriptor {
    /// High-level routing into render graph.
    pub domain: MaterialDomain,

    /// BRDF / shading family.
    pub shading_model: ShadingModel,

    pub base_color: [f32; 4],

    /// Emissive color (linear).
    ///
    /// The final emissive radiance is `emissive * emissive_strength`.
    pub emissive: [f32; 3],

    /// Emissive intensity multiplier (linear).
    ///
    /// Typical ranges:
    /// - 0.0: no emission
    /// - 1.0: subtle self-illumination
    /// - 5..50: strong emission suitable for bloom in HDR pipelines
    pub emissive_strength: f32,
    pub metallic: f32,
    pub roughness: f32,

    /// Normal map scale (1.0 = unchanged).
    pub normal_scale: f32,

    /// Ambient occlusion strength (1.0 = full effect).
    pub occlusion_strength: f32,

    /// Alpha test cutoff for masked materials (used when `ALPHA_TEST` is set).
    pub alpha_cutoff: f32,
    pub flags: MaterialFlags,

    /// Reserved for future expansions (must keep ABI/layout stable within the crate).
    pub reserved: [u32; 2],
}

impl MaterialDescriptor {
    /// Returns emissive radiance (linear HDR): `emissive * emissive_strength`.
    #[inline]
    pub fn emissive_radiance(&self) -> [f32; 3] {
        [
            self.emissive[0] * self.emissive_strength,
            self.emissive[1] * self.emissive_strength,
            self.emissive[2] * self.emissive_strength,
        ]
    }
    /// Sanitizes descriptor values for runtime/render backends.
    ///
    /// This clamps ranges and removes NaNs/Infs to avoid propagating invalid data into GPU code.
    #[inline]
    pub fn sanitize_in_place(&mut self) {
        #[inline]
        fn finite_or(v: f32, fallback: f32) -> f32 {
            if v.is_finite() { v } else { fallback }
        }

        #[inline]
        fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
            if v < lo { lo } else if v > hi { hi } else { v }
        }

        for c in &mut self.base_color {
            *c = finite_or(*c, 1.0);
        }
        self.base_color[0] = clamp(self.base_color[0], 0.0, 1.0);
        self.base_color[1] = clamp(self.base_color[1], 0.0, 1.0);
        self.base_color[2] = clamp(self.base_color[2], 0.0, 1.0);
        self.base_color[3] = clamp(self.base_color[3], 0.0, 1.0);

        for c in &mut self.emissive {
            *c = clamp(finite_or(*c, 0.0), 0.0, 1.0);
        }
        self.emissive_strength = clamp(finite_or(self.emissive_strength, 1.0), 0.0, 10_000.0);

        self.metallic = clamp(finite_or(self.metallic, 0.0), 0.0, 1.0);
        self.roughness = clamp(finite_or(self.roughness, 0.75), 0.02, 1.0);
        self.normal_scale = clamp(finite_or(self.normal_scale, 1.0), 0.0, 8.0);
        self.occlusion_strength = clamp(finite_or(self.occlusion_strength, 1.0), 0.0, 1.0);
        self.alpha_cutoff = clamp(finite_or(self.alpha_cutoff, 0.5), 0.0, 1.0);
    }

    /// Returns a sanitized copy of the descriptor.
    #[inline]
    pub fn sanitized(mut self) -> Self {
        self.sanitize_in_place();
        self
    }


    /// Create a deterministic permutation key used by render backends.
    ///
    /// Note: This is intentionally a pure function of descriptor fields.
    #[inline]
    pub fn permutation_key(&self) -> MaterialPermutationKey {
        // Layout (little-endian, deterministic across platforms):
        // [ domain:8 | shading:8 | _pad:16 | flags:32 ]
        let d = (self.domain as u64) & 0xFF;
        let s = (self.shading_model as u64) & 0xFF;
        let f = self.flags.0 as u64;

        let v = (d) | (s << 8) | (f << 32);
        // Keep 0 reserved.
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
