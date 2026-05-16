#![forbid(unsafe_op_in_unsafe_fn)]

//! Host-side render material-domain provider contract.
//!
//! The goal of this crate is to keep reusable runtime orchestration independent
//! from GameReady/FPS shader packages and material presets. A profile or feature
//! package registers a [`MaterialGpuPipelineProvider`]; the render controller
//! asks for a domain key and receives the engine-side bundle needed by its pass
//! code. Backend-native resources are still created only through `RenderApi`.

use newengine_core::render::{
    BindGroupLayoutId, PipelineId, SamplerId, ShaderId, TextureFormat, TextureId,
};
use newengine_core::{render::RenderApi, EngineResult};

/// Stable render-material domain key.
///
/// The string is a feature/domain contract id, not a renderer-native pipeline id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MaterialGpuPipelineKey(pub &'static str);

impl MaterialGpuPipelineKey {
    #[inline]
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    #[inline]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Build profile supplied by reusable runtime orchestration.
///
/// Material-domain providers own shader packages and pipeline presets, while the
/// render controller still owns target format policy until quality profiles move
/// behind a provider/config boundary as well.
#[derive(Clone, Copy, Debug)]
pub struct MaterialPipelineBuildProfile {
    pub scene_hdr_color_format: TextureFormat,
    pub shadow_map_color_format: TextureFormat,
}

impl MaterialPipelineBuildProfile {
    #[inline]
    pub const fn new(
        scene_hdr_color_format: TextureFormat,
        shadow_map_color_format: TextureFormat,
    ) -> Self {
        Self {
            scene_hdr_color_format,
            shadow_map_color_format,
        }
    }
}

/// Engine-side bundle consumed by the current lit mesh and shadow passes.
///
/// This remains backend-neutral: all handles are stable `RenderApi` resource ids,
/// never Vulkan/WGPU/native handles.
#[derive(Clone, Copy)]
pub struct LitPipeline {
    pub bgl: BindGroupLayoutId,
    pub white_texture: TextureId,
    pub flat_normal_texture: TextureId,
    pub repeat_sampler: SamplerId,
    pub clamp_sampler: SamplerId,
    #[allow(dead_code)]
    pub vs: ShaderId,
    #[allow(dead_code)]
    pub fs: ShaderId,
    #[allow(dead_code)]
    pub terrain_fs: ShaderId,
    #[allow(dead_code)]
    pub shadow_vs: ShaderId,
    #[allow(dead_code)]
    pub shadow_fs: ShaderId,
    pub pipeline: PipelineId,
    pub double_sided_pipeline: PipelineId,
    pub terrain_pipeline: PipelineId,
    pub shadow_pipeline: PipelineId,
    pub shadow_double_sided_pipeline: PipelineId,
    #[allow(dead_code)]
    pub instanced_vs: ShaderId,
    #[allow(dead_code)]
    pub instanced_fs: ShaderId,
    #[allow(dead_code)]
    pub shadow_instanced_vs: ShaderId,
    pub instanced_pipeline: PipelineId,
    pub instanced_double_sided_pipeline: PipelineId,
    pub shadow_instanced_pipeline: PipelineId,
    pub shadow_instanced_double_sided_pipeline: PipelineId,
}

/// std140 layout consumed by the current lit shader family.
pub const LIT_UBO_SIZE: u64 = 464;

/// Vertex stride consumed by the current instanced lit shader family.
///
/// Layout:
/// - model matrix columns: 64 bytes
/// - pass MVP matrix columns: 64 bytes
/// - base color: 16 bytes
/// - UV transform: 16 bytes
/// - material params: 16 bytes
/// - emissive radiance + pad: 16 bytes
pub const LIT_INSTANCE_VERTEX_STRIDE: u32 = 192;

/// Engine-side material pipeline bundle.
#[derive(Clone, Copy)]
pub enum MaterialGpuPipeline {
    Lit(LitPipeline),
}

impl MaterialGpuPipeline {
    #[inline]
    pub const fn lit(self) -> Option<LitPipeline> {
        match self {
            Self::Lit(pipeline) => Some(pipeline),
        }
    }
}

/// Host-side provider contract for an engine-visible material-domain pipeline.
///
/// Providers may cache shader bytecode and `RenderApi` resource ids internally.
/// The reusable render controller owns only this trait object and the selected
/// domain key; GameReady/FPS shader paths and presets live in the feature crate.
pub trait MaterialGpuPipelineProvider: Send {
    fn key(&self) -> MaterialGpuPipelineKey;

    fn require_pipeline(
        &mut self,
        profile: MaterialPipelineBuildProfile,
        r: &mut dyn RenderApi,
    ) -> EngineResult<MaterialGpuPipeline>;
}
