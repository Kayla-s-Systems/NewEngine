use crate::{UiFrameRequest, UiFrameResponse};

const UI_FRAME_REQUEST_BIN_MAGIC: &[u8; 8] = b"NEUIRQ1\0";
const UI_FRAME_RESPONSE_BIN_MAGIC: &[u8; 8] = b"NEUIRS1\0";

/// Encodes the stable v1 binary frame request.
///
/// P2A live-frame fields are intentionally carried by JSON today and are
/// reconstructed from the v1 header on binary decode. This keeps existing
/// release providers from rejecting v1 packets because of trailing bytes.
pub fn encode_ui_frame_request_bin(request: &UiFrameRequest) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(40 + request.render_surface_ids.len() * 24);
    out.extend_from_slice(UI_FRAME_REQUEST_BIN_MAGIC);
    put_u32(&mut out, request.version);
    put_u64(&mut out, request.frame_index);
    put_f32(&mut out, request.dt_sec);
    put_u32(&mut out, request.surface_size_px[0]);
    put_u32(&mut out, request.surface_size_px[1]);
    put_f32(&mut out, request.pixels_per_point.max(0.0001));
    put_string_vec(
        &mut out,
        &request.render_surface_ids,
        "ui frame render surface ids",
    )?;
    Ok(out)
}

pub fn decode_ui_frame_request_bin(bytes: &[u8]) -> Result<UiFrameRequest, String> {
    let mut r = BinReader::new(bytes);
    let magic = r.take(8)?;
    if magic != UI_FRAME_REQUEST_BIN_MAGIC {
        return Err("ui frame request binary packet has invalid magic".to_owned());
    }
    let version = r.u32()?;
    let frame_index = r.u64()?;
    let dt_sec = r.f32()?;
    let surface_size_px = [r.u32()?, r.u32()?];
    let pixels_per_point = r.f32()?.max(0.0001);
    let render_surface_ids = if r.is_eof() {
        Vec::new()
    } else {
        r.string_vec()?
    };
    if !r.is_eof() {
        return Err("ui frame request binary packet has trailing bytes".to_owned());
    }
    Ok(
        UiFrameRequest::new(frame_index, dt_sec, surface_size_px, pixels_per_point)
            .with_render_surface_ids(render_surface_ids)
            .with_diagnostics_flags(Vec::new())
            .with_version_for_binary(version),
    )
}

/// Encodes a provider-produced draw-list response without JSON.
pub fn encode_ui_frame_response_bin(response: &UiFrameResponse) -> Result<Vec<u8>, String> {
    let draw_list = newengine_ui_draw::encode_ui_draw_list_bin(&response.draw_list)?;
    let input_capture = serde_json::to_vec(&response.input_capture)
        .map_err(|e| format!("encode ui frame response input-capture failed: {e}"))?;
    let mut out = Vec::with_capacity(20 + draw_list.len() + input_capture.len());
    out.extend_from_slice(UI_FRAME_RESPONSE_BIN_MAGIC);
    put_u32(&mut out, response.version);
    put_bytes(&mut out, &draw_list, "ui frame response draw-list")?;
    put_bytes(&mut out, &input_capture, "ui frame response input-capture")?;
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
    let input_capture = if r.is_eof() {
        crate::UiInputCaptureState::none()
    } else {
        let input_capture_bytes = r.bytes_vec()?;
        if !r.is_eof() {
            return Err("ui frame response binary packet has trailing bytes".to_owned());
        }
        serde_json::from_slice(&input_capture_bytes)
            .map_err(|e| format!("decode ui frame response input-capture failed: {e}"))?
    };
    let draw_list = newengine_ui_draw::decode_ui_draw_list_bin(&draw_list_bytes)?;
    let mut response = UiFrameResponse::new(draw_list);
    response.version = version;
    response.input_capture = input_capture;
    Ok(response)
}

trait UiFrameRequestBinaryVersionExt {
    fn with_version_for_binary(self, version: u32) -> Self;
}

impl UiFrameRequestBinaryVersionExt for UiFrameRequest {
    #[inline]
    fn with_version_for_binary(mut self, version: u32) -> Self {
        self.version = version;
        self.frame_input.version = version.max(1);
        self
    }
}

#[inline]
fn put_bytes(out: &mut Vec<u8>, bytes: &[u8], what: &str) -> Result<(), String> {
    let len = u32::try_from(bytes.len())
        .map_err(|_| format!("{what} is too large for ui frame binary packet"))?;
    put_u32(out, len);
    out.extend_from_slice(bytes);
    Ok(())
}

