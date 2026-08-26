#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_core::{EngineError, EngineReadinessKey, EngineResult, Module, ModuleCtx};
use newengine_game_module_api::{
    GameModuleDescriptorV1, GameModuleDescriptorV2, GAME_MODULE_CONTRACT_V1,
    GAME_MODULE_CONTRACT_V2, GAME_MODULE_DESCRIBE_METHOD_V1, GAME_MODULE_DESCRIBE_METHOD_V2,
    GAME_MODULE_SERVICE_ID,
};
use newengine_project_runtime::RuntimeCompositionContext;

#[derive(Clone, Debug, Default)]
pub struct GameModuleRuntimeState {
    pub required_module_id: Option<String>,
    pub ready: bool,
    pub descriptor: Option<GameModuleDescriptorV2>,
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
            .get::<RuntimeCompositionContext>()
            .and_then(|runtime| runtime.game_module.clone())
            .filter(|value| !value.trim().is_empty());
        let Some(required_module_id) = required_module_id else {
            ctx.resources_mut()
                .insert(GameModuleRuntimeState::default());
            return Ok(());
        };

        let payload_v2 = serde_json::to_vec(&serde_json::json!({
            "contract": GAME_MODULE_CONTRACT_V2,
            "requested_module_id": required_module_id,
        }))
        .map_err(|error| {
            EngineError::Other(format!("encode game-module V2 describe request: {error}"))
        })?;

        let supports_v2 = newengine_core::describe_service(GAME_MODULE_SERVICE_ID)
            .and_then(|description| serde_json::from_str::<serde_json::Value>(&description).ok())
            .and_then(|description| {
                description
                    .get("methods")
                    .and_then(|methods| methods.as_array())
                    .cloned()
            })
            .is_some_and(|methods| {
                methods
                    .iter()
                    .any(|method| method.as_str() == Some(GAME_MODULE_DESCRIBE_METHOD_V2))
            });

        let descriptor = if supports_v2 {
            let response = newengine_core::call_service_v1(
                GAME_MODULE_SERVICE_ID,
                GAME_MODULE_DESCRIBE_METHOD_V2,
                &payload_v2,
            )
            .map_err(|error| {
                EngineError::Other(format!("game-module V2 describe call failed: {error}"))
            })?;
            let descriptor: GameModuleDescriptorV2 =
                serde_json::from_slice(&response).map_err(|error| {
                    EngineError::Other(format!("decode game-module V2 descriptor: {error}"))
                })?;
            descriptor.validate().map_err(|errors| {
                EngineError::Other(format!(
                    "game-module V2 descriptor invalid: {}",
                    errors.join("; ")
                ))
            })?;
            descriptor
        } else {
            // Migration-only fallback for legacy third-party/old first-party services.
            let payload_v1 = serde_json::to_vec(&serde_json::json!({
                "contract": GAME_MODULE_CONTRACT_V1,
                "requested_module_id": required_module_id,
            }))
            .map_err(|error| {
                EngineError::Other(format!("encode game-module V1 describe request: {error}"))
            })?;
            let response = newengine_core::call_service_v1_optional(
                GAME_MODULE_SERVICE_ID,
                GAME_MODULE_DESCRIBE_METHOD_V1,
                &payload_v1,
            )
            .map_err(|error| EngineError::Other(format!("game-module V1 describe call failed: {error}")))?
            .ok_or_else(|| {
                EngineError::Other(format!(
                    "game runtime requires game_module '{}' but service '{}' exposes neither V2 nor V1 descriptor method after EnginePluginsReady",
                    required_module_id, GAME_MODULE_SERVICE_ID
                ))
            })?;
            let legacy: GameModuleDescriptorV1 =
                serde_json::from_slice(&response).map_err(|error| {
                    EngineError::Other(format!("decode game-module V1 descriptor: {error}"))
                })?;
            GameModuleDescriptorV2::from_v1(legacy).map_err(|errors| {
                EngineError::Other(format!(
                    "normalize game-module V1 descriptor to V2: {}",
                    errors.join("; ")
                ))
            })?
        };
        if descriptor.module_id != required_module_id {
            return Err(EngineError::Other(format!(
                "game-module identity mismatch runtime requires='{}' provider returned='{}'",
                required_module_id, descriptor.module_id
            )));
        }

        newengine_ulog_api::ulog::info!(
            "game-module contract ready id='{}' version='{}' requirements={} providers={} capabilities={} required_services={}",
            descriptor.module_id,
            descriptor.version,
            descriptor.requirements.len(),
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
