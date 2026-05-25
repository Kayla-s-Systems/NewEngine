pub(crate) const PLATFORM_RUNTIME_SYMBOL: &[u8] = b"newengine_platform_runtime_run_v1\0";
pub(crate) const PLUGIN_ROOT_SYMBOL: &[u8] = newengine_plugin_api::PLUGIN_ROOT_SYMBOL_BYTES_NUL;
pub(crate) const PLUGIN_SIGNATURE_SYMBOL: &[u8] = b"newengine_plugin_signature_v1\0";

pub(crate) const PLATFORM_PLUGIN_ID: &str = "newengine.platform.winit";
pub(crate) const CT_JSON_MERGE_PATCH: &str = "application/merge-patch+json";