#![forbid(unsafe_op_in_unsafe_fn)]

use abi_stable::std_types::RString;
use newengine_plugin_api::{Blob, HostApiV1, MethodName};

use crate::asset_access::{AssetAccess, AssetService, AssetState, Rgba8TextureAsset, RuntimeTextureAsset, RuntimeTextureFormat, RuntimeTextureMip};
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

    // Cache MethodName allocations; clones are cheap.
    m_import_v1: MethodName,
    m_reload: MethodName,
    m_pump: MethodName,
    m_info_json_v1: MethodName,
    m_blob_wire_v1: MethodName,
    m_text_v1: MethodName,
    m_raw_bytes_v1: MethodName,
    m_texture_rgba8_v1: MethodName,
    m_texture_dictionary_rgba8_v1: MethodName,
    m_texture_dictionary_runtime_v1: MethodName,
    m_status_json_v1: MethodName,
    m_status_graph_json_v1: MethodName,
    m_project_status_json_v1: MethodName,
    m_resolve_trace_json_v1: MethodName,
    m_formats_json_v1: MethodName,
    m_sources_json_v1: MethodName,
    m_mount_source_json_v1: MethodName,
    m_get_state_v1: MethodName,
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

            m_import_v1: MethodName::from(method::IMPORT_V1),
            m_reload: MethodName::from(method::RELOAD_V1),
            m_pump: MethodName::from(method::PUMP_V1),
            m_info_json_v1: MethodName::from(method::INFO_JSON_V1),
            m_blob_wire_v1: MethodName::from(method::BLOB_WIRE_V1),
            m_text_v1: MethodName::from(method::TEXT_V1),
            m_raw_bytes_v1: MethodName::from(method::RAW_BYTES_V1),
            m_texture_rgba8_v1: MethodName::from(method::TEXTURE_RGBA8_V1),
            m_texture_dictionary_rgba8_v1: MethodName::from(method::TEXTURE_DICTIONARY_RGBA8_V1),
            m_texture_dictionary_runtime_v1: MethodName::from(method::TEXTURE_DICTIONARY_RUNTIME_V1),
            m_status_json_v1: MethodName::from(method::STATUS_JSON_V1),
            m_status_graph_json_v1: MethodName::from(method::STATUS_GRAPH_JSON_V1),
            m_project_status_json_v1: MethodName::from(method::PROJECT_STATUS_JSON_V1),
            m_resolve_trace_json_v1: MethodName::from(method::RESOLVE_TRACE_JSON_V1),
            m_formats_json_v1: MethodName::from(method::FORMATS_JSON_V1),
            m_sources_json_v1: MethodName::from(method::SOURCES_JSON_V1),
            m_mount_source_json_v1: MethodName::from(method::MOUNT_SOURCE_JSON_V1),
            m_get_state_v1: MethodName::from(method::GET_STATE_V1),
        }
    }

    #[inline]
    pub fn service_id(&self) -> &RString {
        &self.service_id
    }

    #[inline]
    fn normalize_logical_path(logical_path: &str) -> String {
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
    fn logical_payload(logical_path: &str) -> Vec<u8> {
        Self::normalize_logical_path(logical_path).into_bytes()
    }

    #[inline]
    fn call(&self, method_name: MethodName, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        let res =
            (self.host.call_service_v1)(self.service_id.clone(), method_name, Blob::from(payload));

        res.into_result()
            .map(|v| v.into_vec())
            .map_err(|e| e.to_string())
    }

    #[inline]
    fn call_raw(&self, method_name: MethodName, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        self.call(method_name, payload)
    }

    #[inline]
    fn decode_utf8(bytes: Vec<u8>) -> Result<String, String> {
        String::from_utf8(bytes).map_err(|_| "asset service returned non-utf8".to_string())
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

    fn decode_texture_rgba8_wire_v1(bytes: Vec<u8>) -> Result<Rgba8TextureAsset, String> {
        let min_len = newengine_assets_api::texture_wire::HEADER_LEN;
        if bytes.len() < min_len {
            return Err(format!("texture_rgba8_v1: short frame bytes={} expected_at_least={min_len}", bytes.len()));
        }
        if &bytes[0..4] != &newengine_assets_api::texture_wire::MAGIC[..] {
            return Err("texture_rgba8_v1: bad magic".to_string());
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != newengine_assets_api::texture_wire::VERSION_RGBA8_V1 {
            return Err(format!("texture_rgba8_v1: unsupported version {version}"));
        }
        let _flags = u16::from_le_bytes([bytes[6], bytes[7]]);
        let width = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let height = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let payload_len = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
        let expected_frame_len = min_len.saturating_add(payload_len);
        if bytes.len() != expected_frame_len {
            return Err(format!(
                "texture_rgba8_v1: payload frame size mismatch bytes={} expected={expected_frame_len}",
                bytes.len()
            ));
        }
        let rgba = bytes[min_len..].to_vec();
        Rgba8TextureAsset::new(width, height, rgba)
    }

    fn decode_texture_runtime_wire_v2(bytes: Vec<u8>) -> Result<RuntimeTextureAsset, String> {
        let header_len = newengine_assets_api::texture_wire::RUNTIME_HEADER_LEN;
        let mip_record_len = newengine_assets_api::texture_wire::RUNTIME_MIP_RECORD_LEN;
        if bytes.len() < header_len {
            return Err(format!("texture_runtime_v1: short frame bytes={} expected_at_least={header_len}", bytes.len()));
        }
        if &bytes[0..4] != &newengine_assets_api::texture_wire::MAGIC[..] {
            return Err("texture_runtime_v1: bad magic".to_string());
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != newengine_assets_api::texture_wire::VERSION_RUNTIME_V2 {
            return Err(format!("texture_runtime_v1: unsupported version {version}"));
        }
        let format_id = u16::from_le_bytes([bytes[8], bytes[9]]);
        let mip_count = u16::from_le_bytes([bytes[10], bytes[11]]) as usize;
        if mip_count == 0 {
            return Err("texture_runtime_v1: empty mip chain".to_string());
        }
        let width = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let height = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let payload_len = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]) as usize;
        let format = RuntimeTextureFormat::from_wire_id(format_id)
            .ok_or_else(|| format!("texture_runtime_v1: unsupported format id {format_id}"))?;
        let records_offset = header_len;
        let payload_offset = records_offset.saturating_add(mip_count.saturating_mul(mip_record_len));
        let expected_len = payload_offset.saturating_add(payload_len);
        if bytes.len() != expected_len {
            return Err(format!("texture_runtime_v1: frame size mismatch bytes={} expected={expected_len}", bytes.len()));
        }
        let mut mips = Vec::with_capacity(mip_count);
        for i in 0..mip_count {
            let o = records_offset + i * mip_record_len;
            let level = u16::from_le_bytes([bytes[o], bytes[o + 1]]) as u32;
            let mip_width = u32::from_le_bytes([bytes[o + 4], bytes[o + 5], bytes[o + 6], bytes[o + 7]]);
            let mip_height = u32::from_le_bytes([bytes[o + 8], bytes[o + 9], bytes[o + 10], bytes[o + 11]]);
            let byte_offset = u32::from_le_bytes([bytes[o + 12], bytes[o + 13], bytes[o + 14], bytes[o + 15]]) as usize;
            let byte_len = u32::from_le_bytes([bytes[o + 16], bytes[o + 17], bytes[o + 18], bytes[o + 19]]) as usize;
            let start = payload_offset.saturating_add(byte_offset);
            let end = start.saturating_add(byte_len);
            if byte_offset > payload_len || end > bytes.len() {
                return Err(format!("texture_runtime_v1: mip range out of bounds level={level} offset={byte_offset} len={byte_len}"));
            }
            mips.push(RuntimeTextureMip { level, width: mip_width, height: mip_height, bytes: bytes[start..end].to_vec() });
        }
        Ok(RuntimeTextureAsset { width, height, format, mips })
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

    /// Enqueue importer-owned asset import by logical path.
    #[inline]
    pub fn import_v1(&self, logical_path: &str) -> Result<String, String> {
        let bytes = self.call_raw(self.m_import_v1.clone(), Self::logical_payload(logical_path))?;
        Self::decode_load_like(bytes, "import_v1")
    }

    /// Read raw bytes from the AssetManager VFS by logical path.
    ///
    /// This intentionally bypasses importers, but it does not bypass AssetManager: resolution
    /// still goes through the mounted VFS layers (.pak, filesystem, future remote sources).
    #[inline]
    pub fn raw_bytes_v1(&self, logical_path: &str) -> Result<Vec<u8>, String> {
        self.call_raw(self.m_raw_bytes_v1.clone(), Self::logical_payload(logical_path))
    }

    /// Read UTF-8/text asset bytes directly through the AssetManager v1 text method.
    #[inline]
    pub fn text_v1(&self, logical_path: &str) -> Result<Vec<u8>, String> {
        self.call_raw(self.m_text_v1.clone(), Self::logical_payload(logical_path))
    }

    /// Validated lifecycle projection for systems that own non-CPU residency,
    /// for example the render controller marking GPU upload/residency stages.
    #[inline]
    pub fn project_status_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String> {
        let bytes = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
        Self::decode_ok_json(self.call_raw(self.m_project_status_json_v1.clone(), bytes)?)
    }

    /// Select and read one texture from a .neytd dictionary.
    ///
    /// The service accepts either texture_name or texture_hash. When both are omitted,
    /// the first dictionary entry is selected.
    #[inline]
    pub fn texture_dictionary_rgba8_v1(&self, dictionary_path: &str, texture_name: Option<&str>, texture_hash: Option<u64>) -> Result<Rgba8TextureAsset, String> {
        let mut req = serde_json::json!({ "dictionary_path": dictionary_path });
        if let Some(name) = texture_name {
            req["texture_name"] = serde_json::Value::String(name.to_owned());
        }
        if let Some(hash) = texture_hash {
            req["texture_hash"] = serde_json::Value::Number(serde_json::Number::from(hash));
        }
        let payload = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
        let bytes = self.call_raw(self.m_texture_dictionary_rgba8_v1.clone(), payload)?;
        Self::decode_texture_rgba8_wire_v1(bytes)
    }

    pub fn texture_dictionary_runtime_v1(&self, dictionary_path: &str, texture_name: Option<&str>, texture_hash: Option<u64>) -> Result<RuntimeTextureAsset, String> {
        let mut req = serde_json::json!({ "dictionary_path": dictionary_path });
        if let Some(name) = texture_name {
            req["texture_name"] = serde_json::Value::String(name.to_owned());
        }
        if let Some(hash) = texture_hash {
            req["texture_hash"] = serde_json::Value::Number(serde_json::Number::from(hash));
        }
        let payload = serde_json::to_vec(&req).map_err(|e| e.to_string())?;
        let bytes = self.call_raw(self.m_texture_dictionary_runtime_v1.clone(), payload)?;
        Self::decode_texture_runtime_wire_v2(bytes)
    }
}

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
            && id_or_logical_path.trim().chars().all(|c| c.is_ascii_hexdigit())
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
            && id_or_logical_path.trim().chars().all(|c| c.is_ascii_hexdigit())
        {
            id_or_logical_path.trim().as_bytes().to_vec()
        } else {
            Self::logical_payload(id_or_logical_path)
        };
        let bytes = self.call_raw(self.m_status_graph_json_v1.clone(), payload)?;
        Self::decode_ok_json(bytes)
    }

    fn project_status_json_v1(&self, payload: serde_json::Value) -> Result<serde_json::Value, String> {
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

    fn texture_rgba8_v1(&self, id_hex32: &str) -> Result<Rgba8TextureAsset, String> {
        // Contract payload: utf8 id_u128_hex32. AssetManager owns texture meta parsing.
        let bytes = self.call_raw(self.m_texture_rgba8_v1.clone(), id_hex32.as_bytes().to_vec())?;
        Self::decode_texture_rgba8_wire_v1(bytes)
    }

    fn texture_dictionary_rgba8_v1(&self, dictionary_path: &str, texture_name: Option<&str>, texture_hash: Option<u64>) -> Result<Rgba8TextureAsset, String> {
        AssetServiceClient::texture_dictionary_rgba8_v1(self, dictionary_path, texture_name, texture_hash)
    }

    fn texture_dictionary_runtime_v1(&self, dictionary_path: &str, texture_name: Option<&str>, texture_hash: Option<u64>) -> Result<RuntimeTextureAsset, String> {
        AssetServiceClient::texture_dictionary_runtime_v1(self, dictionary_path, texture_name, texture_hash)
    }
}

impl AssetService for AssetServiceClient {
    fn reload(&self, logical_path: &str) -> Result<String, String> {
        let bytes = self.call_raw(self.m_reload.clone(), Self::logical_payload(logical_path))?;
        Self::decode_load_like(bytes, "reload_v1")
    }

    fn info_json_v1(&self, logical_path: &str) -> Result<serde_json::Value, String> {
        let bytes = self.call_raw(self.m_info_json_v1.clone(), Self::logical_payload(logical_path))?;
        Self::decode_ok_json(bytes)
    }

    fn formats_json_v1(&self) -> Result<serde_json::Value, String> {
        let bytes = self.call_raw(self.m_formats_json_v1.clone(), Vec::new())?;
        Self::decode_ok_json(bytes)
    }

    fn sources_json_v1(&self) -> Result<serde_json::Value, String> {
        let bytes = self.call_raw(self.m_sources_json_v1.clone(), Vec::new())?;
        Self::decode_ok_json(bytes)
    }

    fn mount_source_json_v1(&self, payload: serde_json::Value) -> Result<(), String> {
        let bytes = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
        let bytes = self.call_raw(self.m_mount_source_json_v1.clone(), bytes)?;
        Self::decode_ok_unit(bytes)
    }

    fn resolve_trace_json_v1(&self, logical_path: &str) -> Result<serde_json::Value, String> {
        let bytes = self.call_raw(
            self.m_resolve_trace_json_v1.clone(),
            Self::logical_payload(logical_path),
        )?;
        Self::decode_ok_json(bytes)
    }
}
