pub const RENDER_SERVICE_ID: &str = "render.api";
pub const RENDER_SERVICE_METHOD_INVOKE: &str = "invoke_json";
pub const RENDER_SERVICE_METHOD_INFO: &str = "info_json";
pub const RENDER_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;

pub type Color4 = [f32; 4];
pub type RenderWireResult<T> = Result<T, String>;
