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
pub struct MaterialDescriptor {
    pub base_color: [f32; 4],

    /// Emissive radiance (linear). Kept separate from base_color for PBR-friendly workflows.
    pub emissive: [f32; 3],
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

impl Default for MaterialDescriptor {
    #[inline]
    fn default() -> Self {
        Self {
            base_color: [0.85, 0.85, 0.90, 1.0],
            emissive: [0.0, 0.0, 0.0],
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
