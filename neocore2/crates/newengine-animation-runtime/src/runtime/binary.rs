#[inline]
fn checked_slice<'a>(
    bytes: &'a [u8],
    offset: usize,
    len: usize,
    label: &str,
) -> Result<&'a [u8], String> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| format!("YCD {label} range overflow"))?;
    bytes
        .get(offset..end)
        .ok_or_else(|| format!("YCD {label} outside body offset={offset} len={len}"))
}

#[inline]
fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        checked_slice(bytes, offset, 4, "u32")?
            .try_into()
            .expect("u32 slice"),
    ))
}

#[inline]
fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        checked_slice(bytes, offset, 8, "u64")?
            .try_into()
            .expect("u64 slice"),
    ))
}

#[inline]
fn read_f32(bytes: &[u8], offset: usize) -> Result<f32, String> {
    let value = f32::from_le_bytes(
        checked_slice(bytes, offset, 4, "f32")?
            .try_into()
            .expect("f32 slice"),
    );
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("YCD contains non-finite f32 at {offset}"))
    }
}

#[inline]
fn usize_from_u64(value: u64, label: &str) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| format!("YCD {label} exceeds usize"))
}

fn read_string(strings: &[u8], offset: u32) -> Result<String, String> {
    let start = offset as usize;
    let tail = strings
        .get(start..)
        .ok_or_else(|| format!("YCD string offset outside table offset={offset}"))?;
    let len = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| format!("YCD string not terminated offset={offset}"))?;
    String::from_utf8(tail[..len].to_vec())
        .map_err(|error| format!("YCD string is not UTF-8: {error}"))
}
