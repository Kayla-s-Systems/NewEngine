#![forbid(unsafe_op_in_unsafe_fn)]

use crate::host_context::{ctx, ServiceEntry};
use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::{
    Blob, CapabilityId, EventSinkV1Dyn, HostApiV1, MethodName, ServiceV1Dyn,
};
use std::cell::Cell;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::Instant;

thread_local! {
    static IN_HOST_API_LOG: Cell<bool> = const { Cell::new(false) };
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

        if g.contains_key(&service_id) {
            return RResult::RErr(RString::from(format!(
                "service already registered: {}",
                service_id
            )));
        }

        g.insert(
            service_id.clone(),
            ServiceEntry {
                owner_plugin_id: owner,
                service: Arc::from(svc),
                describe_json: describe_json.clone(),
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

#[inline]
fn route_host_owned_service(requested_id: &str) -> Option<String> {
    crate::host_context::resolve_service_for_engine_gateway(requested_id)
}

pub extern "C" fn call_service_v1(
    cap_id: CapabilityId,
    method: MethodName,
    payload: Blob,
) -> RResult<Blob, RString> {
    let requested_id = cap_id.to_string();
    let id = route_host_owned_service(&requested_id).unwrap_or(requested_id.clone());
    let c = ctx();

    let (svc, owner) = {
        let g = match c.services.lock() {
            Ok(v) => v,
            Err(_) => return RResult::RErr(RString::from("services mutex poisoned")),
        };

        match g.get(&id) {
            Some(v) => (v.service.clone(), v.owner_plugin_id.clone()),
            None => {
                if id != requested_id {
                    return RResult::RErr(RString::from(format!(
                        "service not found: requested={requested_id} routed={id}"
                    )));
                }
                return RResult::RErr(RString::from(format!("service not found: {id}")));
            }
        }
    };

    let method_string = method.to_string();
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
                    "requested_id": requested_id.as_str(),
                    "method": method_string.as_str(),
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
                    "requested_id": requested_id.as_str(),
                    "method": method_string.as_str(),
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
                        "requested_id": requested_id.as_str(),
                        "method": method_string.as_str(),
                        "owner": owner.as_deref().unwrap_or("<host>"),
                        "payload_bytes": payload_len,
                        "output_bytes": b.len(),
                        "elapsed_ms": elapsed_ms
                    }
                );
            }
        }
        RResult::RErr(e) => {
            newengine_ulog_api::ulog::error_event!(
                "engine.services.call_failed",
                "Service call returned error",
                {
                    "service_id": id.as_str(),
                    "requested_id": requested_id.as_str(),
                    "method": method_string.as_str(),
                    "owner": owner.as_deref().unwrap_or("<host>"),
                    "payload_bytes": payload_len,
                    "elapsed_ms": elapsed_ms,
                    "error": e.to_string()
                }
            );
            debug_no_recurse(format_args!(
                "services: call err id='{}' method='{}' err='{}'",
                id, method, e
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
        call_service_v1: call_service_v1,

        emit_event_v1: host_emit_event_v1,
        subscribe_events_v1: host_subscribe_events_v1,
    }
}
