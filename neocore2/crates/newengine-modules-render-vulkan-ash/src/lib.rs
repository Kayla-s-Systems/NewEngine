mod error;
mod render_api;
mod service;
mod vulkan;

use abi_stable::erased_types::TD_Opaque;
use abi_stable::std_types::{RResult, RString, RVec};
use newengine_platform_api::{
    NativeWindowBackendV1, NativeWindowHandlesV1, PlatformWindowReadyV1,
    PLATFORM_WINDOW_SERVICE_ID, PLATFORM_WINDOW_SERVICE_METHOD_SNAPSHOT_JSON_V1,
};
use newengine_plugin_api::prelude::*;
use newengine_render_api::{
    decode_json, RenderBackendInfoV1, RENDER_SERVICE_ID,
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use serde_json::Value;

use crate::error::VkRenderError;
use crate::render_api::VulkanRenderApi;
use crate::service::VulkanRenderService;

pub const RENDER_BACKEND_ID: &str = "newengine.renderer.vulkan";
pub const RENDER_BACKEND_NAME: &str = "NewEngine Renderer Vulkan";
pub const RENDER_BACKEND_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RENDER_BACKEND_ALIASES: &[&str] = &["vulkan", "vulkan_ash", "newengine.renderer.vulkan"];
pub const RENDER_BACKEND_DEFAULT_SETTINGS_JSON: &str = include_str!("../default_settings.json");

#[derive(Debug, Clone)]
struct VulkanBackendConfig {
    clear_color: [f32; 4],
    debug_text: String,
}

impl Default for VulkanBackendConfig {
    fn default() -> Self {
        Self {
            clear_color: [0.0, 0.0, 0.0, 0.0],
            debug_text: "NewEngine | Vulkan".to_owned(),
        }
    }
}

#[derive(Default)]
struct VulkanRendererPlugin {
    enabled: bool,
}

impl VulkanRendererPlugin {
    #[inline]
    fn descriptor() -> PluginDescriptor {
        PluginDescriptor::builder(
            RENDER_BACKEND_ID,
            RENDER_BACKEND_NAME,
            RENDER_BACKEND_VERSION,
            PluginKind::Runtime,
        )
            .provides_service(
                RENDER_SERVICE_ID,
                1,
                r#"{"role":"render-device-bridge"}"#,
            )
            .push(
                CapabilityDesc::new(
                    "render.backend.v1",
                    CapabilityRole::Provides,
                    CapabilityKind::Other,
                    1,
                )
                    .with_json(r#"{"backend":"vulkan"}"#),
            )
            .requires_service(
                PLATFORM_WINDOW_SERVICE_ID,
                1,
                r#"{"role":"platform-window-snapshot"}"#,
            )
            .requires_service(
                "asset.manager",
                1,
                r#"{"role":"shader-assets"}"#,
            )
            .build()
    }
}

impl PluginModuleV3 for VulkanRendererPlugin {
    fn descriptor_v3(&self) -> PluginDescriptor {
        Self::descriptor()
    }

    fn config_defaults_v1(&self) -> RResult<ConfigBlobV1, RString> {
        RResult::ROk(ConfigBlobV1 {
            content_type: "application/json".into(),
            bytes: RENDER_BACKEND_DEFAULT_SETTINGS_JSON.as_bytes().to_vec().into(),
            format_version: 1,
        })
    }

    fn config_apply_patches_v1(
        &self,
        base: &ConfigBlobV1,
        patches: RVec<ConfigPatchV1>,
    ) -> RResult<ConfigApplyResultV1, RString> {
        let mut effective = match parse_json_object(base.bytes.as_slice(), "render defaults") {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        for patch in patches.iter() {
            let patch_value = match parse_json_object(patch.bytes.as_slice(), "render patch") {
                Ok(v) => v,
                Err(e) => return RResult::RErr(RString::from(e)),
            };
            merge_json_replace(&mut effective, &patch_value);
        }

        let bytes = match serde_json::to_vec(&effective) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e.to_string())),
        };

        RResult::ROk(ConfigApplyResultV1 {
            effective: ConfigBlobV1 {
                content_type: "application/json".into(),
                bytes: bytes.into(),
                format_version: 1,
            },
            diags: RVec::new(),
            changed: true,
        })
    }

    fn config_supports_live_update_v1(&self) -> bool {
        false
    }

    fn config_update_live_v1(
        &mut self,
        _effective: &ConfigBlobV1,
    ) -> RResult<RVec<ConfigDiagV1>, RString> {
        RResult::ROk(RVec::new())
    }

    fn init_v3(&mut self, host: HostApiV1, effective: ConfigBlobV1) -> RResult<(), RString> {
        if !backend_is_selected(RENDER_BACKEND_ID, RENDER_BACKEND_ALIASES) {
            log::info!(
                "render plugin: '{}' is not selected by NEWENGINE_RENDER_BACKEND; staying inactive",
                RENDER_BACKEND_ID
            );
            self.enabled = false;
            return RResult::ROk(());
        }

        let config = match parse_backend_config(&effective) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        let snapshot = match request_platform_window_snapshot(host.clone()) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        let (display, window) = match native_to_raw_handles(snapshot.handles) {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e)),
        };

        let mut renderer = match unsafe {
            vulkan::VulkanRenderer::new(
                host.clone(),
                display,
                window,
                snapshot.surface.width,
                snapshot.surface.height,
            )
        } {
            Ok(v) => v,
            Err(e) => return RResult::RErr(RString::from(e.to_string())),
        };
        renderer.set_debug_text(&config.debug_text);

        let service = VulkanRenderService::new(
            VulkanRenderApi::new(renderer, snapshot.surface.width, snapshot.surface.height),
            RenderBackendInfoV1 {
                backend_id: RENDER_BACKEND_ID.to_owned(),
                backend_name: RENDER_BACKEND_NAME.to_owned(),
                backend_version: RENDER_BACKEND_VERSION.to_owned(),
                debug_text: config.debug_text.clone(),
                clear_color: config.clear_color,
            },
        );
        let dyn_svc = ServiceV1_TO::from_value(service, TD_Opaque);

        match (host.register_service_v1)(dyn_svc) {
            RResult::ROk(()) => {
                log::info!(
                    "render plugin: service registered id='{}' backend='{}' size={}x{}",
                    RENDER_SERVICE_ID,
                    RENDER_BACKEND_ID,
                    snapshot.surface.width,
                    snapshot.surface.height
                );
                self.enabled = true;
                RResult::ROk(())
            }
            RResult::RErr(e) => RResult::RErr(e),
        }
    }

    fn start(&mut self) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn fixed_update(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn update(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn render(&mut self, _dt: f32) -> RResult<(), RString> {
        RResult::ROk(())
    }

    fn shutdown(&mut self) {
        self.enabled = false;
    }
}

#[derive(Default)]
struct VulkanCompatV1;

impl PluginModule for VulkanCompatV1 {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: RENDER_BACKEND_ID.into(),
            name: RENDER_BACKEND_NAME.into(),
            version: RENDER_BACKEND_VERSION.into(),
        }
    }

    fn init(&mut self, _host: HostApiV1) -> RResult<(), RString> { RResult::ROk(()) }
    fn start(&mut self) -> RResult<(), RString> { RResult::ROk(()) }
    fn fixed_update(&mut self, _dt: f32) -> RResult<(), RString> { RResult::ROk(()) }
    fn update(&mut self, _dt: f32) -> RResult<(), RString> { RResult::ROk(()) }
    fn render(&mut self, _dt: f32) -> RResult<(), RString> { RResult::ROk(()) }
    fn shutdown(&mut self) {}
}

