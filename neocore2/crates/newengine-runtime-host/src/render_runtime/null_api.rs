use newengine_core::render::{
    BeginFrameDesc, BeginRenderTargetDesc, BindGroupDesc, BindGroupId, BindGroupLayoutDesc,
    BindGroupLayoutId, BufferDesc, BufferId, BufferSlice, DrawArgs, DrawIndexedArgs,
    IndexFormat, PipelineDesc, PipelineId, RectI32, RenderApi, RenderTargetDesc,
    RenderTargetId, SamplerDesc, SamplerId, ShaderDesc, ShaderId, TextureDesc, TextureId,
    UiDrawList, UiTexId, Viewport,
};
use newengine_core::{EngineError, EngineResult};

#[derive(Debug, Default)]
pub(crate) struct NullRenderApi {
    next_id: u32,
}

impl NullRenderApi {
    #[inline]
    fn alloc_id(&mut self) -> u32 {
        self.next_id = self.next_id.saturating_add(1).max(1);
        self.next_id
    }
}

impl RenderApi for NullRenderApi {
    fn begin_frame(&mut self, _desc: BeginFrameDesc) -> EngineResult<()> {
        Ok(())
    }

    fn set_ui_draw_list(&mut self, _ui: UiDrawList) {}

    fn end_frame(&mut self) -> EngineResult<()> {
        Ok(())
    }

    fn resize(&mut self, _width: u32, _height: u32) -> EngineResult<()> {
        Ok(())
    }

    fn create_render_target(&mut self, _desc: RenderTargetDesc) -> EngineResult<RenderTargetId> {
        Ok(RenderTargetId::new(self.alloc_id()))
    }

    fn destroy_render_target(&mut self, _id: RenderTargetId) {}

    fn render_target_ui_tex_id(&self, _id: RenderTargetId) -> EngineResult<UiTexId> {
        Ok(UiTexId::new(0))
    }

    fn begin_render_target(&mut self, _desc: BeginRenderTargetDesc) -> EngineResult<()> {
        Ok(())
    }

    fn end_render_target(&mut self) -> EngineResult<()> {
        Ok(())
    }

    fn create_buffer(&mut self, _desc: BufferDesc) -> EngineResult<BufferId> {
        Ok(BufferId::new(self.alloc_id()))
    }

    fn destroy_buffer(&mut self, _id: BufferId) {}

    fn write_buffer(&mut self, _id: BufferId, _offset: u64, _data: &[u8]) -> EngineResult<()> {
        Ok(())
    }

    fn create_texture(&mut self, _desc: TextureDesc) -> EngineResult<TextureId> {
        Err(EngineError::other(
            "null render backend: textures are not supported in headless mode",
        ))
    }

    fn destroy_texture(&mut self, _id: TextureId) {}

    fn create_sampler(&mut self, _desc: SamplerDesc) -> EngineResult<SamplerId> {
        Err(EngineError::other(
            "null render backend: samplers are not supported in headless mode",
        ))
    }

    fn destroy_sampler(&mut self, _id: SamplerId) {}

    fn create_shader(&mut self, _desc: ShaderDesc) -> EngineResult<ShaderId> {
        Ok(ShaderId::new(self.alloc_id()))
    }

    fn destroy_shader(&mut self, _id: ShaderId) {}

    fn create_pipeline(&mut self, _desc: PipelineDesc) -> EngineResult<PipelineId> {
        Ok(PipelineId::new(self.alloc_id()))
    }

    fn destroy_pipeline(&mut self, _id: PipelineId) {}

    fn create_bind_group_layout(
        &mut self,
        _desc: BindGroupLayoutDesc,
    ) -> EngineResult<BindGroupLayoutId> {
        Ok(BindGroupLayoutId::new(self.alloc_id()))
    }

    fn destroy_bind_group_layout(&mut self, _id: BindGroupLayoutId) {}

    fn create_bind_group(&mut self, _desc: BindGroupDesc) -> EngineResult<BindGroupId> {
        Ok(BindGroupId::new(self.alloc_id()))
    }

    fn destroy_bind_group(&mut self, _id: BindGroupId) {}

    fn set_viewport(&mut self, _vp: Viewport) -> EngineResult<()> {
        Ok(())
    }

    fn set_scissor(&mut self, _rect: RectI32) -> EngineResult<()> {
        Ok(())
    }

    fn set_pipeline(&mut self, _pipeline: PipelineId) -> EngineResult<()> {
        Ok(())
    }

    fn set_bind_group(&mut self, _index: u32, _group: BindGroupId) -> EngineResult<()> {
        Ok(())
    }

    fn set_vertex_buffer(&mut self, _slot: u32, _slice: BufferSlice) -> EngineResult<()> {
        Ok(())
    }

    fn set_index_buffer(
        &mut self,
        _slice: BufferSlice,
        _format: IndexFormat,
    ) -> EngineResult<()> {
        Ok(())
    }

    fn draw(&mut self, _args: DrawArgs) -> EngineResult<()> {
        Ok(())
    }

    fn draw_indexed(&mut self, _args: DrawIndexedArgs) -> EngineResult<()> {
        Ok(())
    }
}