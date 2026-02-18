#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};

use crate::asset_access::{AssetAccess, AssetState};

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
    // Contract-first method names (per newengine-AssetManager).
    const M_LOAD: &'static str = "asset.load";
    const M_PUMP: &'static str = "asset.pump";
    const M_STATE_JSON: &'static str = "asset.state_json";
    const M_BLOB_WIRE_V1: &'static str = "asset.blob_wire_v1";

    /// Create a client bound to the given host API.
    ///
    /// Service id defaults to `asset.manager` and may be overridden via
    /// `NEWENGINE_ASSET_SERVICE_ID`.
    #[inline]
    pub fn new(host: HostApiV1) -> Self {
        let service_id = std::env::var("NEWENGINE_ASSET_SERVICE_ID")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "asset.manager".to_string());

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
    fn call(&self, method: &'static str, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        let res = (self.host.call_service_v1)(
            self.service_id.clone(),
            MethodName::from(method),
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
            // service returns "loading" too
            _ => AssetState::Loading,
        }
    }

    #[inline]
    fn parse_ok_json(s: &str) -> Result<serde_json::Value, String> {
        serde_json::from_str::<serde_json::Value>(s).map_err(|e| e.to_string())
    }

    fn decode_load_response(bytes: Vec<u8>) -> Result<String, String> {
        // Contract: json { ok, id_u128, error }
        // Fallback: plain string id
        let s = Self::decode_utf8(bytes)?;
        if let Ok(v) = Self::parse_ok_json(&s) {
            let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
            if !ok {
                let err = v.get("error").and_then(|x| x.as_str()).unwrap_or("load failed");
                return Err(err.to_string());
            }
            let id = v
                .get("id_u128")
                .and_then(|x| x.as_str())
                .ok_or_else(|| "load: missing id_u128".to_string())?;
            return Ok(id.trim().to_string());
        }

        let id = s.trim();
        if id.is_empty() {
            return Err("load: empty response".to_string());
        }
        Ok(id.to_string())
    }

    fn decode_state_json_response(bytes: Vec<u8>) -> Result<AssetState, String> {
        // Contract: json { ok, state, error }
        // Fallback: plain string state
        let s = Self::decode_utf8(bytes)?;
        if let Ok(v) = Self::parse_ok_json(&s) {
            let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
            if !ok {
                let err = v.get("error").and_then(|x| x.as_str()).unwrap_or("state failed");
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
}

impl AssetAccess for AssetServiceClient {
    fn load(&self, logical_path: &str) -> Result<String, String> {
        // Contract payload: utf8 logical_path
        // Keep small fallbacks for potential older services, but prefer contract-first.
        let methods = [Self::M_LOAD, "load", "asset.load"];
        let bytes = self.call_try_methods(&methods, logical_path.as_bytes().to_vec())?;
        Self::decode_load_response(bytes)
    }

    fn pump(&self) {
        // Contract payload: empty
        let methods = [Self::M_PUMP, "pump", "asset.pump", "tick", "asset.tick"];
        let _ = self.call_try_methods(&methods, Vec::new());
    }

    fn state(&self, id_hex32: &str) -> Result<AssetState, String> {
        // Contract payload: utf8 id_u128_hex32
        let methods = [Self::M_STATE_JSON, "state_json", "asset.state_json"];
        let bytes = self.call_try_methods(&methods, id_hex32.as_bytes().to_vec())?;
        Self::decode_state_json_response(bytes)
    }

    fn blob_wire_v1(&self, id_hex32: &str) -> Result<(String, Vec<u8>), String> {
        // Contract payload: utf8 id_u128_hex32
        let methods = [Self::M_BLOB_WIRE_V1, "blob_wire_v1", "asset.blob_wire_v1"];
        let bytes = self.call_try_methods(&methods, id_hex32.as_bytes().to_vec())?;
        Self::decode_blob_wire_v1(bytes)
    }
}