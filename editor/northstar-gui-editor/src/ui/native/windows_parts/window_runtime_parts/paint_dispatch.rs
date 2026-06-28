use super::*;

pub(super) unsafe fn paint(hwnd: Hwnd) {
    let mut ps: PaintStruct = zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let client = client_rect(hwnd);
    let width = (client.right - client.left).max(1);
    let height = (client.bottom - client.top).max(1);
    let paint_w = (ps.rc_paint.right - ps.rc_paint.left).max(1);
    let paint_h = (ps.rc_paint.bottom - ps.rc_paint.top).max(1);

    let mem_dc = CreateCompatibleDC(hdc);
    if mem_dc.is_null() {
        IntersectClipRect(
            hdc,
            ps.rc_paint.left,
            ps.rc_paint.top,
            ps.rc_paint.right,
            ps.rc_paint.bottom,
        );
        draw_editor_shell(hdc, client);
        EndPaint(hwnd, &ps);
        return;
    }

    let bitmap = CreateCompatibleBitmap(hdc, width, height);
    if bitmap.is_null() {
        DeleteDC(mem_dc);
        IntersectClipRect(
            hdc,
            ps.rc_paint.left,
            ps.rc_paint.top,
            ps.rc_paint.right,
            ps.rc_paint.bottom,
        );
        draw_editor_shell(hdc, client);
        EndPaint(hwnd, &ps);
        return;
    }

    let old = SelectObject(mem_dc, bitmap);
    IntersectClipRect(
        mem_dc,
        ps.rc_paint.left,
        ps.rc_paint.top,
        ps.rc_paint.right,
        ps.rc_paint.bottom,
    );
    draw_editor_shell(mem_dc, client);
    BitBlt(
        hdc,
        ps.rc_paint.left,
        ps.rc_paint.top,
        paint_w,
        paint_h,
        mem_dc,
        ps.rc_paint.left,
        ps.rc_paint.top,
        SRCCOPY,
    );
    SelectObject(mem_dc, old);
    DeleteObject(bitmap);
    DeleteDC(mem_dc);
    EndPaint(hwnd, &ps);
}
