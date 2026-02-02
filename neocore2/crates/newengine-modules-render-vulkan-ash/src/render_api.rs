use newengine_core::render::*;
use newengine_core::{EngineError, EngineResult};

use crate::vulkan::VulkanRenderer;

pub struct VulkanRenderApi {
    renderer: VulkanRenderer,
    target: Extent2D,
}

impl VulkanRenderApi {
    #[inline]
    pub fn new(renderer: VulkanRenderer, width: u32, height: u32) -> Self {
        Self {
            renderer,
            target: Extent2D::new(width, height),
        }
    }

    #[inline]
    fn not_implemented<T>(&self, what: &'static str) -> EngineResult<T> {
        Err(EngineError::other(format!(
            "VulkanRenderApi: not implemented: {what}"
        )))
    }
}

impl RenderApi for VulkanRenderApi {
    fn begin_frame(&mut self, desc: BeginFrameDesc) -> EngineResult<()> {
        self.renderer.set_debug_text("TEST OK");

        self.renderer
            .draw_clear_color(desc.clear_color)
            .map_err(|e| EngineError::other(e.to_string()))?;

        Ok(())
    }

    fn end_frame(&mut self) -> EngineResult<()> {
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) -> EngineResult<()> {
        self.target = Extent2D::new(width, height);
        self.renderer
            .resize(width, height)
            .map_err(|e| EngineError::other(e.to_string()))
    }

    fn create_buffer(&mut self, _desc: BufferDesc) -> EngineResult<BufferId> {
        self.not_implemented("create_buffer")
    }

    fn destroy_buffer(&mut self, _id: BufferId) {}

    fn write_buffer(&mut self, _id: BufferId, _offset: u64, _data: &[u8]) -> EngineResult<()> {
        self.not_implemented("write_buffer")
    }

    fn create_texture(&mut self, _desc: TextureDesc) -> EngineResult<TextureId> {
        self.not_implemented("create_texture")
    }

    fn destroy_texture(&mut self, _id: TextureId) {}

    fn create_sampler(&mut self, _desc: SamplerDesc) -> EngineResult<SamplerId> {
        self.not_implemented("create_sampler")
    }

    fn destroy_sampler(&mut self, _id: SamplerId) {}

    fn create_shader(&mut self, _desc: ShaderDesc) -> EngineResult<ShaderId> {
        self.not_implemented("create_shader")
    }

    fn destroy_shader(&mut self, _id: ShaderId) {}

    fn create_pipeline(&mut self, _desc: PipelineDesc) -> EngineResult<PipelineId> {
        self.not_implemented("create_pipeline")
    }

    fn destroy_pipeline(&mut self, _id: PipelineId) {}

    fn create_bind_group_layout(
        &mut self,
        _desc: BindGroupLayoutDesc,
    ) -> EngineResult<BindGroupLayoutId> {
        self.not_implemented("create_bind_group_layout")
    }

    fn destroy_bind_group_layout(&mut self, _id: BindGroupLayoutId) {}

    fn create_bind_group(&mut self, _desc: BindGroupDesc) -> EngineResult<BindGroupId> {
        self.not_implemented("create_bind_group")
    }

    fn destroy_bind_group(&mut self, _id: BindGroupId) {}

    fn set_viewport(&mut self, _vp: Viewport) -> EngineResult<()> {
        self.not_implemented("set_viewport")
    }

    fn set_scissor(&mut self, _rect: RectI32) -> EngineResult<()> {
        self.not_implemented("set_scissor")
    }

    fn set_pipeline(&mut self, _pipeline: PipelineId) -> EngineResult<()> {
        self.not_implemented("set_pipeline")
    }

    fn set_bind_group(&mut self, _index: u32, _group: BindGroupId) -> EngineResult<()> {
        self.not_implemented("set_bind_group")
    }

    fn set_vertex_buffer(&mut self, _slot: u32, _slice: BufferSlice) -> EngineResult<()> {
        self.not_implemented("set_vertex_buffer")
    }

    fn set_index_buffer(&mut self, _slice: BufferSlice, _format: IndexFormat) -> EngineResult<()> {
        self.not_implemented("set_index_buffer")
    }

    fn draw(&mut self, _args: DrawArgs) -> EngineResult<()> {
        self.not_implemented("draw")
    }

    fn draw_indexed(&mut self, _args: DrawIndexedArgs) -> EngineResult<()> {
        self.not_implemented("draw_indexed")
    }
}