#![forbid(unsafe_op_in_unsafe_fn)]

use crate::host_context::{ctx, ServiceEntry};
use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::{
    Blob, CapabilityId, EventSinkV1Dyn, HostApiV1, MethodName, ServiceV1Dyn,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
struct CachedServiceDispatch {
    routed_id: String,
    service: Arc<ServiceV1Dyn<'static>>,
    owner_plugin_id: Option<String>,
}

#[derive(Default)]
struct ServiceDispatchCache {
    generation: u64,
    entries: HashMap<String, CachedServiceDispatch>,
}

thread_local! {
    static IN_HOST_API_LOG: Cell<bool> = const { Cell::new(false) };
    static SERVICE_DISPATCH_CACHE: RefCell<ServiceDispatchCache> = RefCell::new(ServiceDispatchCache::default());
}

#[inline]
fn debug_no_recurse(args: std::fmt::Arguments<'_>) {
    if !newengine_ulog_api::ulog::debug_enabled() {
        return;
    }

    IN_HOST_API_LOG.with(|f| {
        if f.get() {
            return;
        }
        f.set(true);
        newengine_ulog_api::ulog::debug!("{}", args);
        f.set(false);
    });
}

#[inline]
fn service_error_is_expected_rejection(error: &str) -> bool {
    let error = error.trim().to_ascii_lowercase();
    [
        "unavailable",
        "not supported",
        "unsupported",
        "disabled by configuration",
        "disabled by config",
        "capability is disabled",
    ]
    .into_iter()
    .any(|marker| error.contains(marker))
}

extern "C" fn host_log_info(s: RString) {
    newengine_ulog_api::ulog::info!("{}", s);
}

extern "C" fn host_log_warn(s: RString) {
    newengine_ulog_api::ulog::warn!("{}", s);
}

extern "C" fn host_log_error(s: RString) {
    newengine_ulog_api::ulog::error!("{}", s);
}

pub fn host_register_service_impl(svc: ServiceV1Dyn<'static>) -> RResult<(), RString> {
    let service_id = svc.id().to_string();
    let describe_json = svc.describe().to_string();
    let owner = crate::host_context::current_plugin_id();
    let owner_for_log = owner.as_deref().unwrap_or("<none>").to_string();

    // Enforce declared capabilities for ABI v2/v3 plugins.
    // ABI v1 plugins have no descriptor -> best-effort allow with warning.
    if let Some(pid) = owner.as_deref() {
        match crate::host_context::plugin_declares_provided_service(pid, &service_id) {
            Some(true) => {}
            Some(false) => {
                return RResult::RErr(RString::from(format!(
                    "service not declared in descriptor: plugin='{}' service='{}'",
                    pid, service_id
                )));
            }
            None => {
                newengine_ulog_api::ulog::warn_event!(
                    "engine.services.descriptor_missing",
                    "Plugin has no descriptor; skipping capability validation",
                    {
                        "plugin_id": pid,
                        "service_id": service_id.as_str()
                    }
                );
            }
        }
    }

    let c = ctx();

    {
        let mut g = match c.services.lock() {
            Ok(v) => v,
            Err(_) => return RResult::RErr(RString::from("services mutex poisoned")),
        };

        if let Some(existing) = g.get(&service_id) {
            return RResult::RErr(RString::from(format!(
                "service already registered: {} owner='{}' contender='{}'",
                service_id,
                existing.owner_plugin_id.as_deref().unwrap_or("<none>"),
                owner_for_log,
            )));
        }

        g.insert(
            service_id.clone(),
            ServiceEntry {
                owner_plugin_id: owner,
                service: Arc::from(svc),
            },
        );
        crate::host_context::bump_services_generation();
    }

    debug_no_recurse(format_args!(
        "services: registered id='{}' owner='{}' describe_len={}",
        service_id,
        owner_for_log,
        describe_json.len()
    ));

    RResult::ROk(())
}

extern "C" fn host_register_service_v1_plain(svc: ServiceV1Dyn<'static>) -> RResult<(), RString> {
    host_register_service_impl(svc)
}

fn resolve_service_dispatch(requested_id: &str) -> Result<CachedServiceDispatch, RString> {
    let generation = crate::host_context::services_generation();

    if let Some(cached) = SERVICE_DISPATCH_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.generation != generation {
            cache.generation = generation;
            cache.entries.clear();
        }
        cache.entries.get(requested_id).cloned()
    }) {
        return Ok(cached);
    }

    let routed_id = crate::host_context::resolve_service_for_engine_gateway(requested_id)
        .unwrap_or_else(|| requested_id.to_owned());
    let c = ctx();
    let entry = {
        let services = c
            .services
            .lock()
            .map_err(|_| RString::from("services mutex poisoned"))?;
        services.get(&routed_id).cloned()
    };
    let Some(entry) = entry else {
        let message = if routed_id != requested_id {
            format!("service not found: requested={requested_id} routed={routed_id}")
        } else {
            format!("service not found: {routed_id}")
        };
        return Err(RString::from(message));
    };

    let dispatch = CachedServiceDispatch {
        routed_id,
        service: entry.service,
        owner_plugin_id: entry.owner_plugin_id,
    };
    SERVICE_DISPATCH_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        // A provider may have changed while the slow lookup was in progress. In
        // that case do not publish the stale route into the fast-path cache.
        if cache.generation == generation
            && crate::host_context::services_generation() == generation
        {
            cache
                .entries
                .insert(requested_id.to_owned(), dispatch.clone());
        }
    });
    Ok(dispatch)
}

