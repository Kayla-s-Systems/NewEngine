mod error;
mod render_api;
mod vulkan;

use crate::error::VkRenderError;
use crate::render_api::VulkanRenderApi;
use newengine_core::host_events::{WindowHandles, WindowInitSize};
use newengine_core::render::{RenderApiRef, RENDER_API_ID, RENDER_API_PROVIDE};
use newengine_core::{EngineError, EngineResult, Module, ModuleCtx};
use newengine_plugin_api::{
    ConfigBlobV1, HostApiV1, RenderBackendDescriptorV1, RENDER_BACKEND_DESCRIPTOR_ABI_V1,
};
use newengine_plugin_host::default_host_api;
use serde_json::Value;

pub const RENDER_BACKEND_ID: &str = "newengine.renderer.vulkan";
pub const RENDER_BACKEND_NAME: &str = "NewEngine Renderer Vulkan";
pub const RENDER_BACKEND_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RENDER_BACKEND_ALIASES: &str = "vulkan,vulkan_ash,newengine.renderer.vulkan";
pub const RENDER_BACKEND_DEFAULT_SETTINGS_JSON: &str =
    include_str!("../default_settings.json");

#[derive(Debug, Clone)]
struct VulkanBackendConfig {
    debug_text: String,
}

impl Default for VulkanBackendConfig {
    fn default() -> Self {
        Self {
            debug_text: "NewEngine | Vulkan".to_owned(),
        }
    }
}

pub struct VulkanAshRenderModule {
    api: Option<RenderApiRef>,
}

impl Default for VulkanAshRenderModule {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Send + 'static> Module<E> for VulkanAshRenderModule {
    fn id(&self) -> &'static str {
        RENDER_BACKEND_ID
    }

    fn provides(&self) -> &'static [newengine_core::ApiProvide] {
        &[RENDER_API_PROVIDE]
    }

    fn init(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let (display, window, w, h) = {
            let handles = ctx.resources().get::<WindowHandles>().ok_or_else(|| {
                EngineError::other(VkRenderError::MissingWindowHandles.to_string())
            })?;

            let size = ctx
                .resources()
                .get::<WindowInitSize>()
                .ok_or_else(|| EngineError::other(VkRenderError::MissingWindowSize.to_string()))?;

            (handles.display, handles.window, size.width, size.height)
        };

        let mut renderer = unsafe { vulkan::VulkanRenderer::new(default_host_api(), display, window, w, h) }
            .map_err(|e| EngineError::other(e.to_string()))?;
        renderer.set_debug_text(&VulkanBackendConfig::default().debug_text);

        let api = RenderApiRef::new(VulkanRenderApi::new(renderer, w, h));

        ctx.resources_mut()
            .register_api(RENDER_API_ID, api.clone())?;

        self.api = Some(api);
        Ok(())
    }

    fn render(&mut self, _ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        Ok(())
    }

    fn shutdown(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let _ = ctx
            .resources_mut()
            .unregister_api::<RenderApiRef>(RENDER_API_ID);
        self.api = None;
        Ok(())
    }
}

impl VulkanAshRenderModule {
    #[inline]
    pub fn new() -> Self {
        Self { api: None }
    }
}

#[no_mangle]
pub unsafe extern "C" fn newengine_render_backend_describe_v1() -> RenderBackendDescriptorV1 {
    RenderBackendDescriptorV1 {
        abi_version: RENDER_BACKEND_DESCRIPTOR_ABI_V1,
        id_ptr: RENDER_BACKEND_ID.as_ptr(),
        id_len: RENDER_BACKEND_ID.len(),
        name_ptr: RENDER_BACKEND_NAME.as_ptr(),
        name_len: RENDER_BACKEND_NAME.len(),
        version_ptr: RENDER_BACKEND_VERSION.as_ptr(),
        version_len: RENDER_BACKEND_VERSION.len(),
        aliases_ptr: RENDER_BACKEND_ALIASES.as_ptr(),
        aliases_len: RENDER_BACKEND_ALIASES.len(),
        default_settings_ptr: RENDER_BACKEND_DEFAULT_SETTINGS_JSON.as_ptr(),
        default_settings_len: RENDER_BACKEND_DEFAULT_SETTINGS_JSON.len(),
    }
}

#[no_mangle]
pub unsafe fn newengine_render_backend_create_v1(
    host: HostApiV1,
    display: raw_window_handle::RawDisplayHandle,
    window: raw_window_handle::RawWindowHandle,
    width: u32,
    height: u32,
    effective: ConfigBlobV1,
) -> Result<Box<dyn newengine_core::render::RenderApi + 'static>, String> {
    let config = parse_backend_config(&effective)?;
    let mut renderer = unsafe { vulkan::VulkanRenderer::new(host, display, window, width, height) }
        .map_err(|e| e.to_string())?;
    renderer.set_debug_text(&config.debug_text);
    Ok(Box::new(VulkanRenderApi::new(renderer, width, height)))
}

fn parse_backend_config(blob: &ConfigBlobV1) -> Result<VulkanBackendConfig, String> {
    if blob.bytes.is_empty() {
        return Ok(VulkanBackendConfig::default());
    }

    let parsed: Value = serde_json::from_slice(blob.bytes.as_slice())
        .map_err(|e| format!("render backend config parse failed: {e}"))?;

    let mut out = VulkanBackendConfig::default();


    if let Some(text) = parsed.get("debug_text").and_then(Value::as_str) {
        let text = text.trim();
        if !text.is_empty() {
            out.debug_text = text.to_owned();
        }
    }

    Ok(out)
}
