use super::gateway::{active_engine_gateways, resolve_service_for_engine_gateway};
use super::state::ctx;

/// Returns true if a plugin-owned service with the given id is currently registered.
#[inline]
pub fn has_service(service_id: &str) -> bool {
    let direct_registered = {
        let c = ctx();
        let g = match c.services.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        };
        g.contains_key(service_id)
    };

    if direct_registered {
        return true;
    }

    resolve_service_for_engine_gateway(service_id).is_some()
}

/// Returns a stable, sorted list of registered plugin-owned service ids.
///
/// Intended for diagnostics and crash reports.
pub fn list_services() -> Vec<String> {
    let c = ctx();
    let g = match c.services.lock() {
        Ok(v) => v,
        Err(e) => e.into_inner(),
    };

    let mut out: Vec<String> = g.keys().cloned().collect();
    drop(g);

    out.extend(active_engine_gateways());
    out.sort();
    out.dedup();
    out
}

/// Returns the `describe()` JSON for the given service id, if present.
#[inline]
pub fn describe_service(service_id: &str) -> Option<String> {
    let routed_id =
        resolve_service_for_engine_gateway(service_id).unwrap_or_else(|| service_id.to_owned());

    let c = ctx();
    let g = c.services.lock().ok()?;
    Some(g.get(&routed_id)?.describe_json.clone())
}
