#[derive(Default)]
struct GdiFramePresenter {
    memory_dc: HDC,
    bitmap: HBITMAP,
    old_bitmap: HGDIOBJ,
    width: i32,
    height: i32,
}

impl GdiFramePresenter {
    fn present(
        &mut self,
        hwnd: HWND,
        background: COLORREF,
        pixels: &[u8],
        source_width: u32,
        source_height: u32,
    ) -> Result<(), String> {
        let expected = source_width as usize * source_height as usize * 4;
        if pixels.len() < expected {
            return Err(format!(
                "startup intro RGB32 sample too small: actual={} expected_at_least={}",
                pixels.len(),
                expected
            ));
        }

        let mut client = RECT::default();
        unsafe { windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut client) }
            .map_err(|error| format!("startup intro GetClientRect failed: {error}"))?;
        let target_w = (client.right - client.left).max(1);
        let target_h = (client.bottom - client.top).max(1);
        let (draw_x, draw_y, draw_w, draw_h) = aspect_fit(
            target_w,
            target_h,
            source_width as i32,
            source_height as i32,
        );

        let window_dc = unsafe { GetDC(Some(hwnd)) };
        if window_dc.is_invalid() {
            return Err("startup intro GetDC returned invalid HDC".to_owned());
        }

        let result = (|| {
            self.ensure_backbuffer(window_dc, target_w, target_h)?;

            // Compose the whole frame off-screen. The visible HWND gets exactly one BitBlt,
            // so the background clear can never become a transient visible flash.
            let brush = unsafe { CreateSolidBrush(background) };
            if !brush.is_invalid() {
                let backbuffer_rect = RECT {
                    left: 0,
                    top: 0,
                    right: target_w,
                    bottom: target_h,
                };
                let _ = unsafe { FillRect(self.memory_dc, &backbuffer_rect, brush) };
                let _ = unsafe { DeleteObject(brush.into()) };
            }

            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: source_width as i32,
                    // copy_rgb32_sample normalizes scanline order to logical top-down rows.
                    biHeight: -(source_height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: expected as u32,
                    ..Default::default()
                },
                ..Default::default()
            };

            unsafe {
                SetStretchBltMode(self.memory_dc, HALFTONE);
                let _ = SetBrushOrgEx(self.memory_dc, 0, 0, None);
            }
            let copied = unsafe {
                StretchDIBits(
                    self.memory_dc,
                    draw_x,
                    draw_y,
                    draw_w,
                    draw_h,
                    0,
                    0,
                    source_width as i32,
                    source_height as i32,
                    Some(pixels.as_ptr().cast()),
                    &info,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                )
            };
            if copied == 0 {
                return Err("startup intro StretchDIBits copied zero scan lines".to_owned());
            }

            unsafe {
                BitBlt(
                    window_dc,
                    0,
                    0,
                    target_w,
                    target_h,
                    Some(self.memory_dc),
                    0,
                    0,
                    SRCCOPY,
                )
            }
            .map_err(|error| format!("startup intro backbuffer BitBlt failed: {error}"))?;
            let _ = unsafe { GdiFlush() };
            Ok(())
        })();

        let _ = unsafe { ReleaseDC(Some(hwnd), window_dc) };
        result
    }

    fn ensure_backbuffer(&mut self, window_dc: HDC, width: i32, height: i32) -> Result<(), String> {
        if !self.memory_dc.is_invalid()
            && !self.bitmap.is_invalid()
            && self.width == width
            && self.height == height
        {
            return Ok(());
        }
        self.reset();

        let memory_dc = unsafe { CreateCompatibleDC(Some(window_dc)) };
        if memory_dc.is_invalid() {
            return Err("startup intro CreateCompatibleDC failed".to_owned());
        }
        let bitmap = unsafe { CreateCompatibleBitmap(window_dc, width, height) };
        if bitmap.is_invalid() {
            let _ = unsafe { DeleteDC(memory_dc) };
            return Err("startup intro CreateCompatibleBitmap failed".to_owned());
        }
        let old_bitmap = unsafe { SelectObject(memory_dc, bitmap.into()) };
        if old_bitmap.is_invalid() {
            let _ = unsafe { DeleteObject(bitmap.into()) };
            let _ = unsafe { DeleteDC(memory_dc) };
            return Err("startup intro SelectObject(backbuffer) failed".to_owned());
        }

        self.memory_dc = memory_dc;
        self.bitmap = bitmap;
        self.old_bitmap = old_bitmap;
        self.width = width;
        self.height = height;
        Ok(())
    }

    fn reset(&mut self) {
        if !self.memory_dc.is_invalid() {
            if !self.old_bitmap.is_invalid() {
                unsafe { SelectObject(self.memory_dc, self.old_bitmap) };
            }
            if !self.bitmap.is_invalid() {
                let _ = unsafe { DeleteObject(self.bitmap.into()) };
            }
            let _ = unsafe { DeleteDC(self.memory_dc) };
        }
        self.memory_dc = HDC::default();
        self.bitmap = HBITMAP::default();
        self.old_bitmap = HGDIOBJ::default();
        self.width = 0;
        self.height = 0;
    }
}

impl Drop for GdiFramePresenter {
    fn drop(&mut self) {
        self.reset();
    }
}

fn aspect_fit(target_w: i32, target_h: i32, source_w: i32, source_h: i32) -> (i32, i32, i32, i32) {
    let tw = target_w.max(1) as i64;
    let th = target_h.max(1) as i64;
    let sw = source_w.max(1) as i64;
    let sh = source_h.max(1) as i64;
    let (dw, dh) = if tw * sh <= th * sw {
        (tw, (tw * sh / sw).max(1))
    } else {
        ((th * sw / sh).max(1), th)
    };
    (
        ((tw - dw) / 2) as i32,
        ((th - dh) / 2) as i32,
        dw as i32,
        dh as i32,
    )
}

fn clear_window(hwnd: HWND, background: COLORREF) {
    let mut rect = RECT::default();
    if unsafe { windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect) }.is_err() {
        return;
    }
    let hdc = unsafe { GetDC(Some(hwnd)) };
    if hdc.is_invalid() {
        return;
    }
    let brush = unsafe { CreateSolidBrush(background) };
    if !brush.is_invalid() {
        let _ = unsafe { FillRect(hdc, &rect, brush) };
        let _ = unsafe { DeleteObject(brush.into()) };
    }
    let _ = unsafe { ReleaseDC(Some(hwnd), hdc) };
    let _ = unsafe { GdiFlush() };
}
