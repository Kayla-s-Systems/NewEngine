use abi_stable::StableAbi;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, StableAbi)]
pub enum PlatformCursorGrabModeV1 {
    None = 0,
    Confined = 1,
    Locked = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, StableAbi)]
pub struct PlatformCursorStateV1 {
    pub visible: bool,
    pub grab: PlatformCursorGrabModeV1,
}

impl Default for PlatformCursorStateV1 {
    #[inline]
    fn default() -> Self {
        Self {
            visible: true,
            grab: PlatformCursorGrabModeV1::None,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, StableAbi)]
pub struct PlatformCursorPollV1 {
    pub has_value: bool,
    pub state: PlatformCursorStateV1,
}

impl Default for PlatformCursorPollV1 {
    #[inline]
    fn default() -> Self {
        Self {
            has_value: false,
            state: PlatformCursorStateV1::default(),
        }
    }
}
