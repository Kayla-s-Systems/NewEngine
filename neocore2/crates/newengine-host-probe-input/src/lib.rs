#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_host_capabilities_api::InputCapabilities;

#[cfg(windows)]
pub fn discover() -> InputCapabilities {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CMOUSEBUTTONS, SM_MOUSEPRESENT, SM_MOUSEWHEELPRESENT,
    };
    let mouse_present = unsafe { GetSystemMetrics(SM_MOUSEPRESENT) } != 0;
    let mouse_buttons = unsafe { GetSystemMetrics(SM_CMOUSEBUTTONS) }.max(0) as u32;
    let mouse_wheel = unsafe { GetSystemMetrics(SM_MOUSEWHEELPRESENT) } != 0;
    InputCapabilities {
        keyboard_present: Some(true),
        mouse_present: Some(mouse_present),
        mouse_buttons: Some(mouse_buttons),
        mouse_wheel_present: Some(mouse_wheel),
        touch_present: None,
    }
}

#[cfg(not(windows))]
pub fn discover() -> InputCapabilities {
    InputCapabilities {
        keyboard_present: None,
        mouse_present: None,
        mouse_buttons: None,
        mouse_wheel_present: None,
        touch_present: None,
    }
}
