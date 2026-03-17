use newengine_platform_api::PlatformAppConfigV1;
use newengine_plugin_api::PluginDescriptor;

pub struct ResolvedPlatformRuntimeConfig {
    pub plugin_id: String,
    pub plugin_name: String,
    pub plugin_version: String,
    pub descriptor: PluginDescriptor,
    pub config: PlatformAppConfigV1,
    pub icon_path: Option<String>,
}