use abi_stable::std_types::{RResult, RString};
use newengine_platform_api::{
    PlatformCursorPollV1, PlatformStepResultV1, PlatformSurfaceMetricsV1,
    PlatformWindowReadyV1,
};

use crate::platform_runtime::runtime_host::HostPlatformRuntime;

#[inline]
fn runtime_state_mut<'a>(user_data: usize) -> &'a mut HostPlatformRuntime {
    unsafe { &mut *(user_data as *mut HostPlatformRuntime) }
}

pub(crate) extern "C" fn host_on_window_ready_v1(
    user_data: usize,
    ready: PlatformWindowReadyV1,
) -> RResult<(), RString> {
    match runtime_state_mut(user_data).on_window_ready(ready) {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

pub(crate) extern "C" fn host_on_window_resized_v1(
    user_data: usize,
    metrics: PlatformSurfaceMetricsV1,
) -> RResult<(), RString> {
    match runtime_state_mut(user_data).on_window_resized(metrics) {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

pub(crate) extern "C" fn host_on_window_focused_v1(
    user_data: usize,
    focused: bool,
) -> RResult<(), RString> {
    match runtime_state_mut(user_data).on_window_focused(focused) {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

pub(crate) extern "C" fn host_on_close_requested_v1(
    user_data: usize,
) -> RResult<(), RString> {
    match runtime_state_mut(user_data).on_close_requested() {
        Ok(()) => RResult::ROk(()),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

pub(crate) extern "C" fn host_step_v1(
    user_data: usize,
    dt_sec: f32,
) -> RResult<PlatformStepResultV1, RString> {
    match runtime_state_mut(user_data).step(dt_sec) {
        Ok(v) => RResult::ROk(v),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

pub(crate) extern "C" fn host_poll_cursor_state_v1(
    user_data: usize,
) -> PlatformCursorPollV1 {
    runtime_state_mut(user_data).poll_cursor_state()
}