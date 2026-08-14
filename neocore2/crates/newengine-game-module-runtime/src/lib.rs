#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::{EngineError, EngineReadinessKey, EngineResult, Module, ModuleCtx};
use newengine_game_module_api::{
    GameModuleDescriptorV1, GAME_MODULE_DESCRIBE_METHOD_V1, GAME_MODULE_SERVICE_ID,
};
use newengine_project_runtime::ProjectRuntimeContext;

#[derive(Clone, Debug, Default)]
pub struct GameModuleRuntimeState {
    pub required_module_id: Option<String>,
    pub ready: bool,
    pub descriptor: Option<GameModuleDescriptorV1>,
    pub last_error: Option<String>,
}

pub struct GameModuleContractModule;

impl GameModuleContractModule {
    #[inline]
    pub const fn new() -> Self {
        Self
    }

    fn validate_contract<E: Send + 'static>(&self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        let required_module_id = ctx
            .resources()
            .get::<ProjectRuntimeContext>()
            .and_then(|project| project.manifest.game_module.clone())
            .filter(|value| !value.trim().is_empty());
        let Some(required_module_id) = required_module_id else {
            ctx.resources_mut()
                .insert(GameModuleRuntimeState::default());
            return Ok(());
        };

        let payload = serde_json::to_vec(&serde_json::json!({
            "contract": newengine_game_module_api::GAME_MODULE_CONTRACT_V1,
            "requested_module_id": required_module_id,
        }))
        .map_err(|error| {
            EngineError::Other(format!("encode game-module describe request: {error}"))
        })?;

        let response = newengine_core::call_service_v1_optional(
            GAME_MODULE_SERVICE_ID,
            GAME_MODULE_DESCRIBE_METHOD_V1,
            &payload,
        )
        .map_err(|error| EngineError::Other(format!("game-module describe call failed: {error}")))?
        .ok_or_else(|| {
            EngineError::Other(format!(
                "project requires game_module '{}' but service '{}' is unavailable after EnginePluginsReady",
                required_module_id, GAME_MODULE_SERVICE_ID
            ))
        })?;

        let descriptor: GameModuleDescriptorV1 =
            serde_json::from_slice(&response).map_err(|error| {
                EngineError::Other(format!("decode game-module descriptor: {error}"))
            })?;
        if let Err(errors) = descriptor.validate() {
            return Err(EngineError::Other(format!(
                "game-module descriptor invalid: {}",
                errors.join("; ")
            )));
        }
        if descriptor.module_id != required_module_id {
            return Err(EngineError::Other(format!(
                "game-module identity mismatch project requires='{}' provider returned='{}'",
                required_module_id, descriptor.module_id
            )));
        }

        newengine_ulog_api::ulog::info!(
            "game-module contract ready id='{}' version='{}' providers={} capabilities={} required_services={}",
            descriptor.module_id,
            descriptor.version,
            descriptor.providers.len(),
            descriptor.capabilities.len(),
            descriptor.required_services.len(),
        );
        ctx.resources_mut().insert(GameModuleRuntimeState {
            required_module_id: Some(required_module_id),
            ready: true,
            descriptor: Some(descriptor),
            last_error: None,
        });
        Ok(())
    }
}

impl Default for GameModuleContractModule {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Send + 'static> Module<E> for GameModuleContractModule {
    fn id(&self) -> &'static str {
        "engine.game.module-contract"
    }

    fn startup_requires(&self) -> &'static [EngineReadinessKey] {
        const REQUIRES: &[EngineReadinessKey] = &[EngineReadinessKey::EnginePluginsReady];
        REQUIRES
    }

    fn start(&mut self, ctx: &mut ModuleCtx<'_, E>) -> EngineResult<()> {
        self.validate_contract(ctx)
    }
}
