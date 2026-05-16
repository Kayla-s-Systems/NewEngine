use std::path::PathBuf;

use newengine_core::physics::{PhysicsApiRef, PHYSICS_API_ID, PHYSICS_API_PROVIDE};
use newengine_core::{EngineError, EngineResult, Module, ModuleCtx};
use newengine_physics_api::PHYSICS_SERVICE_ID;

use crate::physics_runtime::client::PhysicsServiceClient;
use crate::physics_runtime::provider_resolver::{
    plugin_declares_physics_backend, plugin_declares_service, PhysicsProviderResolver,
};
use crate::physics_runtime::service_api::ServiceBackedPhysicsApi;
use crate::physics_runtime::types::ResolvedPhysicsBackendConfig;

pub struct PhysicsBackendRuntimeModule {
    _modules_dir: PathBuf,
    api: Option<PhysicsApiRef>,
}

impl PhysicsBackendRuntimeModule {
    #[inline]
    pub fn new(modules_dir: PathBuf) -> Self {
        Self { _modules_dir: modules_dir, api: None }
    }

    #[inline]
    fn explain_backend_unavailability<E: Send + 'static>(
        &self,
        ctx: &ModuleCtx<'_, E>,
        service_error: &str,
    ) -> String {
        let Some(snapshot) = ctx.resources().get::<newengine_plugin_host::PluginsSnapshot>() else {
            return format!(
                "physics service '{}' is unavailable: {}",
                PHYSICS_SERVICE_ID, service_error
            );
        };

        let loaded_physics_plugins: Vec<String> = snapshot
            .plugins
            .iter()
            .filter(|plugin| {
                plugin_declares_physics_backend(plugin)
                    || plugin_declares_service(plugin, PHYSICS_SERVICE_ID)
            })
            .map(|plugin| format!("{}:{}", plugin.id, plugin.state))
            .collect();

        if loaded_physics_plugins.is_empty() {
            format!(
                "no physics backend plugin was loaded; service '{}' is unavailable: {}",
                PHYSICS_SERVICE_ID, service_error
            )
        } else {
            format!(
                "loaded physics providers=[{}], but service '{}' is unavailable: {}",
                loaded_physics_plugins.join(", "),
                PHYSICS_SERVICE_ID,
                service_error
            )
        }
    }
}

impl<E: Send + 'static> Module<E> for PhysicsBackendRuntimeModule {
    fn id(&self) -> &'static str { "physics.runtime.loader" }

    fn provides(&self) -> &'static [newengine_core::ApiProvide] { &[PHYSICS_API_PROVIDE] }

    fn init(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let host = newengine_plugin_host::default_host_api();
        let client = PhysicsServiceClient::new(host);

        let info = match client.info() {
            Ok(info) => info,
            Err(err) => {
                let reason = self.explain_backend_unavailability(ctx, &err);
                return Err(EngineError::Other(format!(
                    "physics backend could not be bound through service '{}': {}",
                    PHYSICS_SERVICE_ID,
                    reason
                )));
            }
        };

        let snapshot = ctx.resources().get::<newengine_plugin_host::PluginsSnapshot>();
        let selection = PhysicsProviderResolver::resolve(snapshot, &info).map_err(EngineError::other)?;
        let protocol_version = info.protocol_version;

        log::info!(
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
        ctx.resources_mut().register_api(PHYSICS_API_ID, api.clone())?;
        self.api = Some(api);

        Ok(())
    }

    fn shutdown(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let _ = ctx.resources_mut().unregister_api::<PhysicsApiRef>(PHYSICS_API_ID);
        let _ = ctx.resources_mut().remove::<ResolvedPhysicsBackendConfig>();
        self.api = None;
        Ok(())
    }
}
