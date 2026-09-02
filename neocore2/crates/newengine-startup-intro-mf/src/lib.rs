#![forbid(unsafe_op_in_unsafe_fn)]

/// Installs the Windows Media Foundation presenter into the platform-neutral
/// startup-intro presenter port. Registration is deterministic and first-wins.
#[cfg(windows)]
pub fn install() -> bool {
    newengine_startup_intro::install_startup_intro_presenter(windows_provider::present)
}

#[cfg(not(windows))]
pub fn install() -> bool {
    false
}

#[cfg(windows)]
mod windows_provider {
    use std::{
        path::Path,
        sync::mpsc::{sync_channel, TryRecvError},
        thread,
        time::{Duration, Instant},
    };

    use newengine_startup_intro::{
        ResolvedStartupIntro, ResolvedStartupIntroEntry, StartupIntroNativeBackend,
        StartupIntroNativeWindow,
    };
    use windows::{
        core::{implement, Interface, PCWSTR},
        Win32::{
            Foundation::{COLORREF, HWND, RECT},
            Graphics::Gdi::{
                BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateSolidBrush, DeleteDC,
                DeleteObject, FillRect, GdiFlush, GetDC, ReleaseDC, SelectObject, SetBrushOrgEx,
                SetStretchBltMode, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
                DIB_RGB_COLORS, HALFTONE, HBITMAP, HDC, HGDIOBJ, SRCCOPY,
            },
            Media::MediaFoundation::{
                IMF2DBuffer, IMFAttributes, IMFPMediaPlayer, IMFPMediaPlayerCallback,
                IMFPMediaPlayerCallback_Impl, IMFSample, IMFSourceReader, MFCreateAttributes,
                MFCreateMediaType, MFCreateSourceReaderFromURL, MFMediaType_Video,
                MFPCreateMediaPlayer, MFShutdown, MFStartup, MFVideoFormat_RGB32, MFP_EVENT_HEADER,
                MFP_OPTION_FREE_THREADED_CALLBACK, MFSTARTUP_FULL, MF_MT_FRAME_SIZE,
                MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED,
                MF_SOURCE_READERF_ENDOFSTREAM, MF_SOURCE_READERF_ERROR,
                MF_SOURCE_READER_ALL_STREAMS, MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING,
                MF_SOURCE_READER_FIRST_VIDEO_STREAM, MF_VERSION,
            },
            System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED},
            UI::{
                Input::KeyboardAndMouse::{
                    GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON, VK_RETURN, VK_SPACE,
                },
                WindowsAndMessaging::{
                    DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
                },
            },
        },
    };

    pub(super) fn present(
        payload: &ResolvedStartupIntro,
        target: StartupIntroNativeWindow,
    ) -> Result<(), String> {
        if target.backend != StartupIntroNativeBackend::Win32 {
            return Err(format!(
                "startup intro MF provider requires Win32 target; actual={:?}",
                target.backend
            ));
        }
        if target.window == 0 {
            return Err("startup intro MF provider received a null game HWND".to_owned());
        }

        // Decode in a dedicated MTA apartment, but never hand the game HWND to MFPlay/EVR.
        // SourceReader produces RGB32 frames and this provider paints them into the existing
        // game window. The host thread remains responsive by pumping messages while the
        // synchronous startup-intro barrier waits for terminal playback.
        let terminal_background = parse_colorref(&payload.window.background).unwrap_or(COLORREF(0));
        let payload = payload.clone();
        let window = target.window;
        let (result_tx, result_rx) = sync_channel::<Result<(), String>>(1);
        let worker = thread::Builder::new()
            .name("northstar-startup-intro-mf-source-reader".to_owned())
            .spawn(move || {
                let result = present_on_mta(&payload, window);
                let _ = result_tx.send(result);
            })
            .map_err(|error| format!("startup intro MF worker spawn failed: {error}"))?;

        let result = loop {
            pump_messages();
            match result_rx.try_recv() {
                Ok(result) => break result,
                Err(TryRecvError::Empty) => thread::sleep(Duration::from_millis(2)),
                Err(TryRecvError::Disconnected) => {
                    break Err(
                        "startup intro MF worker disconnected before returning playback result"
                            .to_owned(),
                    )
                }
            }
        };

        if worker.join().is_err() {
            clear_window(
                HWND(window as usize as *mut core::ffi::c_void),
                terminal_background,
            );
            return Err("startup intro MF worker panicked".to_owned());
        }

        // On success, deliberately keep the final decoded intro frame resident until the
        // loading presenter performs its first same-HWND Present. Clearing here would create
        // a visible black gap while the boot DXGI device/swapchain is being initialized.
        // Failure paths still clear to the authored background so stale/error pixels do not
        // survive into recovery.
        if result.is_err() {
            clear_window(
                HWND(window as usize as *mut core::ffi::c_void),
                terminal_background,
            );
        }
        result
    }

    fn present_on_mta(payload: &ResolvedStartupIntro, window: u64) -> Result<(), String> {
        let hwnd = HWND(window as usize as *mut core::ffi::c_void);
        let background = parse_colorref(&payload.window.background).unwrap_or(COLORREF(0));
        let _com = ComApartment::initialize()?;
        let _media_foundation = MediaFoundation::startup()?;

        for entry in &payload.sequence {
            play_entry(hwnd, entry, background)?;
            drain_skip_keys();
        }
        Ok(())
    }

    struct ComApartment;

    impl ComApartment {
        fn initialize() -> Result<Self, String> {
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            hr.ok()
                .map_err(|error| format!("startup intro COM MTA initialization failed: {error}"))?;
            Ok(Self)
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }

    struct MediaFoundation;

    impl MediaFoundation {
        fn startup() -> Result<Self, String> {
            unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.map_err(|error| {
                format!("startup intro Media Foundation startup failed: {error}")
            })?;
            Ok(Self)
        }
    }

    impl Drop for MediaFoundation {
        fn drop(&mut self) {
            let _ = unsafe { MFShutdown() };
        }
    }

    #[implement(IMFPMediaPlayerCallback)]
    struct AudioOnlyCallback;

    impl IMFPMediaPlayerCallback_Impl for AudioOnlyCallback_Impl {
        fn OnMediaPlayerEvent(&self, _event: *const MFP_EVENT_HEADER) {}
    }

    /// Optional audio companion. MFPlay is deliberately given no HWND, so it can never own
    /// the game-window video presentation surface. If creation fails (for example a silent
    /// video on a machine without a suitable audio route), video playback still proceeds.
    struct AudioOnlyPlayback {
        player: IMFPMediaPlayer,
        _callback: IMFPMediaPlayerCallback,
    }

    impl AudioOnlyPlayback {
        fn start(source_wide: &[u16], volume: f32) -> Option<Self> {
            if volume <= 0.0 {
                return None;
            }
            let callback: IMFPMediaPlayerCallback = AudioOnlyCallback.into();
            let mut player = None;
            unsafe {
                MFPCreateMediaPlayer(
                    PCWSTR(source_wide.as_ptr()),
                    true,
                    MFP_OPTION_FREE_THREADED_CALLBACK,
                    &callback,
                    None,
                    Some(&mut player),
                )
            }
            .ok()?;
            let player = player?;
            let _ = unsafe { player.SetVolume(volume.clamp(0.0, 1.0)) };
            Some(Self {
                player,
                _callback: callback,
            })
        }
    }

    impl Drop for AudioOnlyPlayback {
        fn drop(&mut self) {
            let _ = unsafe { self.player.Stop() };
            let _ = unsafe { self.player.ClearMediaItem() };
            let _ = unsafe { self.player.Shutdown() };
        }
    }

    fn play_entry(
        hwnd: HWND,
        entry: &ResolvedStartupIntroEntry,
        background: COLORREF,
    ) -> Result<(), String> {
        let source = absolute_source(&entry.source)?;
        let source_wide = wide(&source);
        let _audio = AudioOnlyPlayback::start(&source_wide, entry.volume);
        let reader = create_video_reader(&source_wide, entry)?;
        let video_stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        let mut dimensions = current_frame_dimensions(&reader, video_stream, entry)?;

        let started = Instant::now();
        let timeout = Duration::from_millis(entry.max_duration_ms.max(1));
        let mut first_timestamp_hns: Option<i64> = None;
        let mut timeline_started_at: Option<Instant> = None;
        let mut presenter = GdiFramePresenter::default();

        loop {
            pump_messages();
            if entry.skippable && skip_requested() {
                return Ok(());
            }
            if started.elapsed() >= timeout {
                return Err(format!(
                    "startup intro entry '{}' exceeded max_duration_ms={}",
                    entry.id, entry.max_duration_ms
                ));
            }

            let mut flags = 0u32;
            let mut timestamp_hns = 0i64;
            let mut sample = None;
            unsafe {
                reader.ReadSample(
                    video_stream,
                    0,
                    None,
                    Some(&mut flags),
                    Some(&mut timestamp_hns),
                    Some(&mut sample),
                )
            }
            .map_err(|error| {
                format!(
                    "startup intro entry '{}' SourceReader ReadSample failed: {error}",
                    entry.id
                )
            })?;

            if flags & MF_SOURCE_READERF_ERROR.0 as u32 != 0 {
                return Err(format!(
                    "startup intro entry '{}' SourceReader reported MF_SOURCE_READERF_ERROR",
                    entry.id
                ));
            }
            if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
                dimensions = current_frame_dimensions(&reader, video_stream, entry)?;
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                break;
            }

            let Some(sample) = sample else {
                continue;
            };

            let first_ts = *first_timestamp_hns.get_or_insert(timestamp_hns);
            let clock = *timeline_started_at.get_or_insert_with(Instant::now);
            let relative_hns = timestamp_hns.saturating_sub(first_ts).max(0);
            let due = duration_from_hns(relative_hns);
            wait_until_frame_due(clock, due, entry, started, timeout)?;
            if entry.skippable && skip_requested() {
                return Ok(());
            }

            let frame = copy_rgb32_sample(&sample, dimensions.0, dimensions.1, entry)?;
            presenter.present(hwnd, background, &frame, dimensions.0, dimensions.1)?;
        }

        Ok(())
    }

    fn create_video_reader(
        source_wide: &[u16],
        entry: &ResolvedStartupIntroEntry,
    ) -> Result<IMFSourceReader, String> {
        let mut attributes: Option<IMFAttributes> = None;
        unsafe { MFCreateAttributes(&mut attributes, 2) }.map_err(|error| {
            format!(
                "startup intro entry '{}' SourceReader attributes creation failed: {error}",
                entry.id
            )
        })?;
        let attributes = attributes.ok_or_else(|| {
            format!(
                "startup intro entry '{}' Media Foundation returned no SourceReader attributes",
                entry.id
            )
        })?;
        unsafe { attributes.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1) }.map_err(
            |error| {
                format!(
                    "startup intro entry '{}' enabling SourceReader video processing failed: {error}",
                    entry.id
                )
            },
        )?;

        let reader =
            unsafe { MFCreateSourceReaderFromURL(PCWSTR(source_wide.as_ptr()), &attributes) }
                .map_err(|error| {
                    format!(
                        "startup intro entry '{}' SourceReader creation failed: {error}",
                        entry.id
                    )
                })?;

        let all_streams = MF_SOURCE_READER_ALL_STREAMS.0 as u32;
        let video_stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        let _ = unsafe { reader.SetStreamSelection(all_streams, false) };
        unsafe { reader.SetStreamSelection(video_stream, true) }.map_err(|error| {
            format!(
                "startup intro entry '{}' selecting first video stream failed: {error}",
                entry.id
            )
        })?;

        let media_type = unsafe { MFCreateMediaType() }.map_err(|error| {
            format!(
                "startup intro entry '{}' RGB32 media type creation failed: {error}",
                entry.id
            )
        })?;
        unsafe { media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video) }.map_err(|error| {
            format!(
                "startup intro entry '{}' setting video major type failed: {error}",
                entry.id
            )
        })?;
        unsafe { media_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32) }.map_err(|error| {
            format!(
                "startup intro entry '{}' setting RGB32 subtype failed: {error}",
                entry.id
            )
        })?;
        unsafe { reader.SetCurrentMediaType(video_stream, None, &media_type) }.map_err(
            |error| {
                format!(
                    "startup intro entry '{}' requesting RGB32 decode failed: {error}",
                    entry.id
                )
            },
        )?;
        Ok(reader)
    }

    fn current_frame_dimensions(
        reader: &IMFSourceReader,
        stream: u32,
        entry: &ResolvedStartupIntroEntry,
    ) -> Result<(u32, u32), String> {
        let media_type = unsafe { reader.GetCurrentMediaType(stream) }.map_err(|error| {
            format!(
                "startup intro entry '{}' current media type query failed: {error}",
                entry.id
            )
        })?;
        let packed = unsafe { media_type.GetUINT64(&MF_MT_FRAME_SIZE) }.map_err(|error| {
            format!(
                "startup intro entry '{}' frame-size query failed: {error}",
                entry.id
            )
        })?;
        let width = (packed >> 32) as u32;
        let height = packed as u32;
        if width == 0 || height == 0 {
            return Err(format!(
                "startup intro entry '{}' decoded invalid frame size {}x{}",
                entry.id, width, height
            ));
        }
        Ok((width, height))
    }

    fn wait_until_frame_due(
        clock: Instant,
        due: Duration,
        entry: &ResolvedStartupIntroEntry,
        playback_started: Instant,
        timeout: Duration,
    ) -> Result<(), String> {
        loop {
            let elapsed = clock.elapsed();
            if elapsed >= due {
                return Ok(());
            }
            if entry.skippable && skip_requested() {
                return Ok(());
            }
            if playback_started.elapsed() >= timeout {
                return Err(format!(
                    "startup intro entry '{}' exceeded max_duration_ms={}",
                    entry.id, entry.max_duration_ms
                ));
            }
            pump_messages();
            let remaining = due.saturating_sub(elapsed);
            thread::sleep(remaining.min(Duration::from_millis(2)));
        }
    }

    fn duration_from_hns(hns: i64) -> Duration {
        let hns = hns.max(0) as u64;
        Duration::from_nanos(hns.saturating_mul(100))
    }

    fn copy_rgb32_sample(
        sample: &IMFSample,
        source_width: u32,
        source_height: u32,
        entry: &ResolvedStartupIntroEntry,
    ) -> Result<Vec<u8>, String> {
        let row_bytes = source_width as usize * 4;
        let expected = row_bytes * source_height as usize;

        // Video-processor output is commonly an IMF2DBuffer with a pitch that is not
        // guaranteed to equal width*4. Lock2D gives authoritative scanline-0 + signed pitch.
        // Repack into a tight top-down RGB32 buffer before handing it to GDI. Ignoring pitch
        // produces the characteristic grid/diagonal corruption when rows contain padding.
        if let Ok(buffer) = unsafe { sample.GetBufferByIndex(0) } {
            if let Ok(buffer_2d) = buffer.cast::<IMF2DBuffer>() {
                let mut scanline0 = std::ptr::null_mut();
                let mut pitch = 0i32;
                unsafe { buffer_2d.Lock2D(&mut scanline0, &mut pitch) }.map_err(|error| {
                    format!(
                        "startup intro entry '{}' IMF2DBuffer::Lock2D failed: {error}",
                        entry.id
                    )
                })?;

                let result = (|| {
                    if scanline0.is_null() {
                        return Err(format!(
                            "startup intro entry '{}' IMF2DBuffer returned null scanline0",
                            entry.id
                        ));
                    }
                    let pitch_abs = pitch.unsigned_abs() as usize;
                    if pitch_abs < row_bytes {
                        return Err(format!(
                            "startup intro entry '{}' RGB32 pitch too small: pitch={} row_bytes={}",
                            entry.id, pitch, row_bytes
                        ));
                    }

                    let mut tight = vec![0u8; expected];
                    for y in 0..source_height as usize {
                        let src = unsafe { scanline0.offset((y as isize) * (pitch as isize)) };
                        let dst = unsafe { tight.as_mut_ptr().add(y * row_bytes) };
                        unsafe { std::ptr::copy_nonoverlapping(src, dst, row_bytes) };
                    }
                    Ok(tight)
                })();
                let unlock = unsafe { buffer_2d.Unlock2D() };
                if let Err(error) = unlock {
                    return Err(format!(
                        "startup intro entry '{}' IMF2DBuffer::Unlock2D failed: {error}",
                        entry.id
                    ));
                }
                return result;
            }
        }

        // Conservative fallback for decoders that expose only IMFMediaBuffer. RGB32 requested
        // through SourceReader video processing is normally tightly packed here.
        let buffer = unsafe { sample.ConvertToContiguousBuffer() }.map_err(|error| {
            format!(
                "startup intro entry '{}' sample buffer conversion failed: {error}",
                entry.id
            )
        })?;
        let mut bytes = std::ptr::null_mut();
        let mut current_len = 0u32;
        unsafe { buffer.Lock(&mut bytes, None, Some(&mut current_len)) }.map_err(|error| {
            format!(
                "startup intro entry '{}' sample buffer lock failed: {error}",
                entry.id
            )
        })?;
        let result = if bytes.is_null() || (current_len as usize) < expected {
            Err(format!(
                "startup intro entry '{}' RGB32 contiguous sample too small: actual={} expected_at_least={}",
                entry.id, current_len, expected
            ))
        } else {
            Ok(unsafe { std::slice::from_raw_parts(bytes, expected) }.to_vec())
        };
        let _ = unsafe { buffer.Unlock() };
        result
    }

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

        fn ensure_backbuffer(
            &mut self,
            window_dc: HDC,
            width: i32,
            height: i32,
        ) -> Result<(), String> {
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

    fn aspect_fit(
        target_w: i32,
        target_h: i32,
        source_w: i32,
        source_h: i32,
    ) -> (i32, i32, i32, i32) {
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
        if unsafe { windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect) }
            .is_err()
        {
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

    fn absolute_source(raw: &str) -> Result<String, String> {
        let path = Path::new(raw);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|error| format!("startup intro current_dir failed: {error}"))?
                .join(path)
        };
        Ok(normalize_media_foundation_path(&path.to_string_lossy()))
    }

    fn normalize_media_foundation_path(raw: &str) -> String {
        if let Some(rest) = raw.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{}", rest);
        }
        raw.strip_prefix(r"\\?\").unwrap_or(raw).to_owned()
    }

    fn pump_messages() {
        let mut message = MSG::default();
        loop {
            let available = unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool();
            if !available {
                return;
            }
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    fn skip_requested() -> bool {
        [VK_ESCAPE, VK_SPACE, VK_RETURN, VK_LBUTTON]
            .into_iter()
            .any(|key| (unsafe { GetAsyncKeyState(key.0 as i32) } as u16 & 0x8000) != 0)
    }

    fn drain_skip_keys() {
        let deadline = Instant::now() + Duration::from_millis(250);
        while skip_requested() && Instant::now() < deadline {
            pump_messages();
            thread::sleep(Duration::from_millis(4));
        }
    }

    fn parse_colorref(raw: &str) -> Option<COLORREF> {
        let value = raw.trim().strip_prefix('#')?;
        if value.len() != 6 {
            return None;
        }
        let rgb = u32::from_str_radix(value, 16).ok()?;
        let r = (rgb >> 16) & 0xff;
        let g = (rgb >> 8) & 0xff;
        let b = rgb & 0xff;
        Some(COLORREF(r | (g << 8) | (b << 16)))
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain([0]).collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn colorref_parser_converts_rgb_to_windows_bgr_layout() {
            assert_eq!(parse_colorref("#112233"), Some(COLORREF(0x00332211)));
            assert_eq!(parse_colorref("112233"), None);
        }

        #[test]
        fn media_foundation_path_strips_extended_dos_namespace() {
            assert_eq!(
                normalize_media_foundation_path(r"\\?\C:\NorthStar\logo.mp4"),
                r"C:\NorthStar\logo.mp4"
            );
        }

        #[test]
        fn media_foundation_path_converts_extended_unc_namespace() {
            assert_eq!(
                normalize_media_foundation_path(r"\\?\UNC\server\share\logo.mp4"),
                r"\\server\share\logo.mp4"
            );
        }

        #[test]
        fn aspect_fit_preserves_square_video_inside_widescreen_window() {
            assert_eq!(aspect_fit(1280, 720, 960, 960), (280, 0, 720, 720));
        }

        #[test]
        fn hundred_nanosecond_timestamps_convert_without_float_drift() {
            assert_eq!(duration_from_hns(10_000_000), Duration::from_secs(1));
            assert_eq!(duration_from_hns(-1), Duration::ZERO);
        }
    }
}
