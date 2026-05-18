use crate::{UiFrameRequest, UiFrameResponse};

const UI_FRAME_REQUEST_BIN_MAGIC: &[u8; 8] = b"NEUIRQ1\0";
const UI_FRAME_RESPONSE_BIN_MAGIC: &[u8; 8] = b"NEUIRS1\0";

/// Encodes the small per-frame UI request without JSON.
pub fn encode_ui_frame_request_bin(request: &UiFrameRequest) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(32);
    out.extend_from_slice(UI_FRAME_REQUEST_BIN_MAGIC);
    put_u32(&mut out, request.version);
    put_u64(&mut out, request.frame_index);
    put_f32(&mut out, request.dt_sec);
    put_u32(&mut out, request.surface_size_px[0]);
    put_u32(&mut out, request.surface_size_px[1]);
    put_f32(&mut out, request.pixels_per_point.max(0.0001));
    Ok(out)
}

pub fn decode_ui_frame_request_bin(bytes: &[u8]) -> Result<UiFrameRequest, String> {
    let mut r = BinReader::new(bytes);
    let magic = r.take(8)?;
    if magic != UI_FRAME_REQUEST_BIN_MAGIC {
        return Err("ui frame request binary packet has invalid magic".to_owned());
    }
    let request = UiFrameRequest {
        version: r.u32()?,
        frame_index: r.u64()?,
        dt_sec: r.f32()?,
        surface_size_px: [r.u32()?, r.u32()?],
        pixels_per_point: r.f32()?.max(0.0001),
    };
    if !r.is_eof() {
        return Err("ui frame request binary packet has trailing bytes".to_owned());
    }
    Ok(request)
}

/// Encodes a provider-produced draw-list response without JSON.
pub fn encode_ui_frame_response_bin(response: &UiFrameResponse) -> Result<Vec<u8>, String> {
    let draw_list = newengine_ui_draw::encode_ui_draw_list_bin(&response.draw_list)?;
    let mut out = Vec::with_capacity(16 + draw_list.len());
    out.extend_from_slice(UI_FRAME_RESPONSE_BIN_MAGIC);
    put_u32(&mut out, response.version);
    put_bytes(&mut out, &draw_list, "ui frame response draw-list")?;
    Ok(out)
}

pub fn decode_ui_frame_response_bin(bytes: &[u8]) -> Result<UiFrameResponse, String> {
    let mut r = BinReader::new(bytes);
    let magic = r.take(8)?;
    if magic != UI_FRAME_RESPONSE_BIN_MAGIC {
        return Err("ui frame response binary packet has invalid magic".to_owned());
    }
    let version = r.u32()?;
    let draw_list_bytes = r.bytes_vec()?;
    if !r.is_eof() {
        return Err("ui frame response binary packet has trailing bytes".to_owned());
    }
    let draw_list = newengine_ui_draw::decode_ui_draw_list_bin(&draw_list_bytes)?;
    Ok(UiFrameResponse { version, draw_list })
}

#[inline]
fn put_bytes(out: &mut Vec<u8>, bytes: &[u8], what: &str) -> Result<(), String> {
    let len = u32::try_from(bytes.len()).map_err(|_| format!("{what} is too large for ui frame binary packet"))?;
    put_u32(out, len);
    out.extend_from_slice(bytes);
    Ok(())
}

#[inline]
fn put_u32(out: &mut Vec<u8>, v: u32) { out.extend_from_slice(&v.to_le_bytes()); }
#[inline]
fn put_u64(out: &mut Vec<u8>, v: u64) { out.extend_from_slice(&v.to_le_bytes()); }
#[inline]
fn put_f32(out: &mut Vec<u8>, v: f32) { out.extend_from_slice(&v.to_le_bytes()); }

struct BinReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> BinReader<'a> {
    #[inline]
    fn new(bytes: &'a [u8]) -> Self { Self { bytes, cursor: 0 } }
    #[inline]
    fn is_eof(&self) -> bool { self.cursor == self.bytes.len() }

    fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self.cursor.saturating_add(len);
        if end > self.bytes.len() {
            return Err("ui frame binary packet ended early".to_owned());
        }
        let out = &self.bytes[self.cursor..end];
        self.cursor = end;
        Ok(out)
    }

    #[inline]
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    #[inline]
    fn u64(&mut self) -> Result<u64, String> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }

    #[inline]
    fn f32(&mut self) -> Result<f32, String> {
        let b = self.take(4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn bytes_vec(&mut self) -> Result<Vec<u8>, String> {
        let len = self.u32()? as usize;
        Ok(self.take(len)?.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{UiFrameRequest, UiFrameResponse};

    #[test]
    fn ui_frame_request_binary_roundtrips() {
        let request = UiFrameRequest::new(42, 0.016, [1600, 900], 1.5);
        let bytes = encode_ui_frame_request_bin(&request).unwrap();
        let decoded = decode_ui_frame_request_bin(&bytes).unwrap();
        assert_eq!(decoded.frame_index, request.frame_index);
        assert_eq!(decoded.surface_size_px, request.surface_size_px);
        assert!((decoded.pixels_per_point - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn ui_frame_response_binary_roundtrips() {
        let response = UiFrameResponse::new(newengine_ui_draw::UiDrawList::new());
        let bytes = encode_ui_frame_response_bin(&response).unwrap();
        let decoded = decode_ui_frame_response_bin(&bytes).unwrap();
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.draw_list.screen_size_px, [0, 0]);
    }
}
