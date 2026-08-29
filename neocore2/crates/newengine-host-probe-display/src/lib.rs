#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_host_capabilities_api::DisplayCapabilities;

#[cfg(windows)]
pub fn discover() -> Vec<DisplayCapabilities> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CMONITORS, SM_CXSCREEN, SM_CYSCREEN,
    };
    let count = unsafe { GetSystemMetrics(SM_CMONITORS) }.max(1) as u32;
    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) }.max(0) as u32;
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) }.max(0) as u32;
    (0..count)
        .map(|index| DisplayCapabilities {
            index,
            name: (index == 0).then(|| "primary".to_owned()),
            primary: index == 0,
            width_px: (index == 0 && width > 0).then_some(width),
            height_px: (index == 0 && height > 0).then_some(height),
            refresh_rate_millihz: None,
            hdr_capable: None,
        })
        .collect()
}

#[cfg(not(windows))]
pub fn discover() -> Vec<DisplayCapabilities> {
    Vec::new()
}
