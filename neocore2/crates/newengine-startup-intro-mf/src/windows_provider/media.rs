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
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }
            .map_err(|error| format!("startup intro Media Foundation startup failed: {error}"))?;
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

    let reader = unsafe { MFCreateSourceReaderFromURL(PCWSTR(source_wide.as_ptr()), &attributes) }
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
    unsafe { reader.SetCurrentMediaType(video_stream, None, &media_type) }.map_err(|error| {
        format!(
            "startup intro entry '{}' requesting RGB32 decode failed: {error}",
            entry.id
        )
    })?;
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
