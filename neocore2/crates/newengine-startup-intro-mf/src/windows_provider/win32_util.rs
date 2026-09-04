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
