use newengine_core::render::{RenderBackendCapabilities, RenderWorkBudget};
use newengine_render_api::{ENGINE_RENDER_SERVICE_ID, RENDER_BACKEND_CAPABILITY_ID, RENDER_SERVICE_ID};
use newengine_service_api::BackendServiceSpec;


pub const RENDER_BACKEND_SERVICE_SPEC: BackendServiceSpec = BackendServiceSpec::new(
    "render",
    ENGINE_RENDER_SERVICE_ID,
    RENDER_SERVICE_ID,
    RENDER_BACKEND_CAPABILITY_ID,
);

#[derive(Debug, Clone)]
pub struct ResolvedRenderBackendConfig {
    pub backend_id: String,
    pub clear_color: [f32; 4],
    pub debug_text: String,
    pub capabilities: RenderBackendCapabilities,
    pub work_budget: RenderWorkBudget,
}
