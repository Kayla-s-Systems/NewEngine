#![forbid(unsafe_op_in_unsafe_fn)]

use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

/// Engine-thread local window handles.
#[derive(Debug, Clone, Copy)]
pub struct WinitWindowHandles {
    pub window: RawWindowHandle,
    pub display: RawDisplayHandle,
}

/// Initial window size snapshot.
#[derive(Debug, Clone, Copy)]
pub struct WinitWindowInitSize {
    pub width: u32,
    pub height: u32,
}