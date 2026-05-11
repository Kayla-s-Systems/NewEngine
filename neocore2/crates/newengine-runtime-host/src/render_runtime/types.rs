use newengine_core::render::{RenderBackendCapabilities, RenderWorkBudget};

pub const DEFAULT_RENDER_BACKEND_ID: &str = "newengine.renderer.vulkan";
pub const NULL_RENDER_BACKEND_ID: &str = "newengine.renderer.null";
pub const DEFAULT_RENDER_BACKEND_CLEAR_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

#[derive(Debug, Clone)]
pub struct ResolvedRenderBackendConfig {
    pub backend_id: String,
    pub clear_color: [f32; 4],
    pub debug_text: String,
    pub capabilities: RenderBackendCapabilities,
    pub work_budget: RenderWorkBudget,
}
