use crate::{
    ScriptDiagnostic, ScriptDiagnosticSeverity, ScriptModuleState, ScriptingModuleLoadBytesRequest,
    ScriptingModuleLoadBytesResponse, ScriptingModuleRecord, ScriptingModuleRef,
    ScriptingPermission, ScriptingRequestBytes, ScriptingResponseBytes, ScriptingResponseStatus,
};
use std::collections::BTreeMap;

const VERSION_V1: u16 = 1;
const REQUEST_MAGIC: &[u8; 4] = b"NSCR";
const MODULE_LOAD_MAGIC: &[u8; 4] = b"NSML";
const RESPONSE_MAGIC: &[u8; 4] = b"NSRS";
const MODULE_LOAD_RESPONSE_MAGIC: &[u8; 4] = b"NSLR";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptingWireError(pub String);

impl std::fmt::Display for ScriptingWireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ScriptingWireError {}

pub type ScriptingWireResult<T> = Result<T, ScriptingWireError>;

#[inline]
pub fn encode_scripting_request_bytes(request: &ScriptingRequestBytes) -> Vec<u8> {
    let mut out = Vec::new();
    write_header(&mut out, REQUEST_MAGIC);
    write_string(&mut out, &request.request_id);
    write_string(&mut out, &request.script_ref);
    write_string(&mut out, &request.operation);
    write_bytes(&mut out, &request.payload_bytes);
    write_bytes(&mut out, &request.context_bytes);
    write_permissions(&mut out, &request.permissions);
    write_string_map(&mut out, &request.metadata);
    out
}

#[inline]
pub fn decode_scripting_request_bytes(bytes: &[u8]) -> ScriptingWireResult<ScriptingRequestBytes> {
    let mut reader = WireReader::new(bytes, REQUEST_MAGIC)?;
    let request = ScriptingRequestBytes {
        request_id: reader.read_string()?,
        script_ref: reader.read_string()?,
        operation: reader.read_string()?,
        payload_bytes: reader.read_bytes()?,
        context_bytes: reader.read_bytes()?,
        permissions: reader.read_permissions()?,
        metadata: reader.read_string_map()?,
    };
    reader.finish()?;
    Ok(request)
}

#[inline]
pub fn encode_scripting_module_load_bytes_request(
    request: &ScriptingModuleLoadBytesRequest,
) -> Vec<u8> {
    let mut out = Vec::new();
    write_header(&mut out, MODULE_LOAD_MAGIC);
    write_module_ref(&mut out, &request.module_ref);
    write_bytes(&mut out, &request.module_bytes);
    write_permissions(&mut out, &request.permissions);
    write_string_map(&mut out, &request.metadata);
    out
}

#[inline]
pub fn decode_scripting_module_load_bytes_request(
    bytes: &[u8],
) -> ScriptingWireResult<ScriptingModuleLoadBytesRequest> {
    let mut reader = WireReader::new(bytes, MODULE_LOAD_MAGIC)?;
    let request = ScriptingModuleLoadBytesRequest {
        module_ref: reader.read_module_ref()?,
        module_bytes: reader.read_bytes()?,
        permissions: reader.read_permissions()?,
        metadata: reader.read_string_map()?,
    };
    reader.finish()?;
    Ok(request)
}

#[inline]
pub fn encode_scripting_response_bytes(response: &ScriptingResponseBytes) -> Vec<u8> {
    let mut out = Vec::new();
    write_header(&mut out, RESPONSE_MAGIC);
    write_string(&mut out, &response.request_id);
    out.push(response_status_to_u8(response.status));
    write_bytes(&mut out, &response.payload_bytes);
    write_diagnostics(&mut out, &response.diagnostics);
    write_string(&mut out, &response.trace_id);
    write_string_map(&mut out, &response.metadata);
    out
}

#[inline]
pub fn encode_scripting_module_load_bytes_response(
    response: &ScriptingModuleLoadBytesResponse,
) -> Vec<u8> {
    let mut out = Vec::new();
    write_header(&mut out, MODULE_LOAD_RESPONSE_MAGIC);
    out.push(u8::from(response.ok));
    write_module_record(&mut out, &response.module);
    write_diagnostics(&mut out, &response.diagnostics);
    out
}

