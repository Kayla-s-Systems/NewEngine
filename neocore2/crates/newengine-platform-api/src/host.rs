use abi_stable::std_types::{RResult, RString, RVec};
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

pub const PLATFORM_RUNTIME_DESCRIPTOR_V1_SYMBOL_NAME: &str =
    "newengine_platform_runtime_descriptor_v1";
pub const PLATFORM_RUNTIME_DESCRIPTOR_V1_SYMBOL_BYTES: &[u8] =
    b"newengine_platform_runtime_descriptor_v1";
pub const PLATFORM_RUNTIME_DESCRIPTOR_V1_SYMBOL_BYTES_NUL: &[u8] =
    b"newengine_platform_runtime_descriptor_v1\0";

/// Construction-free metadata for an external platform runtime.
///
/// The host may inspect this descriptor before invoking the runtime entrypoint.
/// Selection is data-driven: concrete window-system/provider names are not part
/// of the host policy contract.
#[repr(C)]
#[derive(Clone, Debug, StableAbi)]
pub struct PlatformRuntimeDescriptorV1 {
    pub schema_version: u32,
    pub id: RString,
    pub name: RString,
    pub version: RString,
    pub backend_priority: i32,
    pub system_tags: RVec<RString>,
}

impl PlatformRuntimeDescriptorV1 {
    pub const SCHEMA_VERSION: u32 = 1;

    #[inline]
    pub fn new(
        id: impl Into<RString>,
        name: impl Into<RString>,
        version: impl Into<RString>,
    ) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            id: id.into(),
            name: name.into(),
            version: version.into(),
            backend_priority: 0,
            system_tags: RVec::new(),
        }
    }

    #[inline]
    pub fn with_backend_priority(mut self, priority: i32) -> Self {
        self.backend_priority = priority;
        self
    }

    pub fn with_system_tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut tags = tags
            .into_iter()
            .map(|tag| tag.as_ref().trim().to_owned())
            .filter(|tag| !tag.is_empty())
            .collect::<Vec<_>>();
        tags.sort();
        tags.dedup();
        self.system_tags = tags.into_iter().map(RString::from).collect::<Vec<_>>().into();
        self
    }
}

pub type PlatformRuntimeDescriptorV1Fn = extern "C" fn() -> PlatformRuntimeDescriptorV1;
