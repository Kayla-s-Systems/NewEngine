use std::num::NonZeroIsize;

use newengine_core::{EngineError, EngineResult};
use newengine_platform_api::{NativeWindowBackendV1, NativeWindowHandlesV1};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

#[cfg(target_os = "windows")]
pub(crate) fn raw_to_native_handles(
    window: RawWindowHandle,
    display: RawDisplayHandle,
) -> EngineResult<NativeWindowHandlesV1> {
    use raw_window_handle::{RawDisplayHandle::Windows, RawWindowHandle::Win32};

    let window = match window {
        Win32(win) => win,
        _ => {
            return Err(EngineError::other(
                "platform runtime raw handle conversion is only implemented for Win32",
            ))
        }
    };

    match display {
        Windows(_) => {}
        _ => {
            return Err(EngineError::other(
                "platform runtime raw display conversion is only implemented for Windows",
            ))
        }
    }

    Ok(NativeWindowHandlesV1 {
        backend: NativeWindowBackendV1::Win32,
        window: window.hwnd.get() as u64,
        display: window.hinstance.map(|v| v.get() as u64).unwrap_or_default(),
        reserved0: 0,
        reserved1: 0,
    })
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn raw_to_native_handles(
    _window: RawWindowHandle,
    _display: RawDisplayHandle,
) -> EngineResult<NativeWindowHandlesV1> {
    Err(EngineError::other(
        "platform runtime raw handle conversion is only implemented for Windows",
    ))
}

#[cfg(target_os = "windows")]
pub(crate) fn native_to_raw_handles(
    handles: NativeWindowHandlesV1,
) -> EngineResult<(RawDisplayHandle, RawWindowHandle)> {
    use raw_window_handle::{Win32WindowHandle, WindowsDisplayHandle};

    if handles.backend != NativeWindowBackendV1::Win32 {
        return Err(EngineError::other(format!(
            "unsupported native window backend: {:?}",
            handles.backend
        )));
    }

    let hwnd = NonZeroIsize::new(handles.window as isize)
        .ok_or_else(|| EngineError::other("platform runtime returned null HWND"))?;

    let mut window = Win32WindowHandle::new(hwnd);
    window.hinstance = NonZeroIsize::new(handles.display as isize);

    let display = RawDisplayHandle::Windows(WindowsDisplayHandle::new());
    let window = RawWindowHandle::Win32(window);
    Ok((display, window))
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn native_to_raw_handles(
    _handles: NativeWindowHandlesV1,
) -> EngineResult<(RawDisplayHandle, RawWindowHandle)> {
    Err(EngineError::other(
        "platform runtime native handle conversion is only implemented for Windows",
    ))
}