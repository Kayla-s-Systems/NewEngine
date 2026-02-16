#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_plugin_api::{Blob, CapabilityId, HostApiV1, MethodName};
use serde::Deserialize;

use crate::asset_access::{AssetAccess, AssetState};

pub struct AssetServiceClient {
    host: HostApiV1,
}

impl AssetServiceClient {
    #[inline]
    pub fn new(host: HostApiV1) -> Self {
        Self { host }
    }

    #[inline]
    fn call(&self, method: &str, payload: &[u8]) -> Result<Vec<u8>, String> {
        let out = (self.host.call_service_v1)(
            CapabilityId::from("asset.manager"),
            MethodName::from(method),
            Blob::from(payload.to_vec()),
        );

        out.into_result()
            .map(|b| b.into_vec())
            .map_err(|e| e.to_string())
    }

    #[inline]
    fn pump_best_effort(&self) {
        let _ = self.call("asset.pump", &[]);
    }
}

#[derive(Deserialize)]
struct LoadResp {
    ok: bool,
    id_u128: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct StateResp {
    ok: bool,
    state: String,
    error: Option<String>,
}

impl AssetAccess for AssetServiceClient {
    fn load(&self, logical_path: &str) -> Result<String, String> {
        let bytes = self.call("asset.load", logical_path.as_bytes())?;
        let r: LoadResp = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if !r.ok {
            return Err(r.error.unwrap_or_else(|| "asset.load failed".to_string()));
        }
        r.id_u128.ok_or_else(|| "asset.load: missing id_u128".to_string())
    }

    fn state(&self, id_u128_hex32: &str) -> Result<AssetState, String> {
        let bytes = self.call("asset.state_json", id_u128_hex32.as_bytes())?;
        let r: StateResp = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if !r.ok {
            return Err(r.error.unwrap_or_else(|| "asset.state_json failed".to_string()));
        }
        AssetState::from_str(r.state.as_str())
            .ok_or_else(|| format!("asset.state_json: unknown state '{}'", r.state))
    }

    fn blob_wire_v1(&self, id_u128_hex32: &str) -> Result<(String, Vec<u8>), String> {
        let frame = self.call("asset.blob_wire_v1", id_u128_hex32.as_bytes())?;
        if frame.len() < 4 {
            return Err("asset.blob_wire_v1: frame too small".to_string());
        }

        let meta_len = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        let need = 4usize.saturating_add(meta_len);
        if frame.len() < need {
            return Err("asset.blob_wire_v1: truncated meta".to_string());
        }

        let meta = &frame[4..4 + meta_len];
        let payload = frame[4 + meta_len..].to_vec();

        let meta_json = std::str::from_utf8(meta)
            .map_err(|_| "asset.blob_wire_v1: meta is not utf8".to_string())?
            .to_string();

        Ok((meta_json, payload))
    }

    fn pump(&self) {
        self.pump_best_effort();
    }
}
