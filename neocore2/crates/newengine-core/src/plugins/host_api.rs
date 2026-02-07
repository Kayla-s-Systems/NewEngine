#![forbid(unsafe_op_in_unsafe_fn)]

use crate::plugins::describe::is_asset_importer;
use crate::plugins::host_context::{ctx, ServiceEntry};
#[cfg(feature = "runtime")]
use crate::plugins::importer::try_auto_register_importer;
use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::{
    Blob, CapabilityId, EventSinkV1Dyn, HostApiV1, MethodName, ServiceV1Dyn,
};
use std::cell::Cell;
use std::sync::Arc;

pub(crate) struct ImporterLoadState {
    pub saw_importer: bool,
    pub staged: Vec<ServiceV1Dyn<'static>>,
}

thread_local! {
    static IMPORTER_LOAD_STATE: Cell<*mut ImporterLoadState> =
        const { Cell::new(std::ptr::null_mut()) };
}

pub(crate) fn with_importer_load_state<R>(
    state: &mut ImporterLoadState,
    f: impl FnOnce() -> R,
) -> R {
    IMPORTER_LOAD_STATE.with(|slot| {
        let prev = slot.replace(state as *mut _);
        let out = f();
        slot.set(prev);
        out
    })
}

extern "C" fn host_log_info(s: RString) {
    log::info!("{}", s);
}

extern "C" fn host_log_warn(s: RString) {
    log::warn!("{}", s);
}

extern "C" fn host_log_error(s: RString) {
    log::error!("{}", s);
}

pub(crate) fn host_register_service_impl(
    svc: ServiceV1Dyn<'static>,
    auto_register_importer: bool,
) -> RResult<(), RString> {
    let service_id = svc.id().to_string();
    let describe_json = svc.describe().to_string();
    let owner = crate::plugins::host_context::current_plugin_id();

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
        crate::plugins::host_context::bump_services_generation();
    }

    #[cfg(feature = "runtime")]
    {
        if auto_register_importer {
            try_auto_register_importer(&service_id, &describe_json);
        }
    }

    RResult::ROk(())
}

extern "C" fn host_register_service_v1_plain(svc: ServiceV1Dyn<'static>) -> RResult<(), RString> {
    host_register_service_impl(svc, false)
}

extern "C" fn host_register_service_v1_importers(
    svc: ServiceV1Dyn<'static>,
) -> RResult<(), RString> {
    IMPORTER_LOAD_STATE.with(|slot| {
        let p = slot.get();
        if p.is_null() {
            return RResult::RErr(RString::from("importer loader: host state is not set"));
        }

        let st = unsafe { &mut *p };

        let describe_json = svc.describe().to_string();
        if is_asset_importer(&describe_json) {
            st.saw_importer = true;
        }

        st.staged.push(svc);
        RResult::ROk(())
    })
}

pub(crate) extern "C" fn call_service_v1(
    cap_id: CapabilityId,
    method: MethodName,
    payload: Blob,
) -> RResult<Blob, RString> {
    let id = cap_id.to_string();
    let c = ctx();

    let svc = {
        let g = match c.services.lock() {
            Ok(v) => v,
            Err(_) => return RResult::RErr(RString::from("services mutex poisoned")),
        };

        match g.get(&id) {
            Some(v) => v.service.clone(),
            None => return RResult::RErr(RString::from(format!("service not found: {id}"))),
        }
    };

    svc.call(method, payload)
}

extern "C" fn host_emit_event_v1(topic: RString, payload: Blob) -> RResult<(), RString> {
    match crate::plugins::host_context::emit_plugin_event(topic, payload) {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

extern "C" fn host_subscribe_events_v1(sink: EventSinkV1Dyn<'static>) -> RResult<(), RString> {
    match crate::plugins::host_context::subscribe_event_sink(sink) {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

pub fn default_host_api() -> HostApiV1 {
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

pub fn importers_host_api() -> HostApiV1 {
    HostApiV1 {
        log_info: host_log_info,
        log_warn: host_log_warn,
        log_error: host_log_error,

        register_service_v1: host_register_service_v1_importers,
        call_service_v1: call_service_v1,

        emit_event_v1: host_emit_event_v1,
        subscribe_events_v1: host_subscribe_events_v1,
    }
}