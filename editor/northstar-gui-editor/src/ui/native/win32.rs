use std::ffi::c_void;

pub(super) type Bool = i32;
pub(super) type Dword = u32;
pub(super) type Hbrush = *mut c_void;
pub(super) type Hcursor = *mut c_void;
pub(super) type Hdc = *mut c_void;
pub(super) type Hfont = *mut c_void;
pub(super) type Hgdiobj = *mut c_void;
pub(super) type Hicon = *mut c_void;
pub(super) type Hinstance = *mut c_void;
pub(super) type Hmenu = *mut c_void;
pub(super) type Hwnd = *mut c_void;
pub(super) type Lparam = isize;
pub(super) type Lpcwstr = *const u16;
pub(super) type Lresult = isize;
pub(super) type Uint = u32;
pub(super) type Wparam = usize;

pub(super) const COLOR_WINDOW: isize = 5;
pub(super) const CS_DBLCLKS: Uint = 0x0008;
pub(super) const CW_USEDEFAULT: i32 = 0x80000000u32 as i32;
pub(super) const DT_END_ELLIPSIS: u32 = 0x8000;
pub(super) const DT_LEFT: u32 = 0x0000;
pub(super) const DT_SINGLELINE: u32 = 0x0020;
pub(super) const DT_TOP: u32 = 0x0000;
pub(super) const DT_VCENTER: u32 = 0x0004;
pub(super) const FW_NORMAL: i32 = 400;
pub(super) const FW_SEMIBOLD: i32 = 600;
pub(super) const IDC_ARROW: usize = 32512;
pub(super) const OUT_DEFAULT_PRECIS: u32 = 0;
pub(super) const CLIP_DEFAULT_PRECIS: u32 = 0;
pub(super) const DEFAULT_QUALITY: u32 = 0;
pub(super) const DEFAULT_PITCH: u32 = 0;
pub(super) const FF_DONTCARE: u32 = 0;
pub(super) const TRANSPARENT: i32 = 1;
pub(super) const SW_SHOW: i32 = 5;
pub(super) const SW_HIDE: i32 = 0;
pub(super) const SRCCOPY: Dword = 0x00CC0020;
pub(super) const WS_EX_TOOLWINDOW: Dword = 0x00000080;
pub(super) const WS_EX_TOPMOST: Dword = 0x00000008;
pub(super) const WM_DESTROY: Uint = 0x0002;
pub(super) const WM_ERASEBKGND: Uint = 0x0014;
pub(super) const WM_KEYDOWN: Uint = 0x0100;
pub(super) const WM_CHAR: Uint = 0x0102;
pub(super) const WM_CLOSE: Uint = 0x0010;
pub(super) const WM_LBUTTONDOWN: Uint = 0x0201;
pub(super) const WM_LBUTTONUP: Uint = 0x0202;
pub(super) const WM_LBUTTONDBLCLK: Uint = 0x0203;
pub(super) const WM_MOUSEMOVE: Uint = 0x0200;
pub(super) const WM_MOUSEWHEEL: Uint = 0x020A;
pub(super) const WM_PAINT: Uint = 0x000F;
pub(super) const WM_SIZE: Uint = 0x0005;
pub(super) const WM_TIMER: Uint = 0x0113;
pub(super) const WS_OVERLAPPEDWINDOW: Dword = 0x00CF0000;
pub(super) const WS_POPUP: Dword = 0x80000000;
pub(super) const WS_CAPTION: Dword = 0x00C00000;
pub(super) const WS_SYSMENU: Dword = 0x00080000;
pub(super) const VK_BACK: usize = 0x08;
pub(super) const VK_CONTROL: usize = 0x11;
pub(super) const VK_KEY_A: usize = 0x41;
pub(super) const VK_KEY_S: usize = 0x53;
pub(super) const VK_KEY_Y: usize = 0x59;
pub(super) const VK_KEY_Z: usize = 0x5A;
pub(super) const VK_DELETE_FORWARD: usize = 0x2E;
pub(super) const VK_RETURN: usize = 0x0D;
pub(super) const VK_ESCAPE: usize = 0x1B;
pub(super) const VK_UP: usize = 0x26;
pub(super) const VK_DOWN: usize = 0x28;
pub(super) const VK_F5: usize = 0x74;
pub(super) const CARET_TIMER_ID: usize = 0x4E53;
pub(super) const CARET_BLINK_MS: Uint = 530;
pub(super) const MODAL_WIDTH: i32 = 860;
pub(super) const MODAL_HEIGHT: i32 = 640;
pub(super) const MAX_PATH: usize = 260;
pub(super) const BIF_RETURNONLYFSDIRS: Uint = 0x0001;
pub(super) const BIF_NEWDIALOGSTYLE: Uint = 0x0040;