fn write_header(out: &mut Vec<u8>, magic: &[u8; 4]) {
    out.extend_from_slice(magic);
    out.extend_from_slice(&VERSION_V1.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
}

fn write_string(out: &mut Vec<u8>, value: &str) {
    write_bytes(out, value.as_bytes());
}

fn write_bytes(out: &mut Vec<u8>, value: &[u8]) {
    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
    out.extend_from_slice(value);
}

fn write_permissions(out: &mut Vec<u8>, values: &[ScriptingPermission]) {
    out.extend_from_slice(&(values.len() as u32).to_le_bytes());
    for value in values {
        write_string(out, &value.id);
        write_string(out, &value.scope);
    }
}

fn write_string_map(out: &mut Vec<u8>, values: &BTreeMap<String, String>) {
    out.extend_from_slice(&(values.len() as u32).to_le_bytes());
    for (key, value) in values {
        write_string(out, key);
        write_string(out, value);
    }
}

fn write_module_ref(out: &mut Vec<u8>, value: &ScriptingModuleRef) {
    write_string(out, &value.reference);
    write_string(out, &value.module_id);
}

fn write_diagnostics(out: &mut Vec<u8>, values: &[ScriptDiagnostic]) {
    out.extend_from_slice(&(values.len() as u32).to_le_bytes());
    for value in values {
        out.push(diagnostic_severity_to_u8(value.severity));
        write_string(out, &value.code);
        write_string(out, &value.message);
        write_string(out, &value.script_ref);
        write_bytes(out, &value.payload_bytes);
    }
}

fn write_module_record(out: &mut Vec<u8>, value: &ScriptingModuleRecord) {
    write_string(out, &value.schema);
    write_module_ref(out, &value.module_ref);
    out.push(module_state_to_u8(value.state));
    write_permissions(out, &value.permissions);
    out.extend_from_slice(&value.module_bytes_len.to_le_bytes());
    write_string_map(out, &value.metadata);
    write_diagnostics(out, &value.diagnostics);
}

struct WireReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> WireReader<'a> {
    fn new(bytes: &'a [u8], expected_magic: &[u8; 4]) -> ScriptingWireResult<Self> {
        if bytes.len() < 8 {
            return Err(ScriptingWireError(
                "scripting binary envelope is shorter than header".to_owned(),
            ));
        }
        if bytes.get(0..4) != Some(&expected_magic[..]) {
            return Err(ScriptingWireError(
                "scripting binary envelope magic mismatch".to_owned(),
            ));
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != VERSION_V1 {
            return Err(ScriptingWireError(format!(
                "unsupported scripting binary envelope version {version}"
            )));
        }
        Ok(Self { bytes, offset: 8 })
    }

    fn finish(&self) -> ScriptingWireResult<()> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(ScriptingWireError(format!(
                "scripting binary envelope has trailing bytes: offset={} len={}",
                self.offset,
                self.bytes.len()
            )))
        }
    }

    fn read_u32(&mut self) -> ScriptingWireResult<u32> {
        let slice = self.read_exact(4)?;
        Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
    }

    fn read_exact(&mut self, len: usize) -> ScriptingWireResult<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| ScriptingWireError("scripting binary range overflow".to_owned()))?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| ScriptingWireError("scripting binary envelope truncated".to_owned()))?;
        self.offset = end;
        Ok(slice)
    }

    fn read_bytes(&mut self) -> ScriptingWireResult<Vec<u8>> {
        let len = self.read_u32()? as usize;
        Ok(self.read_exact(len)?.to_vec())
    }

    fn read_string(&mut self) -> ScriptingWireResult<String> {
        String::from_utf8(self.read_bytes()?).map_err(|e| {
            ScriptingWireError(format!(
                "invalid utf-8 string in scripting binary envelope: {e}"
            ))
        })
    }

    fn read_permissions(&mut self) -> ScriptingWireResult<Vec<ScriptingPermission>> {
        let count = self.read_u32()? as usize;
        let mut out = Vec::with_capacity(count.min(1024));
        for _ in 0..count {
            out.push(ScriptingPermission {
                id: self.read_string()?,
                scope: self.read_string()?,
            });
        }
        Ok(out)
    }

    fn read_string_map(&mut self) -> ScriptingWireResult<BTreeMap<String, String>> {
        let count = self.read_u32()? as usize;
        let mut out = BTreeMap::new();
        for _ in 0..count {
            out.insert(self.read_string()?, self.read_string()?);
        }
        Ok(out)
    }

    fn read_module_ref(&mut self) -> ScriptingWireResult<ScriptingModuleRef> {
        Ok(ScriptingModuleRef {
            reference: self.read_string()?,
            module_id: self.read_string()?,
        })
    }
}

#[inline]
fn response_status_to_u8(value: ScriptingResponseStatus) -> u8 {
    match value {
        ScriptingResponseStatus::Ok => 0,
        ScriptingResponseStatus::Empty => 1,
        ScriptingResponseStatus::Rejected => 2,
        ScriptingResponseStatus::InvalidRequest => 3,
        ScriptingResponseStatus::ProviderError => 4,
    }
}

#[inline]
fn diagnostic_severity_to_u8(value: ScriptDiagnosticSeverity) -> u8 {
    match value {
        ScriptDiagnosticSeverity::Trace => 0,
        ScriptDiagnosticSeverity::Info => 1,
        ScriptDiagnosticSeverity::Warning => 2,
        ScriptDiagnosticSeverity::Error => 3,
    }
}

#[inline]
fn module_state_to_u8(value: ScriptModuleState) -> u8 {
    match value {
        ScriptModuleState::Declared => 0,
        ScriptModuleState::Loaded => 1,
        ScriptModuleState::Disabled => 2,
        ScriptModuleState::Failed => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_wire_roundtrips() {
        let request = ScriptingRequestBytes {
            request_id: "r1".to_owned(),
            script_ref: "scripts/foo.ysc@main".to_owned(),
            operation: "frame".to_owned(),
            payload_bytes: vec![1, 2, 3],
            ..ScriptingRequestBytes::default()
        };
        let decoded =
            decode_scripting_request_bytes(&encode_scripting_request_bytes(&request)).unwrap();
        assert_eq!(decoded.request_id, request.request_id);
        assert_eq!(decoded.script_ref, request.script_ref);
        assert_eq!(decoded.payload_bytes, request.payload_bytes);
    }

    #[test]
    fn module_load_wire_roundtrips() {
        let request = ScriptingModuleLoadBytesRequest {
            module_ref: ScriptingModuleRef::new("scripts/foo.ysc@main"),
            module_bytes: vec![9, 8, 7],
            ..ScriptingModuleLoadBytesRequest::default()
        };
        let decoded = decode_scripting_module_load_bytes_request(
            &encode_scripting_module_load_bytes_request(&request),
        )
        .unwrap();
        assert_eq!(decoded.module_ref.reference, request.module_ref.reference);
        assert_eq!(decoded.module_bytes, request.module_bytes);
    }
}
