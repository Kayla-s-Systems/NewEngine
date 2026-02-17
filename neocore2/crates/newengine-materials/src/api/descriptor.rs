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
/// This starts small and editor-friendly while leaving room for growth.
/// Texture binding is intentionally not part of the base contract yet.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MaterialDescriptor {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub flags: MaterialFlags,

    /// Reserved for future expansions (must keep ABI/layout stable within the crate).
    pub reserved: [u32; 4],
}

impl Default for MaterialDescriptor {
    #[inline]
    fn default() -> Self {
        Self {
            base_color: [0.85, 0.85, 0.90, 1.0],
            metallic: 0.0,
            roughness: 0.75,
            flags: MaterialFlags::NONE,
            reserved: [0; 4],
        }
    }
}
