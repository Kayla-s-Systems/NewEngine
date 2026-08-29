use abi_stable::std_types::{RResult, RString};
use newengine_platform_api::{
    PlatformCursorPollV1, PlatformHostJobCallbackV1, PlatformHostTaskRequestV1,
    PlatformHostTaskTicketV1, PlatformStepResultV1, PlatformSurfaceMetricsV1,
    PlatformWindowReadyV1,
};
use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::platform_runtime::runtime_host::HostPlatformRuntime;

#[inline]
fn runtime_state_mut<'a>(user_data: usize) -> Option<&'a mut HostPlatformRuntime> {
    if user_data == 0 {
        crate::platform_early_log!("host.callback.invalid_user_data null");
        return None;
    }
    Some(unsafe { &mut *(user_data as *mut HostPlatformRuntime) })
}

pub(crate) extern "C" fn host_on_window_ready_v1(
    user_data: usize,
    ready: PlatformWindowReadyV1,
) -> RResult<(), RString> {
    crate::platform_early_log!(
        "host.callback.on_window_ready.begin user_data=0x{:x}",
        user_data
    );
    let Some(runtime) = runtime_state_mut(user_data) else {
        return RResult::RErr(RString::from(
            "host.on_window_ready_v1 received null user_data",
        ));
    };
    match runtime.on_window_ready(ready) {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

pub(crate) extern "C" fn host_on_window_resized_v1(
    user_data: usize,
    metrics: PlatformSurfaceMetricsV1,
) -> RResult<(), RString> {
    crate::platform_early_log!(
        "host.callback.on_window_resized.begin user_data=0x{:x}",
        user_data
    );
    let Some(runtime) = runtime_state_mut(user_data) else {
        return RResult::RErr(RString::from(
            "host.on_window_resized_v1 received null user_data",
        ));
    };
    match runtime.on_window_resized(metrics) {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

pub(crate) extern "C" fn host_on_window_focused_v1(
    user_data: usize,
    focused: bool,
) -> RResult<(), RString> {
    crate::platform_early_log!(
        "host.callback.on_window_focused.begin user_data=0x{:x} focused={}",
        user_data,
        focused
    );
    let Some(runtime) = runtime_state_mut(user_data) else {
        return RResult::RErr(RString::from(
            "host.on_window_focused_v1 received null user_data",
        ));
    };
    match runtime.on_window_focused(focused) {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

pub(crate) extern "C" fn host_on_close_requested_v1(user_data: usize) -> RResult<(), RString> {
    crate::platform_early_log!(
        "host.callback.on_close_requested.begin user_data=0x{:x}",
        user_data
    );
    let Some(runtime) = runtime_state_mut(user_data) else {
        return RResult::RErr(RString::from(
            "host.on_close_requested_v1 received null user_data",
        ));
    };
    match runtime.on_close_requested() {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

pub(crate) extern "C" fn host_submit_job_v1(
    user_data: usize,
    request: PlatformHostTaskRequestV1,
    callback: PlatformHostJobCallbackV1,
    callback_user_data: usize,
) -> PlatformHostTaskTicketV1 {
    let Some(runtime) = runtime_state_mut(user_data) else {
        return PlatformHostTaskTicketV1 {
            accepted: false,
            status: RString::from("rejected"),
            detail: RString::from("host.submit_job_v1 received null user_data"),
            ..Default::default()
        };
    };
    runtime.submit_platform_job(request, callback, callback_user_data)
}

pub(crate) extern "C" fn host_step_v1(
    user_data: usize,
    dt_sec: f32,
) -> RResult<PlatformStepResultV1, RString> {
    let Some(runtime) = runtime_state_mut(user_data) else {
        return RResult::RErr(RString::from("host.step_v1 received null user_data"));
    };

    // Platform callbacks are an FFI boundary. A Rust panic escaping an `extern "C"`
    // callback is converted by Windows into STATUS_FATAL_USER_CALLBACK_EXCEPTION
    // (0xC000041D), killing the process before the engine can publish diagnostics.
    // Keep the original step path intact, but convert callback panics/errors into
    // a visible runtime recovery state instead of process death.
    match catch_unwind(AssertUnwindSafe(|| runtime.step(dt_sec))) {
        Ok(Ok(v)) => RResult::ROk(v),
        Ok(Err(e)) => {
            let message = e.to_string();
            newengine_ulog_api::ulog::error!("platform runtime: host.step_v1 returned engine error; entering soft degradation: {message}");
            RResult::ROk(runtime.enter_runtime_soft_degraded_step("host.step_v1", message))
        }
        Err(payload) => {
            let message = panic_payload_message(payload);
            newengine_ulog_api::ulog::error!("platform runtime: host.step_v1 panic caught at FFI boundary; entering soft degradation: {message}");
            RResult::ROk(runtime.enter_runtime_soft_degraded_step("host.step_v1.panic", message))
        }
    }
}

pub(crate) extern "C" fn host_poll_cursor_state_v1(user_data: usize) -> PlatformCursorPollV1 {
    let Some(runtime) = runtime_state_mut(user_data) else {
        return PlatformCursorPollV1::default();
    };
    match catch_unwind(AssertUnwindSafe(|| runtime.poll_cursor_state())) {
        Ok(poll) => poll,
        Err(payload) => {
            newengine_ulog_api::ulog::error!(
                "platform runtime: host.poll_cursor_state_v1 panic caught at FFI boundary: {}",
                panic_payload_message(payload)
            );
            PlatformCursorPollV1::default()
        }
    }
}

fn panic_payload_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(msg) = payload.downcast_ref::<&str>() {
        (*msg).to_owned()
    } else if let Some(msg) = payload.downcast_ref::<String>() {
        msg.clone()
    } else {
        "<non-string panic payload>".to_owned()
    }
}
