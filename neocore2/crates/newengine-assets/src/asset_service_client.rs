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
        }
    }

    #[inline]
    pub fn service_id(&self) -> &RString {
        &self.service_id
    }

    #[inline]
    fn call(&self, method_name: &'static str, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        let res = (self.host.call_service_v1)(
            self.service_id.clone(),
            MethodName::from(method_name),
            Blob::from(payload),
        );

        res.into_result()
            .map(|v| v.into_vec())
            .map_err(|e| e.to_string())
    }

    #[inline]
    fn call_try_methods(&self, methods: &[&'static str], payload: Vec<u8>) -> Result<Vec<u8>, String> {
        let mut last_err: Option<String> = None;

        for &m in methods {
            match self.call(m, payload.clone()) {
                Ok(v) => return Ok(v),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| "asset service call failed".to_string()))
    }

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
        let methods = [method::LOAD, "load", "asset.load"];
        let bytes = self.call_try_methods(&methods, logical_path.as_bytes().to_vec())?;
        Self::decode_load_like(bytes, "load")
    }

    fn pump(&self) {
        // Contract payload: empty
        let methods = [method::PUMP, "pump", "asset.pump", "tick", "asset.tick"];
        let _ = self.call_try_methods(&methods, Vec::new());
    }

    fn state(&self, id_hex32: &str) -> Result<AssetState, String> {
        // Contract payload: utf8 id_u128_hex32
        let methods = [method::STATE_JSON, "state_json", "asset.state_json"];
        let bytes = self.call_try_methods(&methods, id_hex32.as_bytes().to_vec())?;
        Self::decode_state_json_response(bytes)
    }

    fn blob_wire_v1(&self, id_hex32: &str) -> Result<(String, Vec<u8>), String> {
        // Contract payload: utf8 id_u128_hex32
        let methods = [method::BLOB_WIRE_V1, "blob_wire_v1", "asset.blob_wire_v1"];
        let bytes = self.call_try_methods(&methods, id_hex32.as_bytes().to_vec())?;
        Self::decode_blob_wire_v1(bytes)
    }
}

impl AssetService for AssetServiceClient {
    fn reload(&self, logical_path: &str) -> Result<String, String> {
        let methods = [method::RELOAD, "reload", "asset.reload"];
        let bytes = self.call_try_methods(&methods, logical_path.as_bytes().to_vec())?;
        Self::decode_load_like(bytes, "reload")
    }

    fn info_json(&self, logical_path: &str) -> Result<serde_json::Value, String> {
        let methods = [method::INFO_JSON, "info_json", "asset.info_json"];
        let bytes = self.call_try_methods(&methods, logical_path.as_bytes().to_vec())?;
        Self::decode_ok_json(bytes)
    }

    fn formats_json(&self) -> Result<serde_json::Value, String> {
        let methods = [method::FORMATS_JSON, "formats_json", "asset.formats_json"];
        let bytes = self.call_try_methods(&methods, Vec::new())?;
        Self::decode_ok_json(bytes)
    }

    fn sources_json(&self) -> Result<serde_json::Value, String> {
        let methods = [method::SOURCES_JSON, "sources_json", "asset.sources_json"];
        let bytes = self.call_try_methods(&methods, Vec::new())?;
        Self::decode_ok_json(bytes)
    }

    fn mount_pak(&self, path_to_pak: &str) -> Result<(), String> {
        let methods = [method::MOUNT_PAK, "mount_pak", "asset.mount_pak"];
        let bytes = self.call_try_methods(&methods, path_to_pak.as_bytes().to_vec())?;
        Self::decode_ok_unit(bytes)
    }

    fn mount_dir(&self, path_to_dir: &str) -> Result<(), String> {
        let methods = [method::MOUNT_DIR, "mount_dir", "asset.mount_dir"];
        let bytes = self.call_try_methods(&methods, path_to_dir.as_bytes().to_vec())?;
        Self::decode_ok_unit(bytes)
    }

    fn mount_pak_prio(&self, path_to_pak: &str, priority: i32) -> Result<(), String> {
        // Contract payload: json { path, priority }
        let methods = [method::MOUNT_PAK_PRIO, "mount_pak_prio", "asset.mount_pak_prio"];
        let payload = serde_json::json!({
            "path": path_to_pak,
            "priority": priority
        })
            .to_string()
            .into_bytes();
        let bytes = self.call_try_methods(&methods, payload)?;
        Self::decode_ok_unit(bytes)
    }

    fn mount_dir_prio(&self, path_to_dir: &str, priority: i32) -> Result<(), String> {
        // Contract payload: json { path, priority }
        let methods = [method::MOUNT_DIR_PRIO, "mount_dir_prio", "asset.mount_dir_prio"];
        let payload = serde_json::json!({
            "path": path_to_dir,
            "priority": priority
        })
            .to_string()
            .into_bytes();
        let bytes = self.call_try_methods(&methods, payload)?;
        Self::decode_ok_unit(bytes)
    }

    fn resolve_trace_json(&self, logical_path: &str) -> Result<serde_json::Value, String> {
        let methods = [
            method::RESOLVE_TRACE_JSON,
            "resolve_trace_json",
            "asset.resolve_trace_json",
        ];
        let bytes = self.call_try_methods(&methods, logical_path.as_bytes().to_vec())?;
        Self::decode_ok_json(bytes)
    }
}
