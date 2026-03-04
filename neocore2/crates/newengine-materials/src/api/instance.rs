use crate::api::{MaterialDescriptor, MaterialDomain, MaterialFlags, MaterialId, ShadingModel};

/// Per-instance overrides applied on top of a base material descriptor.
///
/// This is intentionally field-by-field, so the engine can evolve the material
/// descriptor without forcing instance users to re-specify the full structure.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MaterialOverrides {
    pub domain: Option<MaterialDomain>,
    pub shading_model: Option<ShadingModel>,

    pub base_color: Option<[f32; 4]>,
    pub emissive: Option<[f32; 3]>,
    pub emissive_strength: Option<f32>,
    pub metallic: Option<f32>,
    pub roughness: Option<f32>,
    pub normal_scale: Option<f32>,
    pub occlusion_strength: Option<f32>,
    pub alpha_cutoff: Option<f32>,
    pub flags: Option<MaterialFlags>,
}

impl MaterialOverrides {
    /// Apply overrides to a base descriptor.
    #[inline]
    pub fn apply_to(&self, mut base: MaterialDescriptor) -> MaterialDescriptor {
        if let Some(v) = self.domain {
            base.domain = v;
        }
        if let Some(v) = self.shading_model {
            base.shading_model = v;
        }
        if let Some(v) = self.base_color {
            base.base_color = v;
        }
        if let Some(v) = self.emissive {
            base.emissive = v;
        }
        if let Some(v) = self.emissive_strength {
            base.emissive_strength = v;
        }
        if let Some(v) = self.metallic {
            base.metallic = v;
        }
        if let Some(v) = self.roughness {
            base.roughness = v;
        }
        if let Some(v) = self.normal_scale {
            base.normal_scale = v;
        }
        if let Some(v) = self.occlusion_strength {
            base.occlusion_strength = v;
        }
        if let Some(v) = self.alpha_cutoff {
            base.alpha_cutoff = v;
        }
        if let Some(v) = self.flags {
            base.flags = v;
        }
        base
    }
}

/// Runtime material instance (base asset + overrides).
///
/// Instances are identified by a `MaterialId` with a dedicated high bit set.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MaterialInstanceDesc {
    pub base: MaterialId,
    pub overrides: MaterialOverrides,
}
