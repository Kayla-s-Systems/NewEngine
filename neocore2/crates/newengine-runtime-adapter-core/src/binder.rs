use newengine_core::{EngineError, EngineResult, ModuleCtx};
use newengine_service_api::BackendServiceSpec;

use super::{
    resolver::{explain_backend_unavailability, resolve_backend_provider},
    BackendSelection,
};

/// Common host-side bind step for a domain backend service.
///
/// The domain supplies the already-decoded `info_json` result. The framework handles
/// uniform error reporting and validates the active provider through
/// the host’s immutable `CompositionPlan`; backend ids in info packets are diagnostic only and must
/// not participate in provider selection.
pub fn bind_backend_info<E, I>(
    ctx: &ModuleCtx<'_, E>,
    spec: BackendServiceSpec,
    info_result: Result<I, String>,
) -> EngineResult<(I, BackendSelection)>
where
    E: Send + 'static,
{
    let info = match info_result {
        Ok(info) => info,
        Err(err) => {
            let reason = explain_backend_unavailability(ctx, spec, &err);
            return Err(EngineError::Other(format!(
                "{} backend could not be bound through service '{}': {}",
                spec.domain, spec.engine_gateway_id, reason
            )));
        }
    };

    let snapshot = ctx
        .resources()
        .get::<newengine_plugin_host::PluginsSnapshot>();
    let selection = resolve_backend_provider(snapshot, spec).map_err(EngineError::other)?;
    Ok((info, selection))
}