fn put_string_vec(out: &mut Vec<u8>, values: &[String], what: &str) -> Result<(), String> {
    let len = u32::try_from(values.len()).map_err(|_| format!("{what} count is too large"))?;
    put_u32(out, len);
    for value in values {
        put_bytes(out, value.as_bytes(), what)?;
    }
    Ok(())
}

#[inline]
fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
#[inline]
fn put_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}
#[inline]
fn put_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_le_bytes());
}

struct BinReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> BinReader<'a> {
    #[inline]
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }
    #[inline]
    fn is_eof(&self) -> bool {
        self.cursor == self.bytes.len()
    }

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
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
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

    fn string_vec(&mut self) -> Result<Vec<String>, String> {
        let len = self.u32()? as usize;
        let mut out = Vec::with_capacity(len.min(64));
        for _ in 0..len {
            let bytes = self.bytes_vec()?;
            let value = String::from_utf8(bytes)
                .map_err(|e| format!("ui frame request render surface id is not utf8: {e}"))?;
            let value = value.trim().to_owned();
            if !value.is_empty() {
                out.push(value);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{UiFrameRequest, UiFrameResponse};

    #[test]
    fn ui_frame_request_binary_roundtrips_v1_surface_fields() {
        let request = UiFrameRequest::new(42, 0.016, [1600, 900], 1.5)
            .with_now_ms(123_456)
            .with_render_surface_ids(vec!["apps.aurelia_ui_test.main".to_owned()])
            .with_diagnostics_flags(vec!["font".to_owned(), "frame".to_owned()]);
        let bytes = encode_ui_frame_request_bin(&request).unwrap();
        let decoded = decode_ui_frame_request_bin(&bytes).unwrap();
        assert_eq!(decoded.frame_index, request.frame_index);
        assert_eq!(decoded.surface_size_px, request.surface_size_px);
        assert_eq!(decoded.render_surface_ids, request.render_surface_ids);
        assert!((decoded.pixels_per_point - 1.5).abs() < f32::EPSILON);

        let live = decoded.live_input();
        assert_eq!(live.frame_index, 42);
        assert_eq!(live.viewport_px, [1600, 900]);
        assert_eq!(
            live.render_surface_ids,
            vec!["apps.aurelia_ui_test.main".to_owned()]
        );
        assert_eq!(
            live.now_ms, 0,
            "v1 binary intentionally does not append P2A JSON-only fields"
        );
        assert!(
            live.diagnostics_flags.is_empty(),
            "v1 binary keeps compatibility with existing release providers"
        );
    }

    #[test]
    fn ui_frame_request_json_preserves_live_input_contract() {
        let request = UiFrameRequest::new(7, 0.033, [1920, 1080], 2.0)
            .with_now_ms(987_654)
            .with_render_surface_ids(vec!["surface.main".to_owned()])
            .with_diagnostics_flags(vec!["font.resolve".to_owned(), "caret".to_owned()]);
        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: UiFrameRequest = serde_json::from_slice(&bytes).unwrap();
        let live = decoded.live_input();
        assert_eq!(live.frame_index, 7);
        assert_eq!(live.now_ms, 987_654);
        assert_eq!(live.dt_sec, 0.033);
        assert_eq!(live.viewport_px, [1920, 1080]);
        assert_eq!(live.pixels_per_point, 2.0);
        assert_eq!(live.render_surface_ids, vec!["surface.main".to_owned()]);
        assert_eq!(
            live.diagnostics_flags,
            vec!["font.resolve".to_owned(), "caret".to_owned()]
        );
    }

    #[test]
    fn ui_frame_response_binary_roundtrips() {
        let response = UiFrameResponse::new(newengine_ui_draw::UiDrawList::new());
        let bytes = encode_ui_frame_response_bin(&response).unwrap();
        let decoded = decode_ui_frame_response_bin(&bytes).unwrap();
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.draw_list.screen_size_px, [0, 0]);
        assert!(decoded.diagnostics.font_resolve.is_empty());
        assert!(!decoded.input_capture.requests_capture());
    }

    #[test]
    fn ui_frame_response_binary_roundtrips_input_capture() {
        let mut response = UiFrameResponse::new(newengine_ui_draw::UiDrawList::new());
        response.input_capture = crate::UiInputCaptureState::modal("surface.main", "test modal");
        let bytes = encode_ui_frame_response_bin(&response).unwrap();
        let decoded = decode_ui_frame_response_bin(&bytes).unwrap();
        assert!(decoded.input_capture.requests_capture());
        assert_eq!(decoded.input_capture.surfaces, vec!["surface.main".to_owned()]);
    }
}
