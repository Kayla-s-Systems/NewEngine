use std::path::PathBuf;

use newengine_core::render::{RenderApiRef, RENDER_API_ID, RENDER_API_PROVIDE};
use newengine_core::{EngineResult, Module, ModuleCtx};
use newengine_plugin_api::{CapabilityKind, CapabilityRole};
use newengine_render_api::RENDER_SERVICE_ID;

use crate::render_runtime::backend_match::backend_matches;
use crate::render_runtime::client::RenderServiceClient;
use crate::render_runtime::null_api::NullRenderApi;
use crate::render_runtime::service_api::ServiceBackedRenderApi;
use crate::render_runtime::types::{
    ResolvedRenderBackendConfig,
    DEFAULT_RENDER_BACKEND_CLEAR_COLOR,
    NULL_RENDER_BACKEND_ID,
};

pub struct RenderBackendRuntimeModule {
    backend_spec: String,
    _modules_dir: PathBuf,
    api: Option<RenderApiRef>,
}

impl RenderBackendRuntimeModule {
    #[inline]
    pub fn new(backend_spec: String, modules_dir: PathBuf) -> Self {
        Self {
            backend_spec,
            _modules_dir: modules_dir,
            api: None,
        }
    }

    #[inline]
    fn null_debug_text(&self) -> String {
        format!("NewEngine | Headless ({})", self.backend_spec)
    }

    #[inline]
    fn plugin_declares_render_service(plugin: &newengine_plugin_host::PluginSnapshotEntry) -> bool {
        plugin.capabilities.iter().any(|cap| {
            cap.role == CapabilityRole::Provides
                && cap.kind == CapabilityKind::ServiceV1
                && cap.id.as_str() == RENDER_SERVICE_ID
        })
    }

    fn explain_backend_unavailability<E: Send + 'static>(
        &self,
        ctx: &ModuleCtx<'_, E>,
        service_error: &str,
    ) -> String {
        let Some(snapshot) = ctx.resources().get::<newengine_plugin_host::PluginsSnapshot>() else {
            return format!(
                "render service '{}' is unavailable: {}",
                RENDER_SERVICE_ID, service_error
            );
        };

        let configured = self.backend_spec.as_str();

        if let Some(plugin) = snapshot.plugins.iter().find(|plugin| plugin.id == configured) {
            let declared = if Self::plugin_declares_render_service(plugin) {
                "declares render.api.v1"
            } else {
                "does not declare render.api.v1"
            };

            return match plugin.state.as_str() {
                "disabled" => format!(
                    "configured render plugin '{}' is loaded but disabled: {}",
                    configured,
                    plugin
                        .disabled_reason
                        .as_deref()
                        .unwrap_or("reason was not provided by the plugin host")
                ),
                state => format!(
                    "configured render plugin '{}' is loaded (state='{}', {}) but service '{}' is not registered: {}",
                    configured,
                    state,
                    declared,
                    RENDER_SERVICE_ID,
                    service_error
                ),
            };
        }

        let loaded_render_plugins: Vec<String> = snapshot
            .plugins
            .iter()
            .filter(|plugin| {
                plugin.id.starts_with("newengine.renderer.")
                    || Self::plugin_declares_render_service(plugin)
            })
            .map(|plugin| format!("{}:{}", plugin.id, plugin.state))
            .collect();

        if loaded_render_plugins.is_empty() {
            format!(
                "configured render plugin '{}' is not loaded and no render plugins are active; service '{}' is unavailable: {}",
                configured,
                RENDER_SERVICE_ID,
                service_error
            )
        } else {
            format!(
                "configured render plugin '{}' is not loaded; loaded render plugins=[{}]; service '{}' is unavailable: {}",
                configured,
                loaded_render_plugins.join(", "),
                RENDER_SERVICE_ID,
                service_error
            )
        }
    }

    fn enable_null_render_backend<E: Send + 'static>(
        &mut self,
        ctx: &mut ModuleCtx<'_, E>,
        reason: impl Into<String>,
    ) -> EngineResult<()> {
        let reason = reason.into();

        log::warn!(
            "render backend: '{}' is unavailable; enabling headless null backend ({})",
            self.backend_spec,
            reason
        );

        let api = RenderApiRef::new(NullRenderApi::default());
        let resolved = ResolvedRenderBackendConfig {
            backend_id: NULL_RENDER_BACKEND_ID.to_owned(),
            clear_color: DEFAULT_RENDER_BACKEND_CLEAR_COLOR,
            debug_text: self.null_debug_text(),
        };

        ctx.resources_mut().insert(resolved);
        ctx.resources_mut().register_api(RENDER_API_ID, api.clone())?;
        self.api = Some(api);

        Ok(())
    }
}

impl<E: Send + 'static> Module<E> for RenderBackendRuntimeModule {
    fn id(&self) -> &'static str {
        "render.runtime.loader"
    }

    fn provides(&self) -> &'static [newengine_core::ApiProvide] {
        &[RENDER_API_PROVIDE]
    }

    fn init(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let host = newengine_plugin_host::default_host_api();
        let client = RenderServiceClient::new(host);

        let info = match client.info() {
            Ok(info) => info,
            Err(err) => {
                let reason = self.explain_backend_unavailability(ctx, &err);
                return self.enable_null_render_backend(ctx, reason);
            }
        };

        if !backend_matches(&self.backend_spec, &info.backend_id) {
            return self.enable_null_render_backend(
                ctx,
                format!(
                    "selected backend '{}' does not match active plugin '{}'",
                    self.backend_spec, info.backend_id
                ),
            );
        }

        log::info!(
            "render backend: bridge bound id='{}' name='{}' version='{}' debug_text='{}'",
            info.backend_id,
            info.backend_name,
            info.backend_version,
            info.debug_text
        );

        let resolved = ResolvedRenderBackendConfig {
            backend_id: info.backend_id,
            clear_color: info.clear_color,
            debug_text: info.debug_text,
        };

        let api = RenderApiRef::new(ServiceBackedRenderApi::new(client));

        ctx.resources_mut().insert(resolved);
        ctx.resources_mut().register_api(RENDER_API_ID, api.clone())?;
        self.api = Some(api);

        Ok(())
    }

    fn shutdown(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let _ = ctx.resources_mut().unregister_api::<RenderApiRef>(RENDER_API_ID);
        let _ = ctx.resources_mut().remove::<ResolvedRenderBackendConfig>();
        self.api = None;
        Ok(())
    }
}
