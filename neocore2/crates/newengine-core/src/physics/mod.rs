use crate::error::{EngineError, EngineResult};
use crate::module::{ApiProvide, ApiVersion};
use parking_lot::{Mutex, MutexGuard};
use std::sync::Arc;

pub use newengine_physics_api::*;

pub const PHYSICS_API_ID: &str = newengine_physics_api::PHYSICS_SERVICE_ID;
pub const PHYSICS_API_VERSION: ApiVersion = ApiVersion::new(0, 1, 0);
pub const PHYSICS_API_PROVIDE: ApiProvide = ApiProvide::new(PHYSICS_API_ID, PHYSICS_API_VERSION);

#[derive(Debug, Clone, Default)]
pub struct PhysicsBackendStatus {
    pub degraded: bool,
    pub phase: Option<&'static str>,
    pub message: Option<String>,
}

impl PhysicsBackendStatus {
    #[inline]
    pub fn healthy() -> Self { Self::default() }

    #[inline]
    pub fn degraded(phase: &'static str, message: impl Into<String>) -> Self {
        Self { degraded: true, phase: Some(phase), message: Some(message.into()) }
    }
}

pub trait PhysicsApi: Send {
    fn backend_info(&mut self) -> EngineResult<PhysicsBackendInfo>;
    fn step_frame(&mut self, input: PhysicsFrameInput) -> EngineResult<PhysicsFrameOutput>;

    #[inline]
    fn diagnostics_snapshot(&mut self) -> EngineResult<PhysicsBackendInfo> {
        self.backend_info()
    }
}

#[derive(Clone)]
pub struct PhysicsApiRef(Arc<Mutex<Box<dyn PhysicsApi + 'static>>>);

impl PhysicsApiRef {
    #[inline]
    pub fn new(api: impl PhysicsApi + 'static) -> Self {
        Self::from_box(Box::new(api))
    }

    #[inline]
    pub fn from_box(api: Box<dyn PhysicsApi + 'static>) -> Self {
        Self(Arc::new(Mutex::new(api)))
    }

    #[inline]
    pub fn lock(&self) -> MutexGuard<'_, Box<dyn PhysicsApi + 'static>> {
        self.0.lock()
    }
}

#[inline]
pub fn require_physics_api<'a, E: Send + 'static>(
    ctx: &'a crate::module::ModuleCtx<'_, E>,
) -> EngineResult<&'a PhysicsApiRef> {
    ctx.api_required::<PhysicsApiRef>(PHYSICS_API_ID)
        .map_err(|_| EngineError::other("Physics API is not available (missing physics backend module?)"))
}
