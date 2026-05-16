#[derive(Clone, Copy)]
pub struct LitPipeline {
    pub bgl: newengine_core::render::BindGroupLayoutId,
    pub white_texture: newengine_core::render::TextureId,
    pub flat_normal_texture: newengine_core::render::TextureId,
    pub repeat_sampler: newengine_core::render::SamplerId,
    pub clamp_sampler: newengine_core::render::SamplerId,
    #[allow(dead_code)]
    pub vs: newengine_core::render::ShaderId,
    #[allow(dead_code)]
    pub fs: newengine_core::render::ShaderId,
    #[allow(dead_code)]
    pub terrain_fs: newengine_core::render::ShaderId,
    #[allow(dead_code)]
    pub shadow_vs: newengine_core::render::ShaderId,
    #[allow(dead_code)]
    pub shadow_fs: newengine_core::render::ShaderId,
    pub pipeline: newengine_core::render::PipelineId,
    pub double_sided_pipeline: newengine_core::render::PipelineId,
    pub terrain_pipeline: newengine_core::render::PipelineId,
    pub shadow_pipeline: newengine_core::render::PipelineId,
    pub shadow_double_sided_pipeline: newengine_core::render::PipelineId,
    #[allow(dead_code)]
    pub instanced_vs: newengine_core::render::ShaderId,
    #[allow(dead_code)]
    pub instanced_fs: newengine_core::render::ShaderId,
    #[allow(dead_code)]
    pub shadow_instanced_vs: newengine_core::render::ShaderId,
    pub instanced_pipeline: newengine_core::render::PipelineId,
    pub instanced_double_sided_pipeline: newengine_core::render::PipelineId,
    pub shadow_instanced_pipeline: newengine_core::render::PipelineId,
    pub shadow_instanced_double_sided_pipeline: newengine_core::render::PipelineId,
}

// std140 layout (see assets/shaders/game_lit_*):
// mat4 mvp (64)
// mat4 model (64)
// vec4 base_color (16)
// vec4 emissive (16)
// vec4 ambient (16)
// vec4 dir_dir_intensity (16)
// vec4 dir_color (16)
// point lights: 4 * (vec4 pos_range + vec4 color_intensity) = 4 * 32 = 128
// vec4 point_count_pad (16)
// vec4 uv_transform (16)
// vec4 material_params (16)
// mat4 light_mvp (64)
// vec4 shadow_params (16)
// Total: 464 bytes.
pub const LIT_UBO_SIZE: u64 = 464;

#[derive(Clone, Copy)]
pub struct PrimitiveGpu {
    pub vb: newengine_core::render::BufferId,
    pub ib: newengine_core::render::BufferId,
    pub index_count: u32,
}

#[derive(Clone, Copy)]
pub struct DebugLineGpu {
    pub vb: newengine_core::render::BufferId,
    pub ubo: newengine_core::render::BufferId,
    pub bg: newengine_core::render::BindGroupId,
    pub bgl: newengine_core::render::BindGroupLayoutId,
    #[allow(dead_code)]
    pub vs: newengine_core::render::ShaderId,
    #[allow(dead_code)]
    pub fs: newengine_core::render::ShaderId,
    pub pipeline: newengine_core::render::PipelineId,
    pub capacity_vertices: u32,
}


pub(super) const DEBUG_LINE_UBO_SIZE: u64 = 16;