#[repr(C)]
pub(super) struct Point {
    pub(super) x: i32,
    pub(super) y: i32,
}

#[repr(C)]
pub(super) struct Msg {
    pub(super) hwnd: Hwnd,
    pub(super) message: Uint,
    pub(super) w_param: Wparam,
    pub(super) l_param: Lparam,
    pub(super) time: Dword,
    pub(super) pt: Point,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(super) struct Rect {
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) right: i32,
    pub(super) bottom: i32,
}

#[repr(C)]
pub(super) struct PaintStruct {
    pub(super) hdc: Hdc,
    pub(super) f_erase: Bool,
    pub(super) rc_paint: Rect,
    pub(super) f_restore: Bool,
    pub(super) f_inc_update: Bool,
    pub(super) rgb_reserved: [u8; 32],
}

#[repr(C)]
pub(super) struct WndClassW {
    pub(super) style: Uint,
    pub(super) lpfn_wnd_proc:
        Option<unsafe extern "system" fn(Hwnd, Uint, Wparam, Lparam) -> Lresult>,
    pub(super) cb_cls_extra: i32,
    pub(super) cb_wnd_extra: i32,
    pub(super) h_instance: Hinstance,
    pub(super) h_icon: Hicon,
    pub(super) h_cursor: Hcursor,
    pub(super) hbr_background: Hbrush,
    pub(super) lpsz_menu_name: Lpcwstr,
    pub(super) lpsz_class_name: Lpcwstr,
}

#[repr(C)]
pub(super) struct BrowseInfoW {
    pub(super) hwnd_owner: Hwnd,
    pub(super) pidl_root: *const c_void,
    pub(super) psz_display_name: *mut u16,
    pub(super) lpsz_title: Lpcwstr,
    pub(super) ul_flags: Uint,
    pub(super) lpfn: Option<unsafe extern "system" fn(Hwnd, Uint, Lparam, Lparam) -> i32>,
    pub(super) l_param: Lparam,
    pub(super) i_image: i32,
}

#[link(name = "kernel32")]
extern "system" {
    pub(super) fn GetModuleHandleW(lp_module_name: Lpcwstr) -> Hinstance;
}

#[link(name = "gdi32")]
extern "system" {
    pub(super) fn BitBlt(
        hdc: Hdc,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        hdc_src: Hdc,
        x1: i32,
        y1: i32,
        rop: Dword,
    ) -> Bool;
    pub(super) fn CreateCompatibleBitmap(hdc: Hdc, cx: i32, cy: i32) -> Hgdiobj;
    pub(super) fn CreateCompatibleDC(hdc: Hdc) -> Hdc;
    pub(super) fn CreateFontW(
        c_height: i32,
        c_width: i32,
        c_escapement: i32,
        c_orientation: i32,
        c_weight: i32,
        b_italic: Dword,
        b_underline: Dword,
        b_strike_out: Dword,
        i_char_set: Dword,
        i_out_precision: Dword,
        i_clip_precision: Dword,
        i_quality: Dword,
        i_pitch_and_family: Dword,
        psz_face_name: Lpcwstr,
    ) -> Hfont;
    pub(super) fn CreateSolidBrush(color: Dword) -> Hbrush;
    pub(super) fn DeleteDC(hdc: Hdc) -> Bool;
    pub(super) fn DeleteObject(ho: Hgdiobj) -> Bool;
    pub(super) fn SelectObject(hdc: Hdc, h: Hgdiobj) -> Hgdiobj;
    pub(super) fn SetBkMode(hdc: Hdc, mode: i32) -> i32;
    pub(super) fn SetTextColor(hdc: Hdc, color: Dword) -> Dword;
}

