use super::*;

pub(super) unsafe extern "system" fn window_proc(
    hwnd: Hwnd,
    msg: Uint,
    w_param: Wparam,
    l_param: Lparam,
) -> Lresult {
    match msg {
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_LBUTTONDOWN => {
            SetFocus(hwnd);
            handle_click(hwnd, lparam_x(l_param), lparam_y(l_param));
            0
        }
        WM_LBUTTONDBLCLK => {
            SetFocus(hwnd);
            handle_double_click(hwnd, lparam_x(l_param), lparam_y(l_param));
            0
        }
        WM_LBUTTONUP => {
            handle_mouse_up(hwnd);
            0
        }
        WM_MOUSEMOVE => {
            handle_hover_panel(hwnd, lparam_x(l_param), lparam_y(l_param));
            0
        }
        WM_MOUSEWHEEL => {
            handle_mouse_wheel(hwnd, wheel_delta(w_param));
            0
        }
        WM_KEYDOWN => {
            handle_key(hwnd, w_param);
            0
        }
        WM_CHAR => {
            handle_char(hwnd, w_param);
            0
        }
        WM_TIMER if w_param == CARET_TIMER_ID => {
            if toggle_caret_blink() {
                apply_ui_update(hwnd, UiUpdateRequest::Full);
            }
            0
        }
        WM_SIZE => {
            apply_ui_update(hwnd, UiUpdateRequest::Layout);
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, CARET_TIMER_ID);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w_param, l_param),
    }
}
