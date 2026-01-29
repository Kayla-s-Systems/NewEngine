use crate::{CefApi, CefApiRef, CefViewId};
use log::info;
use parking_lot::Mutex;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

pub struct CefRuntime {
    api: CefApiRef,
    state: Arc<State>,
}

struct State {
    ready: AtomicBool,
    view_id: AtomicU64,
    host: Mutex<HostState>,
    content: Mutex<ContentState>,
}

#[derive(Default)]
struct HostState {
    attached: bool,
    width: u32,
    height: u32,
    focused: bool,

    #[cfg(target_os = "windows")]
    hwnd_host: isize,

    #[cfg(target_os = "windows")]
    hwnd_frame: isize,
}

#[derive(Default)]
struct ContentState {
    last_url: Option<String>,
    last_html: Option<String>,
}

impl CefRuntime {
    pub fn new() -> Result<Self, String> {
        let state = Arc::new(State {
            ready: AtomicBool::new(false),
            view_id: AtomicU64::new(1),
            host: Mutex::new(HostState::default()),
            content: Mutex::new(ContentState::default()),
        });

        let api: CefApiRef = Arc::new(CefApiImpl {
            state: state.clone(),
        });

        state.ready.store(true, Ordering::Release);

        Ok(Self { api, state })
    }

    #[inline]
    pub fn api(&self) -> CefApiRef {
        self.api.clone()
    }

    pub fn attach_window(
        &mut self,
        window: RawWindowHandle,
        _display: RawDisplayHandle,
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let mut host = self.state.host.lock();
        host.width = width;
        host.height = height;

        #[cfg(target_os = "windows")]
        {
            use raw_window_handle::RawWindowHandle as Rwh;

            let hwnd_host: isize = match window {
                Rwh::Win32(h) => h.hwnd.get() as isize,
                _ => return Err("unsupported RawWindowHandle for windows target".into()),
            };

            host.hwnd_host = hwnd_host;

            if host.hwnd_frame == 0 {
                let hwnd_frame = unsafe { win_create_child_frame(hwnd_host, width, height) };
                if hwnd_frame == 0 {
                    return Err("failed to create child frame window".into());
                }
                host.hwnd_frame = hwnd_frame;
            } else {
                unsafe { win_resize_child_frame(host.hwnd_frame, width, height) };
            }

            // TODO: create CEF browser and set parent = hwnd_frame.
        }

        host.attached = true;
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let mut host = self.state.host.lock();
        host.width = width;
        host.height = height;

        #[cfg(target_os = "windows")]
        if host.hwnd_frame != 0 {
            unsafe { win_resize_child_frame(host.hwnd_frame, width, height) };
        }
    }

    pub fn focus(&mut self, focused: bool) {
        let mut host = self.state.host.lock();
        host.focused = focused;

        #[cfg(target_os = "windows")]
        if focused && host.hwnd_frame != 0 {
            unsafe { win_focus_child_frame(host.hwnd_frame) };
        }
    }

    pub fn tick(&mut self) {
        // TODO:
        // - if using external message pump: cef_do_message_loop_work()
        // - otherwise no-op (multi-threaded loop)
    }

    pub fn shutdown(&mut self) {
        self.state.ready.store(false, Ordering::Release);

        #[cfg(target_os = "windows")]
        {
            let mut host = self.state.host.lock();
            if host.hwnd_frame != 0 {
                unsafe { win_destroy_child_frame(host.hwnd_frame) };
                host.hwnd_frame = 0;
            }
        }
    }
}

struct CefApiImpl {
    state: Arc<State>,
}

impl CefApi for CefApiImpl {
    fn is_ready(&self) -> bool {
        self.state.ready.load(Ordering::Acquire)
    }

    fn ensure_primary_view(&self) -> CefViewId {
        CefViewId(self.state.view_id.load(Ordering::Relaxed))
    }

    fn load_local_html(&self, html: &str) {
        let mut content = self.state.content.lock();
        content.last_html = Some(html.to_string());
        content.last_url = None;
        info!("CEF HTML content updated ({} bytes)", html.len());
    }

    fn load_url(&self, url: &str) {
        let mut content = self.state.content.lock();
        content.last_url = Some(url.to_string());
        content.last_html = None;
        info!("CEF URL updated: {url}");
    }

    fn eval_js(&self, _js: &str) {}

    fn focus(&self, focused: bool) {
        let mut host = self.state.host.lock();
        host.focused = focused;

        #[cfg(target_os = "windows")]
        if focused && host.hwnd_frame != 0 {
            unsafe { win_focus_child_frame(host.hwnd_frame) };
        }
    }

    fn resize(&self, width: u32, height: u32) {
        let mut host = self.state.host.lock();
        host.width = width;
        host.height = height;

        #[cfg(target_os = "windows")]
        if host.hwnd_frame != 0 {
            unsafe { win_resize_child_frame(host.hwnd_frame, width, height) };
        }
    }
}

#[cfg(target_os = "windows")]
unsafe fn win_create_child_frame(parent_hwnd: isize, width: u32, height: u32) -> isize {
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, SetWindowPos, ShowWindow, HMENU, HWND_TOP, SW_SHOW, WS_CHILD,
        WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_NOPARENTNOTIFY, WS_VISIBLE,
    };

    let hinstance = GetModuleHandleW(null());
    let class_name = wide_null("STATIC");
    let window_name = wide_null("");

    let hwnd: HWND = CreateWindowExW(
        WS_EX_NOPARENTNOTIFY,
        class_name.as_ptr(),
        window_name.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
        0,
        0,
        width as i32,
        height as i32,
        parent_hwnd as HWND,
        0 as HMENU,
        hinstance,
        null_mut(),
    );

    if !hwnd.is_null() {
        SetWindowPos(hwnd, HWND_TOP, 0, 0, width as i32, height as i32, 0);
        ShowWindow(hwnd, SW_SHOW);
        hwnd as isize
    } else {
        0
    }
}

#[cfg(target_os = "windows")]
unsafe fn win_resize_child_frame(hwnd: isize, width: u32, height: u32) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOP};

    SetWindowPos(hwnd as HWND, HWND_TOP, 0, 0, width as i32, height as i32, 0);
}

#[cfg(target_os = "windows")]
unsafe fn win_focus_child_frame(hwnd: isize) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

    SetForegroundWindow(hwnd as HWND);
}

#[cfg(target_os = "windows")]
unsafe fn win_destroy_child_frame(hwnd: isize) {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow;

    DestroyWindow(hwnd as HWND);
}

#[cfg(target_os = "windows")]
fn wide_null(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
