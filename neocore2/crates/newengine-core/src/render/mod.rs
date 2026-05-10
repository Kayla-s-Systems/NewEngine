use crate::error::{EngineError, EngineResult};
use crate::module::{ApiProvide, ApiVersion};
use parking_lot::{Mutex, MutexGuard};
use std::sync::Arc;

pub use newengine_render_api::*;

pub const RENDER_API_ID: &str = "render.api";
pub const RENDER_API_VERSION: ApiVersion = ApiVersion::new(0, 4, 0);
pub const RENDER_API_PROVIDE: ApiProvide = ApiProvide::new(RENDER_API_ID, RENDER_API_VERSION);

pub trait RenderApi: Send {
    fn begin_frame(&mut self, desc: BeginFrameDesc) -> EngineResult<()>;
    fn set_ui_draw_list(&mut self, ui: UiDrawList);
    fn end_frame(&mut self) -> EngineResult<()>;
    fn resize(&mut self, width: u32, height: u32) -> EngineResult<()>;

    fn create_render_target(&mut self, desc: RenderTargetDesc) -> EngineResult<RenderTargetId>;
    fn destroy_render_target(&mut self, id: RenderTargetId);
    fn render_target_ui_tex_id(&self, id: RenderTargetId) -> EngineResult<UiTexId>;
    fn render_target_color_texture_id(&self, id: RenderTargetId) -> EngineResult<TextureId>;

    fn begin_render_target(&mut self, desc: BeginRenderTargetDesc) -> EngineResult<()>;
    fn end_render_target(&mut self) -> EngineResult<()>;

    fn create_buffer(&mut self, desc: BufferDesc) -> EngineResult<BufferId>;
    fn destroy_buffer(&mut self, id: BufferId);
    fn write_buffer(&mut self, id: BufferId, offset: u64, data: &[u8]) -> EngineResult<()>;

    fn create_texture(&mut self, desc: TextureDesc) -> EngineResult<TextureId>;
    fn destroy_texture(&mut self, id: TextureId);

    fn create_sampler(&mut self, desc: SamplerDesc) -> EngineResult<SamplerId>;
    fn destroy_sampler(&mut self, id: SamplerId);

    fn create_shader(&mut self, desc: ShaderDesc) -> EngineResult<ShaderId>;
    fn destroy_shader(&mut self, id: ShaderId);

    fn create_pipeline(&mut self, desc: PipelineDesc) -> EngineResult<PipelineId>;
    fn destroy_pipeline(&mut self, id: PipelineId);

    fn create_bind_group_layout(
        &mut self,
        desc: BindGroupLayoutDesc,
    ) -> EngineResult<BindGroupLayoutId>;
    fn destroy_bind_group_layout(&mut self, id: BindGroupLayoutId);

    fn create_bind_group(&mut self, desc: BindGroupDesc) -> EngineResult<BindGroupId>;
    fn destroy_bind_group(&mut self, id: BindGroupId);

    fn set_viewport(&mut self, vp: Viewport) -> EngineResult<()>;
    fn set_scissor(&mut self, rect: RectI32) -> EngineResult<()>;

    fn set_pipeline(&mut self, pipeline: PipelineId) -> EngineResult<()>;
    fn set_bind_group(&mut self, index: u32, group: BindGroupId) -> EngineResult<()>;

    fn set_vertex_buffer(&mut self, slot: u32, slice: BufferSlice) -> EngineResult<()>;
    fn set_index_buffer(&mut self, slice: BufferSlice, format: IndexFormat) -> EngineResult<()>;

    fn draw(&mut self, args: DrawArgs) -> EngineResult<()>;
    fn draw_indexed(&mut self, args: DrawIndexedArgs) -> EngineResult<()>;
}

#[derive(Clone)]
pub struct RenderApiRef(Arc<Mutex<Box<dyn RenderApi + 'static>>>);

impl RenderApiRef {
    #[inline]
    pub fn new(api: impl RenderApi + 'static) -> Self {
        Self::from_box(Box::new(api))
    }

    #[inline]
    pub fn from_box(api: Box<dyn RenderApi + 'static>) -> Self {
        Self(Arc::new(Mutex::new(api)))
    }

    #[inline]
    pub fn lock(&self) -> MutexGuard<'_, Box<dyn RenderApi + 'static>> {
        self.0.lock()
    }
}

#[inline]
pub fn require_render_api<'a, E: Send + 'static>(
    ctx: &'a crate::module::ModuleCtx<'_, E>,
) -> EngineResult<&'a RenderApiRef> {
    ctx.api_required::<RenderApiRef>(RENDER_API_ID)
        .map_err(|_| EngineError::other("Render API is not available (missing render backend module?)"))
}
