#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};

use crate::asset_access::{AssetAccess, AssetService, AssetState};
use crate::consts::{method, ASSET_SERVICE_ID};

/// Thin client over the engine AssetManager service.
///
/// The AssetManager is an engine service (typically provided by a runtime plugin).
/// This client performs service calls through `HostApiV1` and does not link
/// against any concrete AssetManager implementation.
#[derive(Clone)]
pub struct AssetServiceClient {
    host: HostApiV1,
    service_id: RString,

    // Hot path: avoid per-call allocations for common method names.
    m_load: RString,
    m_pump: RString,
    m_state_json: RString,
    m_blob_wire_v1: RString,
    m_reload: RString,
    m_info_json: RString,
    m_formats_json: RString,
    m_sources_json: RString,
    m_mount_pak: RString,
    m_mount_dir: RString,
    m_mount_pak_prio: RString,
    m_mount_dir_prio: RString,
    m_resolve_trace_json: RString,
}

impl AssetServiceClient {
    /// Create a client bound to the given host API.
    ///
    /// Service id defaults to [`ASSET_SERVICE_ID`] and may be overridden via
    /// `NEWENGINE_ASSET_SERVICE_ID`.
    #[inline]
    pub fn new(host: HostApiV1) -> Self {
        let service_id = std::env::var("NEWENGINE_ASSET_SERVICE_ID")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| ASSET_SERVICE_ID.to_string());

        Self {
            host,
            service_id: RString::from(service_id),

            // Prefer the canonical v1 names from `consts::method`.
            m_load: RString::from(method::LOAD),
            m_pump: RString::from(method::PUMP),
            m_state_json: RString::from(method::STATE_JSON),
            m_blob_wire_v1: RString::from(method::BLOB_WIRE_V1),
            m_reload: RString::from(method::RELOAD),
            m_info_json: RString::from(method::INFO_JSON),
            m_formats_json: RString::from(method::FORMATS_JSON),
            m_sources_json: RString::from(method::SOURCES_JSON),
            m_mount_pak: RString::from(method::MOUNT_PAK),
            m_mount_dir: RString::from(method::MOUNT_DIR),
            m_mount_pak_prio: RString::from(method::MOUNT_PAK_PRIO),
            m_mount_dir_prio: RString::from(method::MOUNT_DIR_PRIO),
            m_resolve_trace_json: RString::from(method::RESOLVE_TRACE_JSON),
        }
    }

    #[inline]
    pub fn service_id(&self) -> &RString {
        &self.service_id
    }

    #[inline]

    fn call_method(&self, method_name: &RString, payload: &[u8]) -> Result<Vec<u8>, String> {
        let res = (self.host.call_service_v1)(
            self.service_id.clone(),
            MethodName::from(method_name.clone()),
            Blob::from(payload.to_vec()),
        );

        res.into_result()
            .map(|v| v.into_vec())
            .map_err(|e| e.to_string())
    }

    // NOTE: we intentionally do not provide method fallbacks here.
    // This client is part of the engine<->plugin contract and must be deterministic.

    #[inline]
    fn decode_utf8(bytes: Vec<u8>) -> Result<String, String> {
        String::from_utf8(bytes).map_err(|_| "asset service returned non-utf8".to_string())
    }

    #[inline]
    fn decode_state_str(s: &str) -> AssetState {
        match s.trim().to_ascii_lowercase().as_str() {
            "ready" => AssetState::Ready,
            "failed" => AssetState::Failed,
            "unloaded" => AssetState::Unloaded,
            _ => AssetState::Loading,
        }
    }

    #[inline]
    fn parse_json(s: &str) -> Result<serde_json::Value, String> {
        serde_json::from_str::<serde_json::Value>(s).map_err(|e| e.to_string())
    }

    fn decode_ok_json(bytes: Vec<u8>) -> Result<serde_json::Value, String> {
        let s = Self::decode_utf8(bytes)?;
        let v = Self::parse_json(&s)?;
        Ok(v)
    }

    fn decode_load_like(bytes: Vec<u8>, op: &'static str) -> Result<String, String> {
        // Contract: json { ok, id_u128, error }
        // Fallback: plain string id
        let s = Self::decode_utf8(bytes)?;
        if let Ok(v) = Self::parse_json(&s) {
            let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
            if !ok {
                let err = v
                    .get("error")
                    .and_then(|x| x.as_str())
                    .unwrap_or("operation failed");
                return Err(err.to_string());
            }
            let id = v
                .get("id_u128")
                .and_then(|x| x.as_str())
                .ok_or_else(|| format!("{op}: missing id_u128"))?;
            return Ok(id.trim().to_string());
        }

        let id = s.trim();
        if id.is_empty() {
            return Err(format!("{op}: empty response"));
        }
        Ok(id.to_string())
    }

    fn decode_state_json_response(bytes: Vec<u8>) -> Result<AssetState, String> {
        // Contract: json { ok, state, error }
        // Fallback: plain string state
        let s = Self::decode_utf8(bytes)?;
        if let Ok(v) = Self::parse_json(&s) {
            let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
            if !ok {
                let err = v
                    .get("error")
                    .and_then(|x| x.as_str())
                    .unwrap_or("state failed");
                return Err(err.to_string());
            }
            let st = v.get("state").and_then(|x| x.as_str()).unwrap_or("invalid");
            return Ok(Self::decode_state_str(st));
        }

        Ok(Self::decode_state_str(&s))
    }

    fn decode_blob_wire_v1(bytes: Vec<u8>) -> Result<(String, Vec<u8>), String> {
        // Contract: wire_v1 = u32(le) meta_len + meta_json_bytes + payload_bytes
        if bytes.len() < 4 {
            return Err("blob_wire_v1: short frame".to_string());
        }
        let meta_len = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        if 4 + meta_len > bytes.len() {
            return Err("blob_wire_v1: bad meta_len".to_string());
        }

        let meta_bytes = &bytes[4..4 + meta_len];
        let payload = bytes[4 + meta_len..].to_vec();

        let meta_json = std::str::from_utf8(meta_bytes)
            .map_err(|_| "blob_wire_v1: meta is not utf8".to_string())?
            .to_string();

        Ok((meta_json, payload))
    }

    fn decode_ok_unit(bytes: Vec<u8>) -> Result<(), String> {
        if bytes.is_empty() {
            return Ok(());
        }
        let s = Self::decode_utf8(bytes)?;
        if let Ok(v) = Self::parse_json(&s) {
            let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
            if ok {
                return Ok(());
            }
            let err = v
                .get("error")
                .and_then(|x| x.as_str())
                .unwrap_or("operation failed");
            return Err(err.to_string());
        }
        Ok(())
    }
}