#[derive(Default)]
struct VulkanCompatV2;

impl PluginModuleV2 for VulkanCompatV2 {
    fn descriptor(&self) -> PluginDescriptor {
        VulkanRendererPlugin::descriptor()
    }

    fn init(&mut self, _host: HostApiV1) -> RResult<(), RString> { RResult::ROk(()) }
    fn start(&mut self) -> RResult<(), RString> { RResult::ROk(()) }
    fn fixed_update(&mut self, _dt: f32) -> RResult<(), RString> { RResult::ROk(()) }
    fn update(&mut self, _dt: f32) -> RResult<(), RString> { RResult::ROk(()) }
    fn render(&mut self, _dt: f32) -> RResult<(), RString> { RResult::ROk(()) }
    fn shutdown(&mut self) {}
}

extern "C" fn create_v1() -> PluginModuleDyn<'static> {
    PluginModule_TO::from_value(VulkanCompatV1, TD_Opaque)
}

extern "C" fn create_v2() -> PluginModuleV2Dyn<'static> {
    PluginModuleV2_TO::from_value(VulkanCompatV2, TD_Opaque)
}

extern "C" fn create_v3() -> PluginModuleV3Dyn<'static> {
    PluginModuleV3_TO::from_value(VulkanRendererPlugin::default(), TD_Opaque)
}

