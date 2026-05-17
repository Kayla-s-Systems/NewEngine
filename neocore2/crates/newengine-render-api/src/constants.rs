pub const RENDER_SERVICE_ID: &str = "render.api";
pub const RENDER_BACKEND_CAPABILITY_ID: &str = "render.backend";
pub const RENDER_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const RENDER_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const RENDER_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;

pub type Color4 = [f32; 4];
pub type RenderWireResult<T> = Result<T, String>;