impl AssetAccess for AssetServiceClient {
    fn load(&self, logical_path: &str) -> Result<String, String> {
        // Contract payload: utf8 logical_path
        let bytes = self.call_method(&self.m_load, logical_path.as_bytes())?;
        Self::decode_load_like(bytes, "load")
    }

    fn pump(&self) {
        // Contract payload: empty
        let _ = self.call_method(&self.m_pump, &[]);
    }

    fn state(&self, id_hex32: &str) -> Result<AssetState, String> {
        // Contract payload: utf8 id_u128_hex32
        let bytes = self.call_method(&self.m_state_json, id_hex32.as_bytes())?;
        Self::decode_state_json_response(bytes)
    }

    fn blob_wire_v1(&self, id_hex32: &str) -> Result<(String, Vec<u8>), String> {
        // Contract payload: utf8 id_u128_hex32
        let bytes = self.call_method(&self.m_blob_wire_v1, id_hex32.as_bytes())?;
        Self::decode_blob_wire_v1(bytes)
    }
}

impl AssetService for AssetServiceClient {
    fn reload(&self, logical_path: &str) -> Result<String, String> {
        let bytes = self.call_method(&self.m_reload, logical_path.as_bytes())?;
        Self::decode_load_like(bytes, "reload")
    }

    fn info_json(&self, logical_path: &str) -> Result<serde_json::Value, String> {
        let bytes = self.call_method(&self.m_info_json, logical_path.as_bytes())?;
        Self::decode_ok_json(bytes)
    }

    fn formats_json(&self) -> Result<serde_json::Value, String> {
        let bytes = self.call_method(&self.m_formats_json, &[])?;
        Self::decode_ok_json(bytes)
    }

    fn sources_json(&self) -> Result<serde_json::Value, String> {
        let bytes = self.call_method(&self.m_sources_json, &[])?;
        Self::decode_ok_json(bytes)
    }

    fn mount_pak(&self, path_to_pak: &str) -> Result<(), String> {
        let bytes = self.call_method(&self.m_mount_pak, path_to_pak.as_bytes())?;
        Self::decode_ok_unit(bytes)
    }

    fn mount_dir(&self, path_to_dir: &str) -> Result<(), String> {
        let bytes = self.call_method(&self.m_mount_dir, path_to_dir.as_bytes())?;
        Self::decode_ok_unit(bytes)
    }

    fn mount_pak_prio(&self, path_to_pak: &str, priority: i32) -> Result<(), String> {
        // Contract payload: json { path, priority }
        let m0 = &self.m_mount_pak_prio;
        let payload = serde_json::json!({
            "path": path_to_pak,
            "priority": priority
        })
            .to_string()
            .into_bytes();
        let bytes = self.call_method(m0, &payload)?;
        Self::decode_ok_unit(bytes)
    }

    fn mount_dir_prio(&self, path_to_dir: &str, priority: i32) -> Result<(), String> {
        // Contract payload: json { path, priority }
        let m0 = &self.m_mount_dir_prio;
        let payload = serde_json::json!({
            "path": path_to_dir,
            "priority": priority
        })
            .to_string()
            .into_bytes();
        let bytes = self.call_method(m0, &payload)?;
        Self::decode_ok_unit(bytes)
    }

    fn resolve_trace_json(&self, logical_path: &str) -> Result<serde_json::Value, String> {
        let bytes = self.call_method(&self.m_resolve_trace_json, logical_path.as_bytes())?;
        Self::decode_ok_json(bytes)
    }
}
