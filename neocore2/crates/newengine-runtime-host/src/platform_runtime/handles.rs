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

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn raw_to_native_handles(
    window: RawWindowHandle,
    display: RawDisplayHandle,
) -> EngineResult<NativeWindowHandlesV1> {
    match (display, window) {
        (RawDisplayHandle::Wayland(display), RawWindowHandle::Wayland(window)) => {
            Ok(NativeWindowHandlesV1 {
                backend: NativeWindowBackendV1::Wayland,
                window: window.surface.as_ptr() as u64,
                display: display.display.as_ptr() as u64,
                reserved0: 0,
                reserved1: 0,
            })
        }
        (RawDisplayHandle::Xlib(display), RawWindowHandle::Xlib(window)) => {
            Ok(NativeWindowHandlesV1 {
                backend: NativeWindowBackendV1::Xlib,
                window: window.window,
                display: display.display.map(|v| v.as_ptr() as u64).unwrap_or_default(),
                reserved0: display.screen as u64,
                reserved1: window.visual_id,
            })
        }
        (RawDisplayHandle::Xcb(display), RawWindowHandle::Xcb(window)) => {
            Ok(NativeWindowHandlesV1 {
                backend: NativeWindowBackendV1::Xcb,
                window: window.window.get() as u64,
                display: display.connection.map(|v| v.as_ptr() as u64).unwrap_or_default(),
                reserved0: display.screen as u64,
                reserved1: window.visual_id.map(|v| v.get() as u64).unwrap_or_default(),
            })
        }
        _ => Err(EngineError::other(
            "platform runtime raw handle conversion supports Wayland, Xlib and Xcb on Linux/Unix",
        )),
    }
}

#[cfg(not(any(target_os = "windows", all(unix, not(target_os = "macos")))))]
pub(crate) fn raw_to_native_handles(
    _window: RawWindowHandle,
    _display: RawDisplayHandle,
) -> EngineResult<NativeWindowHandlesV1> {
    Err(EngineError::other(
        "platform runtime raw handle conversion is not implemented for this OS",
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

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn native_to_raw_handles(
    handles: NativeWindowHandlesV1,
) -> EngineResult<(RawDisplayHandle, RawWindowHandle)> {
    use core::ffi::c_void;
    use core::ptr::NonNull;
    use raw_window_handle::{
        WaylandDisplayHandle, WaylandWindowHandle, XcbDisplayHandle, XcbWindowHandle,
        XlibDisplayHandle, XlibWindowHandle,
    };
    use std::num::NonZeroU32;

    match handles.backend {
        NativeWindowBackendV1::Wayland => {
            let display = NonNull::new(handles.display as *mut c_void)
                .ok_or_else(|| EngineError::other("platform runtime returned null wl_display"))?;
            let surface = NonNull::new(handles.window as *mut c_void)
                .ok_or_else(|| EngineError::other("platform runtime returned null wl_surface"))?;
            Ok((
                RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display)),
                RawWindowHandle::Wayland(WaylandWindowHandle::new(surface)),
            ))
        }
        NativeWindowBackendV1::Xlib => {
            let display = NonNull::new(handles.display as *mut c_void);
            let mut display_handle = XlibDisplayHandle::new(display, handles.reserved0 as i32);
            display_handle.screen = handles.reserved0 as i32;
            let mut window = XlibWindowHandle::new(handles.window);
            window.visual_id = handles.reserved1;
            Ok((
                RawDisplayHandle::Xlib(display_handle),
                RawWindowHandle::Xlib(window),
            ))
        }
        NativeWindowBackendV1::Xcb => {
            let connection = NonNull::new(handles.display as *mut c_void);
            let display = XcbDisplayHandle::new(connection, handles.reserved0 as i32);
            let window_id = NonZeroU32::new(handles.window as u32)
                .ok_or_else(|| EngineError::other("platform runtime returned null XCB window"))?;
            let mut window = XcbWindowHandle::new(window_id);
            window.visual_id = NonZeroU32::new(handles.reserved1 as u32);
            Ok((
                RawDisplayHandle::Xcb(display),
                RawWindowHandle::Xcb(window),
            ))
        }
        other => Err(EngineError::other(format!(
            "unsupported native window backend on Linux/Unix: {:?}",
            other
        ))),
    }
}

#[cfg(not(any(target_os = "windows", all(unix, not(target_os = "macos")))))]
pub(crate) fn native_to_raw_handles(
    _handles: NativeWindowHandlesV1,
) -> EngineResult<(RawDisplayHandle, RawWindowHandle)> {
    Err(EngineError::other(
        "platform runtime native handle conversion is not implemented for this OS",
    ))
}
