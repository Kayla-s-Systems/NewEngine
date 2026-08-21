#[derive(Clone, Copy)]
pub struct PrimitiveGpu {
    pub vb: newengine_core::render::BufferId,
    pub ib: newengine_core::render::BufferId,
    pub vertex_count: u32,
    pub index_count: u32,
    pub bounds_center: newengine_math::Vec3,
    pub bounds_radius: f32,
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

#[derive(Clone, Copy)]
pub struct PlayerSkinGpu {
    pub vb: newengine_core::render::BufferId,
    pub vertex_count: u32,
    pub max_joint_index: u16,
}

#[derive(Clone, Copy)]
pub struct SkinPaletteGpu {
    pub buffer: newengine_core::render::BufferId,
    pub bg: newengine_core::render::BindGroupId,
    pub capacity_joints: u32,
    pub revision: u64,
}
