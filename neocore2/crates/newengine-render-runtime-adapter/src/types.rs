use newengine_core::render::{RenderBackendCapabilities, RenderWorkBudget};
pub use newengine_render_api::RENDER_BACKEND_SERVICE_SPEC;

#[derive(Debug, Clone)]
pub struct ResolvedRenderBackendConfig {
    pub backend_id: String,
    pub clear_color: [f32; 4],
    pub debug_text: String,
    pub capabilities: RenderBackendCapabilities,
    pub work_budget: RenderWorkBudget,
}
