use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, MethodName};

use super::AssetServiceClient;
use crate::{AssetError, AssetResult};

impl AssetServiceClient {
    #[inline]
    pub(super) fn normalize_logical_path(logical_path: &str) -> String {
        let mut s = logical_path.trim().replace('\\', "/");
        while let Some(rest) = s.strip_prefix("./") {
            s = rest.to_owned();
        }
        s = s.trim_start_matches('/').to_owned();
        while s.contains("//") {
            s = s.replace("//", "/");
        }
        s
    }

    #[inline]
    pub(super) fn logical_payload(logical_path: &str) -> Vec<u8> {
        Self::normalize_logical_path(logical_path).into_bytes()
    }

    #[inline]
    pub(super) fn call(
        &self,
        method_name: MethodName,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        self.call_typed(method_name, payload)
            .map_err(|e| e.to_string())
    }

    #[inline]
    pub(super) fn call_typed(
        &self,
        method_name: MethodName,
        payload: Vec<u8>,
    ) -> AssetResult<Vec<u8>> {
        self.call_service_typed(self.service_id.clone(), method_name, payload)
    }

    #[inline]
    pub(super) fn call_service(
        &self,
        service_id: &'static str,
        method_name: MethodName,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        self.call_service_typed(RString::from(service_id), method_name, payload)
            .map_err(|e| e.to_string())
    }

    #[inline]
    pub(super) fn call_service_typed(
        &self,
        service_id: RString,
        method_name: MethodName,
        payload: Vec<u8>,
    ) -> AssetResult<Vec<u8>> {
        let res = (self.host.call_service_v1)(service_id, method_name, Blob::from(payload));

        res.into_result()
            .map(|v| v.into_vec())
            .map_err(|e| AssetError::from_wire_or_message(e.to_string()))
    }

    #[inline]
    pub(super) fn call_raw(
        &self,
        method_name: MethodName,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        self.call(method_name, payload)
    }

    #[inline]
    pub(super) fn call_raw_typed(
        &self,
        method_name: MethodName,
        payload: Vec<u8>,
    ) -> AssetResult<Vec<u8>> {
        self.call_typed(method_name, payload)
    }

    #[inline]
    pub(super) fn call_json_value(
        &self,
        method_name: MethodName,
        payload: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let payload = serde_json::to_vec(payload).map_err(|e| e.to_string())?;
        Self::decode_ok_json(self.call_raw(method_name, payload)?)
    }

    #[inline]
    pub(super) fn call_logical_json(
        &self,
        method_name: MethodName,
        logical_path: &str,
    ) -> Result<serde_json::Value, String> {
        Self::decode_ok_json(self.call_raw(method_name, Self::logical_payload(logical_path))?)
    }

    #[inline]
    pub(super) fn call_empty_json(
        &self,
        method_name: MethodName,
    ) -> Result<serde_json::Value, String> {
        Self::decode_ok_json(self.call_raw(method_name, Vec::new())?)
    }

    #[inline]
    pub(super) fn call_json_unit(
        &self,
        method_name: MethodName,
        payload: &serde_json::Value,
    ) -> Result<(), String> {
        let payload = serde_json::to_vec(payload).map_err(|e| e.to_string())?;
        Self::decode_ok_unit(self.call_raw(method_name, payload)?)
    }

    #[inline]
    pub(super) fn call_json_typed<Request, Response>(
        &self,
        method_name: MethodName,
        payload: &Request,
        op: &'static str,
    ) -> Result<Response, String>
    where
        Request: serde::Serialize,
        Response: serde::de::DeserializeOwned,
    {
        let payload =
            serde_json::to_vec(payload).map_err(|e| format!("{op}: invalid request: {e}"))?;
        Self::decode_json(self.call_raw(method_name, payload)?, op)
    }

    #[inline]
    pub(super) fn call_service_json_typed<Request, Response>(
        &self,
        service_id: &'static str,
        method_name: MethodName,
        payload: &Request,
        op: &'static str,
    ) -> Result<Response, String>
    where
        Request: serde::Serialize,
        Response: serde::de::DeserializeOwned,
    {
        let payload =
            serde_json::to_vec(payload).map_err(|e| format!("{op}: invalid request: {e}"))?;
        Self::decode_json(self.call_service(service_id, method_name, payload)?, op)
    }

    #[inline]
    pub(super) fn decode_utf8(bytes: Vec<u8>) -> Result<String, String> {
        String::from_utf8(bytes).map_err(|_| "asset service returned non-utf8".to_string())
    }

    #[inline]
    pub(super) fn parse_json(s: &str) -> Result<serde_json::Value, String> {
        serde_json::from_str::<serde_json::Value>(s).map_err(|e| e.to_string())
    }

    pub(super) fn decode_json<T: serde::de::DeserializeOwned>(
        bytes: Vec<u8>,
        op: &'static str,
    ) -> Result<T, String> {
        let s = Self::decode_utf8(bytes)?;
        serde_json::from_str::<T>(&s).map_err(|e| format!("{op}: invalid json response: {e}"))
    }

    pub(super) fn decode_ok_json(bytes: Vec<u8>) -> Result<serde_json::Value, String> {
        let s = Self::decode_utf8(bytes)?;
        let v = Self::parse_json(&s)?;
        Ok(v)
    }

    pub(super) fn decode_load_like(bytes: Vec<u8>, op: &'static str) -> Result<String, String> {
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

    pub(super) fn decode_blob_wire_v1(bytes: Vec<u8>) -> Result<(String, Vec<u8>), String> {
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

    #[inline]
    pub(super) fn json_payload_typed(value: &serde_json::Value) -> AssetResult<Vec<u8>> {
        serde_json::to_vec(value).map_err(|e| AssetError::invalid_request(e.to_string()))
    }

    pub(super) fn decode_ok_unit(bytes: Vec<u8>) -> Result<(), String> {
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
