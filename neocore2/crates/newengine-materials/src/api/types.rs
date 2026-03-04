/// Material domain defines which render pipeline stage the material belongs to.
///
/// Domains are renderer-agnostic, but they allow the renderer to route materials
/// into the correct render graph passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum MaterialDomain {
    /// Standard 3D surface material.
    Surface = 0,
    /// Screen-space material (post-processing / full-screen passes).
    PostProcess = 1,
    /// UI / 2D overlay material.
    Ui = 2,
}

impl Default for MaterialDomain {
    #[inline]
    fn default() -> Self {
        Self::Surface
    }
}

/// Shading model describes the lighting / BRDF family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum ShadingModel {
    /// Unlit shading (vertex/pixel color only).
    Unlit = 0,
    /// PBR metallic-roughness model.
    PbrMetallicRoughness = 1,
}

impl Default for ShadingModel {
    #[inline]
    fn default() -> Self {
        Self::PbrMetallicRoughness
    }
}

/// Deterministic material permutation key.
///
/// Render backends can use this key to cache pipelines and layout variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct MaterialPermutationKey(pub u64);

impl MaterialPermutationKey {
    /// Returns the sentinel invalid key.
    #[inline]
    pub const fn invalid() -> Self {
        Self(0)
    }

    /// Returns `true` when the key is non-zero.
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}
