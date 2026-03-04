use super::error::{MaterialBinaryError, MaterialBinaryResult};

#[inline]
pub(crate) fn push_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

#[inline]
pub(crate) fn push_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

#[inline]
pub(crate) fn push_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Pads the output buffer to the next 4-byte boundary with zero bytes.
#[inline]
pub(crate) fn pad_to_4(out: &mut Vec<u8>) {
    let padded_len = round_up_4(out.len());
    if padded_len > out.len() {
        out.resize(padded_len, 0);
    }
}

/// Advances an offset to the next 4-byte boundary.
#[inline]
pub(crate) fn skip_padding_4(bytes: &[u8], off: &mut usize) -> MaterialBinaryResult<()> {
    let padded = round_up_4(*off);
    if padded > bytes.len() {
        return Err(MaterialBinaryError::UnexpectedEof);
    }
    *off = padded;
    Ok(())
}

/// Rounds a byte count up to the next 4-byte boundary.
#[inline]
pub(crate) const fn round_up_4(v: usize) -> usize {
    (v + 3) & !3
}

#[inline]
pub(crate) fn read_u8(bytes: &[u8], off: &mut usize) -> MaterialBinaryResult<u8> {
    if *off + 1 > bytes.len() {
        return Err(MaterialBinaryError::UnexpectedEof);
    }
    let v = bytes[*off];
    *off += 1;
    Ok(v)
}

#[inline]
pub(crate) fn read_u16(bytes: &[u8], off: &mut usize) -> MaterialBinaryResult<u16> {
    if *off + 2 > bytes.len() {
        return Err(MaterialBinaryError::UnexpectedEof);
    }
    let v = u16::from_le_bytes([bytes[*off], bytes[*off + 1]]);
    *off += 2;
    Ok(v)
}

#[inline]
pub(crate) fn read_u32(bytes: &[u8], off: &mut usize) -> MaterialBinaryResult<u32> {
    if *off + 4 > bytes.len() {
        return Err(MaterialBinaryError::UnexpectedEof);
    }
    let v = u32::from_le_bytes([
        bytes[*off],
        bytes[*off + 1],
        bytes[*off + 2],
        bytes[*off + 3],
    ]);
    *off += 4;
    Ok(v)
}

#[inline]
pub(crate) fn read_f32(bytes: &[u8], off: &mut usize) -> MaterialBinaryResult<f32> {
    if *off + 4 > bytes.len() {
        return Err(MaterialBinaryError::UnexpectedEof);
    }
    let v = f32::from_le_bytes([
        bytes[*off],
        bytes[*off + 1],
        bytes[*off + 2],
        bytes[*off + 3],
    ]);
    *off += 4;
    Ok(v)
}
