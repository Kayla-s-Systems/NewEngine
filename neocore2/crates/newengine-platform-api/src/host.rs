use abi_stable::std_types::{RResult, RString};
use abi_stable::StableAbi;
use newengine_plugin_api::HostApiV1;

use crate::{
    PlatformAppConfigV1, PlatformCursorPollV1, PlatformHostJobCallbackV1,
    PlatformHostTaskRequestV1, PlatformHostTaskTicketV1, PlatformStepResultV1,
    PlatformSurfaceMetricsV1, PlatformWindowReadyV1,
};

#[repr(C)]
#[derive(Clone, StableAbi)]
pub struct PlatformHostApiV1 {
    pub user_data: usize,
    pub on_window_ready_v1: extern "C" fn(usize, PlatformWindowReadyV1) -> RResult<(), RString>,
    pub on_window_resized_v1:
        extern "C" fn(usize, PlatformSurfaceMetricsV1) -> RResult<(), RString>,
    pub on_window_focused_v1: extern "C" fn(usize, bool) -> RResult<(), RString>,
    pub on_close_requested_v1: extern "C" fn(usize) -> RResult<(), RString>,
    pub step_v1: extern "C" fn(usize, f32) -> RResult<PlatformStepResultV1, RString>,
    pub poll_cursor_state_v1: extern "C" fn(usize) -> PlatformCursorPollV1,
    pub submit_job_v1: extern "C" fn(
        usize,
        PlatformHostTaskRequestV1,
        PlatformHostJobCallbackV1,
        usize,
    ) -> PlatformHostTaskTicketV1,
}

pub type PlatformRunResultV1 = RResult<(), RString>;
pub type PlatformRuntimeRunFnV1 =
    unsafe extern "C" fn(HostApiV1, PlatformHostApiV1, PlatformAppConfigV1) -> PlatformRunResultV1;
