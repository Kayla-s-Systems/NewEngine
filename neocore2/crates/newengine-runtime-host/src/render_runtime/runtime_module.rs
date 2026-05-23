use std::path::PathBuf;

use newengine_core::render::{RenderApiRef, RENDER_API_ID, RENDER_API_PROVIDE};
use newengine_core::{EngineResult, Module, ModuleCtx};

use crate::render_runtime::client::RenderServiceClient;
use crate::render_runtime::service_api::ServiceBackedRenderApi;
use crate::render_runtime::types::{ResolvedRenderBackendConfig, RENDER_BACKEND_SERVICE_SPEC};
use crate::service_runtime::bind_backend_info;

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

        let (info, selection) = match bind_backend_info(
            ctx,
            RENDER_BACKEND_SERVICE_SPEC,
            client.info(),
        ) {
            Ok(bound) => bound,
            Err(err) => {
                log::warn!(
                    "render backend: unavailable; render API not registered and runtime will degrade without rendering: {}",
                    err
                );
                return Ok(());
            }
        };
        let protocol_version = info.protocol_version;

        log::info!(
            "render backend: service bridge bound id='{}' name='{}' version='{}' provider='{}' provider_state='{}' matched_by='{}' debug_text='{}' protocol=v{}.{}.{} features={} hardware_tier={:?} upload_budget={}MB/frame",
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
            info.capabilities.hardware_tier,
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
