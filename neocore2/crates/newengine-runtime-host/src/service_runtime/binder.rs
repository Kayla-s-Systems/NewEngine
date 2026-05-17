use newengine_core::{EngineError, EngineResult, ModuleCtx};
use newengine_service_api::BackendServiceSpec;

use super::{
    resolver::{explain_backend_unavailability, resolve_backend_provider},
    BackendSelection,
};

/// Common host-side bind step for a domain backend service.
///
/// The domain supplies the already-decoded `info_json` result and a way to read
/// the backend id from that info packet. The framework handles uniform error
/// reporting and validates that the active service is backed by a plugin that
/// declares both the domain service id and backend capability.
pub(crate) fn bind_backend_info<E, I, F>(
    ctx: &ModuleCtx<'_, E>,
    spec: BackendServiceSpec,
    info_result: Result<I, String>,
    backend_id: F,
) -> EngineResult<(I, BackendSelection)>
where
    E: Send + 'static,
    F: FnOnce(&I) -> &str,
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

    let snapshot = ctx.resources().get::<newengine_plugin_host::PluginsSnapshot>();
    let selection = resolve_backend_provider(snapshot, spec, backend_id(&info))
        .map_err(EngineError::other)?;
    Ok((info, selection))
}
