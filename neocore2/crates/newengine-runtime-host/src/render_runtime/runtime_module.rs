use std::path::PathBuf;

use newengine_core::render::{RenderApiRef, RENDER_API_ID, RENDER_API_PROVIDE};
use newengine_core::{EngineError, EngineResult, Module, ModuleCtx};
use newengine_render_api::RENDER_SERVICE_ID;

use crate::render_runtime::client::RenderServiceClient;
use crate::render_runtime::provider_resolver::{
    plugin_declares_render_backend, plugin_declares_service, RenderProviderResolver,
};
use crate::render_runtime::service_api::ServiceBackedRenderApi;
use crate::render_runtime::types::ResolvedRenderBackendConfig;

pub struct RenderBackendRuntimeModule {
    _modules_dir: PathBuf,
    api: Option<RenderApiRef>,
}

impl RenderBackendRuntimeModule {
    #[inline]
    pub fn new(modules_dir: PathBuf) -> Self {
        Self {
            _modules_dir: modules_dir,
            api: None,
        }
    }

    #[inline]
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

        let loaded_render_plugins: Vec<String> = snapshot
            .plugins
            .iter()
            .filter(|plugin| {
                plugin_declares_render_backend(plugin)
                    || plugin_declares_service(plugin, RENDER_SERVICE_ID)
            })
            .map(|plugin| format!("{}:{}", plugin.id, plugin.state))
            .collect();

        if loaded_render_plugins.is_empty() {
            format!(
                "no render backend plugin was loaded; service '{}' is unavailable: {}",
                RENDER_SERVICE_ID,
                service_error
            )
        } else {
            format!(
                "loaded render providers=[{}], but service '{}' is unavailable: {}",
                loaded_render_plugins.join(", "),
                RENDER_SERVICE_ID,
                service_error
            )
        }
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
                return Err(EngineError::Other(format!(
                    "render backend could not be bound through service '{}': {}",
                    RENDER_SERVICE_ID,
                    reason
                )));
            }
        };

        let snapshot = ctx.resources().get::<newengine_plugin_host::PluginsSnapshot>();
        let selection = RenderProviderResolver::resolve(snapshot, &info)
            .map_err(EngineError::other)?;
        let protocol_version = info.protocol_version;

        log::info!(
            "render backend: service bridge bound id='{}' name='{}' version='{}' provider='{}' provider_state='{}' matched_by='{}' debug_text='{}' protocol=v{}.{}.{} features={} upload_budget={}MB/frame",
            info.backend_id,
            info.backend_name,
            info.backend_version,
            selection.provider_plugin_id,
            selection.provider_state,
            selection.matched_by,
            info.debug_text,
            protocol_version.major,
            protocol_version.minor,
            protocol_version.patch,
            info.capabilities.features.len(),
            info.work_budget.max_upload_bytes_per_frame / (1024 * 1024)
        );

        let resolved = ResolvedRenderBackendConfig {
            backend_id: info.backend_id,
            clear_color: info.clear_color,
            debug_text: info.debug_text,
            capabilities: info.capabilities,
            work_budget: info.work_budget,
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
