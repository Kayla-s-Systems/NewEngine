use abi_stable::std_types::{RResult, RString};
use abi_stable::StableAbi;

#[repr(C)]
#[derive(Debug, Clone, PartialEq, StableAbi)]
pub struct PlatformHostTaskRequestV1 {
    pub label: RString,
    pub source: RString,
    pub owner: RString,
    pub category: RString,
    pub lane: RString,
    pub priority: RString,
    pub task_id: RString,
    pub can_cancel: bool,
}

impl Default for PlatformHostTaskRequestV1 {
    #[inline]
    fn default() -> Self {
        Self {
            label: RString::from("platform.job"),
            source: RString::from("engine.platform"),
            owner: RString::from("platform-runtime"),
            category: RString::from("platform"),
            lane: RString::from("background"),
            priority: RString::from("normal"),
            task_id: RString::from(""),
            can_cancel: true,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, StableAbi)]
pub struct PlatformHostTaskTicketV1 {
    pub accepted: bool,
    pub job_id: RString,
    pub status: RString,
    pub detail: RString,
}

impl Default for PlatformHostTaskTicketV1 {
    #[inline]
    fn default() -> Self {
        Self {
            accepted: false,
            job_id: RString::from(""),
            status: RString::from("not-submitted"),
            detail: RString::from(""),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi, Default)]
pub struct PlatformHostJobCallbackV1 {
    pub callback_addr: usize,
}

impl PlatformHostJobCallbackV1 {
    #[inline]
    pub fn from_fn(callback: extern "C" fn(usize) -> RResult<(), RString>) -> Self {
        Self {
            callback_addr: callback as usize,
        }
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.callback_addr == 0
    }
}
