pub const RENDER_SERVICE_ID: &str = "render.api.v1";
pub const RENDER_SERVICE_METHOD_INVOKE_V1: &str = "invoke_json_v1";
pub const RENDER_SERVICE_METHOD_INFO_V1: &str = "info_json_v1";

pub type Color4 = [f32; 4];
pub type RenderWireResult<T> = Result<T, String>;