export_plugin_root!(create_v1, create_v2, create_v3);

fn parse_backend_config(blob: &ConfigBlobV1) -> Result<VulkanBackendConfig, String> {
    if blob.bytes.is_empty() {
        return Ok(VulkanBackendConfig::default());
    }

    let parsed: Value = serde_json::from_slice(blob.bytes.as_slice())
        .map_err(|e| format!("render backend config parse failed: {e}"))?;

    let mut out = VulkanBackendConfig::default();

    if let Some(arr) = parsed.get("clear_color").and_then(Value::as_array) {
        if arr.len() == 4 {
            for (idx, item) in arr.iter().enumerate().take(4) {
                out.clear_color[idx] = item.as_f64().unwrap_or(out.clear_color[idx] as f64) as f32;
            }
        }
    }

    if let Some(text) = parsed.get("debug_text").and_then(Value::as_str) {
        let text = text.trim();
        if !text.is_empty() {
            out.debug_text = text.to_owned();
        }
    }

    Ok(out)
}

fn request_platform_window_snapshot(host: HostApiV1) -> Result<PlatformWindowReadyV1, String> {
    let bytes = (host.call_service_v1)(
        CapabilityId::from(PLATFORM_WINDOW_SERVICE_ID),
        MethodName::from(PLATFORM_WINDOW_SERVICE_METHOD_SNAPSHOT_JSON_V1),
        Blob::from(Vec::<u8>::new()),
    )
        .into_result()
        .map(|blob| blob.into_vec())
        .map_err(|e| e.to_string())?;

    decode_json(&bytes)
}

#[cfg(target_os = "windows")]
fn native_to_raw_handles(
    handles: NativeWindowHandlesV1,
) -> Result<(RawDisplayHandle, RawWindowHandle), String> {
    use std::num::NonZeroIsize;
    use raw_window_handle::{Win32WindowHandle, WindowsDisplayHandle};

    if handles.backend != NativeWindowBackendV1::Win32 {
        return Err(format!("unsupported native window backend: {:?}", handles.backend));
    }

    let hwnd = NonZeroIsize::new(handles.window as isize)
        .ok_or_else(|| VkRenderError::MissingWindowHandles.to_string())?;

    let mut window = Win32WindowHandle::new(hwnd);
    window.hinstance = NonZeroIsize::new(handles.display as isize);

    Ok((
        RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
        RawWindowHandle::Win32(window),
    ))
}

#[cfg(not(target_os = "windows"))]
fn native_to_raw_handles(
    _handles: NativeWindowHandlesV1,
) -> Result<(RawDisplayHandle, RawWindowHandle), String> {
    Err("platform window handle conversion is only implemented for Windows".to_owned())
}

fn backend_is_selected(canonical_id: &str, aliases: &[&str]) -> bool {
    let selected = std::env::var("NEWENGINE_RENDER_BACKEND")
        .ok()
        .unwrap_or_else(|| canonical_id.to_owned());
    let selected = normalize_backend_token(&selected);
    if selected.is_empty() {
        return true;
    }

    if normalize_backend_token(canonical_id) == selected {
        return true;
    }

    aliases
        .iter()
        .map(|v| normalize_backend_token(v))
        .any(|alias| alias == selected)
}

fn normalize_backend_token(input: &str) -> String {
    input
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch.to_ascii_lowercase() } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_json_object(raw: &[u8], what: &str) -> Result<Value, String> {
    let parsed: Value = serde_json::from_slice(raw)
        .map_err(|e| format!("{what} parse failed: {e}"))?;
    if parsed.is_object() {
        Ok(parsed)
    } else {
        Err(format!("{what} must be a JSON object"))
    }
}

fn merge_json_replace(dst: &mut Value, src: &Value) {
    match (dst, src) {
        (Value::Object(dst_map), Value::Object(src_map)) => {
            for (key, src_value) in src_map {
                match dst_map.get_mut(key) {
                    Some(dst_value) => merge_json_replace(dst_value, src_value),
                    None => {
                        dst_map.insert(key.clone(), src_value.clone());
                    }
                }
            }
        }
        (dst_value, src_value) => {
            *dst_value = src_value.clone();
        }
    }
}
