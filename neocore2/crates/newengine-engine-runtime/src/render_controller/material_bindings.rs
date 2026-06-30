#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::render::TextureId;
use newengine_materials::api::{MaterialFlags, MaterialResolved};

#[derive(Clone, Debug)]
pub(super) enum MaterialTextureGpuResidency {
    /// Path has been declared by scene/material extraction but no AssetManager
    /// request has been sent yet.
    Requested,
    /// AssetManager owns IO/import readiness. This state is deliberately
    /// non-blocking; the render loop polls it instead of calling wait_ready().
    AssetLoading {
        id_hex32: String,
        requested_frame: u64,
    },
    /// CPU-heavy texture decoding has been submitted to engine.threading and the
    /// render thread must keep presenting fallback material textures.
    CpuDecoding {
        requested_frame: u64,
    },
    /// CPU payload was decoded and a GPU upload has been enqueued.
    GpuLoading {
        texture: TextureId,
        requested_frame: u64,
    },
    Ready {
        texture: TextureId,
    },
    Failed {
        message: String,
    },
}

/// CPU-side material plan consumed by the lit render pass.
///
/// This keeps material semantics outside draw-loop plumbing and makes the
/// renderer consume a small, stable DTO instead of reaching into registry
/// internals at every call site.
#[derive(Clone, Copy, Debug)]
pub(super) struct LitMaterialPlan<'a> {
    pub base_color: [f32; 4],
    pub emissive_radiance: [f32; 3],
    pub uv_transform: [f32; 4],
    pub material_params: [f32; 4],
    pub base_color_texture: Option<&'a str>,
    pub normal_texture: Option<&'a str>,
    pub roughness_texture: Option<&'a str>,
    pub double_sided: bool,
    pub cast_shadows: bool,
    pub receive_shadows: bool,
}

impl<'a> LitMaterialPlan<'a> {
    #[inline]
    pub fn from_resolved(resolved: Option<&'a MaterialResolved>, fallback_color: [f32; 4]) -> Self {
        let Some(material) = resolved else {
            return Self::fallback(fallback_color);
        };

        Self {
            base_color: material.desc.base_color,
            emissive_radiance: material.desc.emissive_radiance(),
            uv_transform: [
                material.textures.uv_scale[0],
                material.textures.uv_scale[1],
                material.textures.uv_offset[0],
                material.textures.uv_offset[1],
            ],
            material_params: [
                material.desc.normal_scale,
                material.desc.roughness,
                material.desc.metallic,
                material.desc.occlusion_strength,
            ],
            base_color_texture: material.textures.base_color_texture.as_deref(),
            normal_texture: material.textures.normal_texture.as_deref(),
            roughness_texture: material.textures.roughness_texture.as_deref(),
            double_sided: material.desc.flags.contains(MaterialFlags::DOUBLE_SIDED),
            cast_shadows: material.desc.flags.contains(MaterialFlags::CAST_SHADOWS),
            receive_shadows: material.desc.flags.contains(MaterialFlags::RECEIVE_SHADOWS)
                || material.desc.flags.contains(MaterialFlags::CAST_SHADOWS),
        }
    }

    #[inline]
    pub fn has_textures(self) -> bool {
        self.base_color_texture.is_some()
            || self.normal_texture.is_some()
            || self.roughness_texture.is_some()
    }

    #[inline]
    fn fallback(base_color: [f32; 4]) -> Self {
        Self {
            base_color,
            emissive_radiance: [0.0, 0.0, 0.0],
            uv_transform: [1.0, 1.0, 0.0, 0.0],
            material_params: [1.0, 0.75, 0.0, 1.0],
            base_color_texture: None,
            normal_texture: None,
            roughness_texture: None,
            double_sided: false,
            cast_shadows: true,
            receive_shadows: true,
        }
    }
}