pub extern "C" fn call_service_v1(
    cap_id: CapabilityId,
    method: MethodName,
    payload: Blob,
) -> RResult<Blob, RString> {
    let requested_id = cap_id.as_str();
    let dispatch = match resolve_service_dispatch(requested_id) {
        Ok(dispatch) => dispatch,
        Err(error) => return RResult::RErr(error),
    };
    let id = dispatch.routed_id;
    let svc = dispatch.service;
    let owner = dispatch.owner_plugin_id;

    let method_name = method.as_str();
    let payload_len = payload.len();
    let started = Instant::now();

    /*
    debug_no_recurse(format_args!(
        "services: call id='{}' method='{}' payload_len={}",
        id,
        method,
        payload_len
    )); */

    let do_call = || svc.call(method.clone(), payload);

    let res = match owner.as_deref() {
        Some(pid) => catch_unwind(AssertUnwindSafe(|| {
            crate::host_context::with_current_plugin_id(pid, do_call)
        }))
        .unwrap_or_else(|_| {
            newengine_ulog_api::ulog::error_event!(
                "engine.services.call_panicked",
                "Service call panicked; auto-unregistering owner",
                {
                    "service_id": id.as_str(),
                    "requested_id": requested_id,
                    "method": method_name,
                    "owner": pid,
                    "auto_unregister": true
                }
            );
            crate::host_context::unregister_by_owner(pid);
            RResult::RErr(RString::from("service panicked"))
        }),
        None => catch_unwind(AssertUnwindSafe(do_call)).unwrap_or_else(|_| {
            newengine_ulog_api::ulog::error_event!(
                "engine.services.call_panicked",
                "Host-owned service call panicked",
                {
                    "service_id": id.as_str(),
                    "requested_id": requested_id,
                    "method": method_name,
                    "owner": "<host>",
                    "auto_unregister": false
                }
            );
            RResult::RErr(RString::from("service panicked"))
        }),
    };

    let elapsed_ms = crate::diagnostics::elapsed_ms(started);
    match &res {
        RResult::ROk(b) => {
            if elapsed_ms >= 8.0 {
                newengine_ulog_api::ulog::debug_event!(
                    "engine.services.call_slow",
                    "Slow service call",
                    {
                        "service_id": id.as_str(),
                        "requested_id": requested_id,
                        "method": method_name,
                        "owner": owner.as_deref().unwrap_or("<host>"),
                        "payload_bytes": payload_len,
                        "output_bytes": b.len(),
                        "elapsed_ms": elapsed_ms
                    }
                );
            }
        }
        RResult::RErr(e) => {
            let error = e.to_string();
            if service_error_is_expected_rejection(&error) {
                newengine_ulog_api::ulog::warn_event!(
                    "engine.services.call_rejected",
                    "Service call rejected by provider",
                    {
                        "service_id": id.as_str(),
                        "requested_id": requested_id,
                        "method": method_name,
                        "owner": owner.as_deref().unwrap_or("<host>"),
                        "payload_bytes": payload_len,
                        "elapsed_ms": elapsed_ms,
                        "error": error.as_str(),
                        "expected_fallback": true
                    }
                );
            } else {
                newengine_ulog_api::ulog::error_event!(
                    "engine.services.call_failed",
                    "Service call returned error",
                    {
                        "service_id": id.as_str(),
                        "requested_id": requested_id,
                        "method": method_name,
                        "owner": owner.as_deref().unwrap_or("<host>"),
                        "payload_bytes": payload_len,
                        "elapsed_ms": elapsed_ms,
                        "error": error.as_str()
                    }
                );
            }
            debug_no_recurse(format_args!(
                "services: call err id='{}' method='{}' err='{}'",
                id, method, error
            ));
        }
    }

    res
}

extern "C" fn host_emit_event_v1(topic: RString, payload: Blob) -> RResult<(), RString> {
    match crate::host_context::emit_plugin_event(topic, payload) {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

extern "C" fn host_subscribe_events_v1(sink: EventSinkV1Dyn<'static>) -> RResult<(), RString> {
    debug_no_recurse(format_args!("events: subscribe"));

    match crate::host_context::subscribe_event_sink(sink) {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

pub fn default_host_api() -> HostApiV1 {
    crate::ulog_event::install_structured_ulog_sink_once();
    HostApiV1 {
        log_info: host_log_info,
        log_warn: host_log_warn,
        log_error: host_log_error,

        register_service_v1: host_register_service_v1_plain,
        call_service_v1,

        emit_event_v1: host_emit_event_v1,
        subscribe_events_v1: host_subscribe_events_v1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_optional_backend_feature_is_expected_rejection() {
        assert!(service_error_is_expected_rejection(
            "independent multi-adapter mesh workers are unavailable"
        ));
        assert!(service_error_is_expected_rejection(
            "feature not supported by the selected adapter"
        ));
        assert!(service_error_is_expected_rejection(
            "capability is disabled"
        ));
    }

    #[test]
    fn malformed_requests_and_panics_remain_failures() {
        assert!(!service_error_is_expected_rejection("invalid payload"));
        assert!(!service_error_is_expected_rejection("service panicked"));
        assert!(!service_error_is_expected_rejection("unknown method"));
        assert!(!service_error_is_expected_rejection("unknown service"));
    }
}
