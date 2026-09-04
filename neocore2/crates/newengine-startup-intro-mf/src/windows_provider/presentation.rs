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
