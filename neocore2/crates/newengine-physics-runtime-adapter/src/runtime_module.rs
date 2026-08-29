use std::path::PathBuf;

use newengine_core::physics::{PhysicsApiRef, PHYSICS_API_ID, PHYSICS_API_PROVIDE};
use newengine_core::{EngineResult, Module, ModuleCtx};

use crate::client::PhysicsServiceClient;
use crate::service_api::ServiceBackedPhysicsApi;
use crate::types::{ResolvedPhysicsBackendConfig, PHYSICS_BACKEND_SERVICE_SPEC};
use newengine_runtime_adapter_core::bind_backend_info;

pub struct PhysicsBackendRuntimeModule {
    _modules_dir: PathBuf,
    api: Option<PhysicsApiRef>,
}

impl PhysicsBackendRuntimeModule {
    #[inline]
    pub fn new(modules_dir: PathBuf) -> Self {
        Self {
            _modules_dir: modules_dir,
            api: None,
        }
    }
}

impl<E: Send + 'static> Module<E> for PhysicsBackendRuntimeModule {
    fn id(&self) -> &'static str {
        "physics.runtime.loader"
    }

    fn provides(&self) -> &'static [newengine_core::ApiProvide] {
        &[PHYSICS_API_PROVIDE]
    }

    fn init(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let host = newengine_plugin_host::default_host_api();
        let client = PhysicsServiceClient::new(host);

        let (info, selection) = match bind_backend_info(
            ctx,
            PHYSICS_BACKEND_SERVICE_SPEC,
            client.info(),
        ) {
            Ok(bound) => bound,
            Err(err) => {
                newengine_ulog_api::ulog::warn!(
                    "physics backend: unavailable; PhysicsApiRef not registered and physics steps will be skipped: {}",
                    err
                );
                return Ok(());
            }
        };
        let protocol_version = info.protocol_version;

        newengine_ulog_api::ulog::info!(
            "physics backend: service bridge bound id='{}' name='{}' version='{}' provider='{}' provider_state='{}' matched_by='{}' debug_text='{}' protocol=v{}.{}.{} features={}",
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
        );

        let resolved = ResolvedPhysicsBackendConfig {
            backend_id: info.backend_id,
            debug_text: info.debug_text,
            capabilities: info.capabilities,
        };

        let api = PhysicsApiRef::new(ServiceBackedPhysicsApi::new(client));

        ctx.resources_mut().insert(resolved);
        ctx.resources_mut()
            .register_api(PHYSICS_API_ID, api.clone())?;
        self.api = Some(api);

        Ok(())
    }

    fn shutdown(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let _ = ctx
            .resources_mut()
            .unregister_api::<PhysicsApiRef>(PHYSICS_API_ID);
        let _ = ctx.resources_mut().remove::<ResolvedPhysicsBackendConfig>();
        self.api = None;
        Ok(())
    }
}