#[link(name = "shell32")]
extern "system" {
    pub(super) fn SHBrowseForFolderW(lpbi: *mut BrowseInfoW) -> *mut c_void;
    pub(super) fn SHGetPathFromIDListW(pidl: *const c_void, psz_path: *mut u16) -> Bool;
}

#[link(name = "ole32")]
extern "system" {
    pub(super) fn CoTaskMemFree(pv: *mut c_void);
}

#[link(name = "user32")]
extern "system" {
    pub(super) fn BeginPaint(hwnd: Hwnd, lp_paint: *mut PaintStruct) -> Hdc;
    pub(super) fn CreateWindowExW(
        dw_ex_style: Dword,
        lp_class_name: Lpcwstr,
        lp_window_name: Lpcwstr,
        dw_style: Dword,
        x: i32,
        y: i32,
        n_width: i32,
        n_height: i32,
        h_wnd_parent: Hwnd,
        h_menu: Hmenu,
        h_instance: Hinstance,
        lp_param: *mut c_void,
    ) -> Hwnd;
    pub(super) fn DefWindowProcW(
        hwnd: Hwnd,
        msg: Uint,
        w_param: Wparam,
        l_param: Lparam,
    ) -> Lresult;
    pub(super) fn DestroyWindow(hwnd: Hwnd) -> Bool;
    pub(super) fn DispatchMessageW(lp_msg: *const Msg) -> Lresult;
    pub(super) fn DrawTextW(
        hdc: Hdc,
        lpch_text: Lpcwstr,
        cch_text: i32,
        lprc: *mut Rect,
        format: Uint,
    ) -> i32;
    pub(super) fn EndPaint(hwnd: Hwnd, lp_paint: *const PaintStruct) -> Bool;
    pub(super) fn FillRect(hdc: Hdc, lprc: *const Rect, hbr: Hbrush) -> i32;
    pub(super) fn FrameRect(hdc: Hdc, lprc: *const Rect, hbr: Hbrush) -> i32;
    pub(super) fn GetClientRect(hwnd: Hwnd, lp_rect: *mut Rect) -> Bool;
    pub(super) fn GetKeyState(n_virt_key: i32) -> i16;
    pub(super) fn GetMessageW(
        lp_msg: *mut Msg,
        hwnd: Hwnd,
        w_msg_filter_min: Uint,
        w_msg_filter_max: Uint,
    ) -> Bool;
    pub(super) fn GetWindowRect(hwnd: Hwnd, lp_rect: *mut Rect) -> Bool;
    pub(super) fn InvalidateRect(hwnd: Hwnd, lp_rect: *const Rect, b_erase: Bool) -> Bool;
    pub(super) fn IntersectClipRect(hdc: Hdc, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    pub(super) fn KillTimer(hwnd: Hwnd, n_id_event: usize) -> Bool;
    pub(super) fn LoadCursorW(h_instance: Hinstance, lp_cursor_name: Lpcwstr) -> Hcursor;
    pub(super) fn MoveWindow(
        hwnd: Hwnd,
        x: i32,
        y: i32,
        n_width: i32,
        n_height: i32,
        b_repaint: Bool,
    ) -> Bool;
    pub(super) fn PostQuitMessage(n_exit_code: i32);
    pub(super) fn RegisterClassW(lp_wnd_class: *const WndClassW) -> u16;
    pub(super) fn ReleaseCapture() -> Bool;
    pub(super) fn SetFocus(hwnd: Hwnd) -> Hwnd;
    pub(super) fn SetCapture(hwnd: Hwnd) -> Hwnd;
    pub(super) fn SetTimer(
        hwnd: Hwnd,
        n_id_event: usize,
        u_elapse: Uint,
        lp_timer_func: *const c_void,
    ) -> usize;
    pub(super) fn SetWindowTextW(hwnd: Hwnd, lp_string: Lpcwstr) -> Bool;
    pub(super) fn ShowWindow(hwnd: Hwnd, n_cmd_show: i32) -> Bool;
    pub(super) fn TranslateMessage(lp_msg: *const Msg) -> Bool;
    pub(super) fn UpdateWindow(hwnd: Hwnd) -> Bool;
}
