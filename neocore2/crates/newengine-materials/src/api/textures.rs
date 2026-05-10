use crate::api::MaterialId;

/// Minimal texture bindings layered on top of renderer-agnostic material descriptors.
///
/// The descriptor itself remains compact and copy-friendly, while texture paths live
/// beside it inside the registry / asset document layer.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialTextureBindings {
    pub base_color_texture: Option<String>,
    pub normal_texture: Option<String>,
    pub roughness_texture: Option<String>,
    pub uv_scale: [f32; 2],
    pub uv_offset: [f32; 2],
}


impl Default for MaterialTextureBindings {
    #[inline]
    fn default() -> Self {
        Self {
            base_color_texture: None,
            normal_texture: None,
            roughness_texture: None,
            uv_scale: [1.0, 1.0],
            uv_offset: [0.0, 0.0],
        }
    }
}

impl MaterialTextureBindings {
    #[inline]
    pub fn sanitized(mut self) -> Self {
        for v in &mut self.uv_scale {
            if !v.is_finite() || v.abs() < 1.0e-6 {
                *v = 1.0;
            }
        }
        for v in &mut self.uv_offset {
            if !v.is_finite() {
                *v = 0.0;
            }
        }
        fn sanitize_path(path: &mut Option<String>) {
            if let Some(value) = path {
                let trimmed = value.trim();
                *path = if trimmed.is_empty() { None } else { Some(trimmed.to_string()) };
            }
        }
        sanitize_path(&mut self.base_color_texture);
        sanitize_path(&mut self.normal_texture);
        sanitize_path(&mut self.roughness_texture);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct MaterialAssetDocument {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub desc: crate::api::MaterialDescriptor,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub textures: MaterialTextureBindings,
}

impl MaterialAssetDocument {
    #[inline]
    pub fn sanitized(mut self) -> Self {
        self.desc.sanitize_in_place();
        self.textures = self.textures.sanitized();
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaterialResolved {
    pub id: MaterialId,
    pub desc: crate::api::MaterialDescriptor,
    pub textures: MaterialTextureBindings,
}
