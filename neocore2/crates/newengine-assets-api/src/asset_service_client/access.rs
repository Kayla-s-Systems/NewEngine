use super::AssetServiceClient;
use crate::{AssetAccess, AssetDecodeRequest, AssetState, Rgba8TextureAsset, RuntimeTextureAsset};

impl AssetAccess for AssetServiceClient {
    #[inline]
    fn import_v1(&self, logical_path: &str) -> Result<String, String> {
        AssetServiceClient::import_v1(self, logical_path)
    }

    fn pump(&self) {
        // Contract payload: empty
        let _ = self.call_raw(self.m_pump.clone(), Vec::new());
    }

    fn state(&self, id_hex32: &str) -> Result<AssetState, String> {
        let id_u128 = u128::from_str_radix(id_hex32.trim(), 16)
            .map_err(|_| format!("asset.get_state_v1: bad id '{id_hex32}'"))?;
        let bytes = self.call_raw(self.m_get_state_v1.clone(), id_u128.to_le_bytes().to_vec())?;
        let code = bytes.first().copied().unwrap_or(0);
        Ok(match code {
            2 => AssetState::Ready,
            1 => AssetState::Loading,
            3 => AssetState::Failed,
            0 => AssetState::Unloaded,
            _ => AssetState::Unknown,
        })
    }

    fn status_json_v1(&self, id_or_logical_path: &str) -> Result<serde_json::Value, String> {
        let payload = if id_or_logical_path.trim().len() == 32
            && id_or_logical_path
                .trim()
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        {
            id_or_logical_path.trim().as_bytes().to_vec()
        } else {
            Self::logical_payload(id_or_logical_path)
        };
        let bytes = self.call_raw(self.m_status_json_v1.clone(), payload)?;
        Self::decode_ok_json(bytes)
    }

    fn status_graph_json_v1(&self, id_or_logical_path: &str) -> Result<serde_json::Value, String> {
        let payload = if id_or_logical_path.trim().len() == 32
            && id_or_logical_path
                .trim()
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        {
            id_or_logical_path.trim().as_bytes().to_vec()
        } else {
            Self::logical_payload(id_or_logical_path)
        };
        let bytes = self.call_raw(self.m_status_graph_json_v1.clone(), payload)?;
        Self::decode_ok_json(bytes)
    }

    fn project_status_json_v1(
        &self,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        AssetServiceClient::project_status_json_v1(self, payload)
    }

    #[inline]
    fn text_v1(&self, logical_path: &str) -> Result<Vec<u8>, String> {
        AssetServiceClient::text_v1(self, logical_path)
    }

    #[inline]
    fn raw_bytes_v1(&self, logical_path: &str) -> Result<Vec<u8>, String> {
        AssetServiceClient::raw_bytes_v1(self, logical_path)
    }

    fn blob_wire_v1(&self, id_hex32: &str) -> Result<(String, Vec<u8>), String> {
        // Contract payload: utf8 id_u128_hex32
        let bytes = self.call_raw(self.m_blob_wire_v1.clone(), id_hex32.as_bytes().to_vec())?;
        Self::decode_blob_wire_v1(bytes)
    }

    fn decode_v1(&self, request: &AssetDecodeRequest) -> Result<Vec<u8>, String> {
        AssetServiceClient::decode_v1(self, request)
    }

    fn texture_rgba8_v1(&self, id_hex32: &str) -> Result<Rgba8TextureAsset, String> {
        // Contract payload: utf8 id_u128_hex32. AssetManager owns texture meta parsing.
        let bytes = self.call_raw(
            self.m_texture_rgba8_v1.clone(),
            id_hex32.as_bytes().to_vec(),
        )?;
        Self::decode_texture_rgba8_wire_v1(bytes)
    }

    fn texture_dictionary_rgba8_v1(
        &self,
        dictionary_path: &str,
        texture_name: Option<&str>,
        texture_hash: Option<u64>,
    ) -> Result<Rgba8TextureAsset, String> {
        AssetServiceClient::texture_dictionary_rgba8_v1(
            self,
            dictionary_path,
            texture_name,
            texture_hash,
        )
    }

    fn texture_dictionary_runtime_v1(
        &self,
        dictionary_path: &str,
        texture_name: Option<&str>,
        texture_hash: Option<u64>,
    ) -> Result<RuntimeTextureAsset, String> {
        AssetServiceClient::texture_dictionary_runtime_v1(
            self,
            dictionary_path,
            texture_name,
            texture_hash,
        )
    }

    fn textures_entry_rgba8_v1(&self, texture_ref: &str) -> Result<Rgba8TextureAsset, String> {
        AssetServiceClient::textures_entry_rgba8_ref_v1_typed(self, texture_ref)
            .map_err(|e| e.to_string())
    }

    fn textures_entry_runtime_v1(&self, texture_ref: &str) -> Result<RuntimeTextureAsset, String> {
        AssetServiceClient::textures_entry_runtime_ref_v1_typed(self, texture_ref)
            .map_err(|e| e.to_string())
    }
}
