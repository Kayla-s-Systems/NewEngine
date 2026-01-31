use crate::error::{EngineError, EngineResult};
use crate::module::{ApiProvide, ApiVersion};

use parking_lot::{Mutex, MutexGuard};
use std::sync::Arc;

pub const RENDER_API_ID: &str = "render.api";
pub const RENDER_API_VERSION: ApiVersion = ApiVersion::new(0, 1, 0);
pub const RENDER_API_PROVIDE: ApiProvide = ApiProvide::new(RENDER_API_ID, RENDER_API_VERSION);

pub type Color4 = [f32; 4];

#[derive(Debug, Clone, Copy)]
pub struct BeginFrameDesc {
    pub clear_color: Color4,
}

impl BeginFrameDesc {
    #[inline]
    pub const fn new(clear_color: Color4) -> Self {
        Self { clear_color }
    }
}

/// Render backend contract.
///
/// Must be `Send` because modules are `Send` and RenderApiRef is stored inside modules/resources.
pub trait RenderApi: Send {
    fn begin_frame(&mut self, desc: BeginFrameDesc) -> EngineResult<()>;
    fn end_frame(&mut self) -> EngineResult<()>;
    fn resize(&mut self, width: u32, height: u32) -> EngineResult<()>;
}

#[derive(Clone)]
pub struct RenderApiRef(Arc<Mutex<Box<dyn RenderApi + Send + 'static>>>);

impl RenderApiRef {
    #[inline]
    pub fn new(api: impl RenderApi + Send + 'static) -> Self {
        Self(Arc::new(Mutex::new(Box::new(api))))
    }

    #[inline]
    pub fn lock(&self) -> MutexGuard<'_, Box<dyn RenderApi + Send + 'static>> {
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