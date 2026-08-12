use abi_stable::std_types::RString;
use newengine_plugin_api::MethodName;

use super::AssetServiceClient;
use crate::{
    require_asset_reference_extension, textures_method, AssetError, AssetResult, Rgba8TextureAsset,
    RuntimeTextureAsset, RuntimeTextureFormat, RuntimeTextureMip,
    ENGINE_ASSETS_TEXTURES_SERVICE_ID,
};

impl AssetServiceClient {
    pub(super) fn decode_texture_rgba8_wire_v1(
        bytes: Vec<u8>,
    ) -> Result<Rgba8TextureAsset, String> {
        let min_len = crate::texture_wire::HEADER_LEN;
        if bytes.len() < min_len {
            return Err(format!(
                "texture_rgba8_v1: short frame bytes={} expected_at_least={min_len}",
                bytes.len()
            ));
        }
        if bytes[0..4] != crate::texture_wire::MAGIC[..] {
            return Err("texture_rgba8_v1: bad magic".to_string());
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != crate::texture_wire::VERSION_RGBA8_V1 {
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

    pub(super) fn decode_texture_runtime_wire_v2(
        bytes: Vec<u8>,
    ) -> Result<RuntimeTextureAsset, String> {
        let header_len = crate::texture_wire::RUNTIME_HEADER_LEN;
        let mip_record_len = crate::texture_wire::RUNTIME_MIP_RECORD_LEN;
        if bytes.len() < header_len {
            return Err(format!(
                "texture_runtime_v1: short frame bytes={} expected_at_least={header_len}",
                bytes.len()
            ));
        }
        if bytes[0..4] != crate::texture_wire::MAGIC[..] {
            return Err("texture_runtime_v1: bad magic".to_string());
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != crate::texture_wire::VERSION_RUNTIME_V2 {
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
        let payload_offset =
            records_offset.saturating_add(mip_count.saturating_mul(mip_record_len));
        let expected_len = payload_offset.saturating_add(payload_len);
        if bytes.len() != expected_len {
            return Err(format!(
                "texture_runtime_v1: frame size mismatch bytes={} expected={expected_len}",
                bytes.len()
            ));
        }
        let mut mips = Vec::with_capacity(mip_count);
        for i in 0..mip_count {
            let o = records_offset + i * mip_record_len;
            let level = u16::from_le_bytes([bytes[o], bytes[o + 1]]) as u32;
            let mip_width =
                u32::from_le_bytes([bytes[o + 4], bytes[o + 5], bytes[o + 6], bytes[o + 7]]);
            let mip_height =
                u32::from_le_bytes([bytes[o + 8], bytes[o + 9], bytes[o + 10], bytes[o + 11]]);
            let byte_offset =
                u32::from_le_bytes([bytes[o + 12], bytes[o + 13], bytes[o + 14], bytes[o + 15]])
                    as usize;
            let byte_len =
                u32::from_le_bytes([bytes[o + 16], bytes[o + 17], bytes[o + 18], bytes[o + 19]])
                    as usize;
            let start = payload_offset.saturating_add(byte_offset);
            let end = start.saturating_add(byte_len);
            if byte_offset > payload_len || end > bytes.len() {
                return Err(format!("texture_runtime_v1: mip range out of bounds level={level} offset={byte_offset} len={byte_len}"));
            }
            mips.push(RuntimeTextureMip {
                level,
                width: mip_width,
                height: mip_height,
                bytes: bytes[start..end].to_vec(),
            });
        }
        Ok(RuntimeTextureAsset {
            width,
            height,
            format,
            mips,
        })
    }

    #[inline]
    pub(super) fn decode_texture_rgba8_wire_v1_typed(
        bytes: Vec<u8>,
    ) -> AssetResult<Rgba8TextureAsset> {
        Self::decode_texture_rgba8_wire_v1(bytes).map_err(AssetError::decode_failed)
    }

    #[inline]
    pub(super) fn decode_texture_runtime_wire_v2_typed(
        bytes: Vec<u8>,
    ) -> AssetResult<RuntimeTextureAsset> {
        Self::decode_texture_runtime_wire_v2(bytes).map_err(|message| {
            let lower = message.to_ascii_lowercase();
            if lower.contains("unsupported")
                || lower.contains("bad magic")
                || lower.contains("format")
            {
                AssetError::unsupported_format(message)
            } else {
                AssetError::decode_failed(message)
            }
        })
    }

    /// Select and read one texture from a .ytd dictionary.
    ///
    /// The service accepts either texture_name or texture_hash. When both are omitted,
    /// the first dictionary entry is selected.
    #[inline]
    pub fn texture_dictionary_rgba8_v1_typed(
        &self,
        dictionary_path: &str,
        texture_name: Option<&str>,
        texture_hash: Option<u64>,
    ) -> AssetResult<Rgba8TextureAsset> {
        let mut req = serde_json::json!({ "dictionary_path": dictionary_path });
        if let Some(name) = texture_name {
            req["texture_name"] = serde_json::Value::String(name.to_owned());
        }
        if let Some(hash) = texture_hash {
            req["texture_hash"] = serde_json::Value::Number(serde_json::Number::from(hash));
        }
        let payload = Self::json_payload_typed(&req)?;
        let bytes = self.call_raw_typed(self.m_texture_dictionary_rgba8_v1.clone(), payload)?;
        Self::decode_texture_rgba8_wire_v1_typed(bytes)
            .map_err(|e| e.with_logical_path(dictionary_path))
    }

    #[inline]
    pub fn texture_dictionary_rgba8_v1(
        &self,
        dictionary_path: &str,
        texture_name: Option<&str>,
        texture_hash: Option<u64>,
    ) -> Result<Rgba8TextureAsset, String> {
        self.texture_dictionary_rgba8_v1_typed(dictionary_path, texture_name, texture_hash)
            .map_err(|e| e.to_string())
    }

    #[inline]
    pub fn texture_dictionary_runtime_v1_typed(
        &self,
        dictionary_path: &str,
        texture_name: Option<&str>,
        texture_hash: Option<u64>,
    ) -> AssetResult<RuntimeTextureAsset> {
        let mut req = serde_json::json!({ "dictionary_path": dictionary_path });
        if let Some(name) = texture_name {
            req["texture_name"] = serde_json::Value::String(name.to_owned());
        }
        if let Some(hash) = texture_hash {
            req["texture_hash"] = serde_json::Value::Number(serde_json::Number::from(hash));
        }
        let payload = Self::json_payload_typed(&req)?;
        let bytes = self.call_raw_typed(self.m_texture_dictionary_runtime_v1.clone(), payload)?;
        Self::decode_texture_runtime_wire_v2_typed(bytes)
            .map_err(|e| e.with_logical_path(dictionary_path))
    }

    pub fn texture_dictionary_runtime_v1(
        &self,
        dictionary_path: &str,
        texture_name: Option<&str>,
        texture_hash: Option<u64>,
    ) -> Result<RuntimeTextureAsset, String> {
        self.texture_dictionary_runtime_v1_typed(dictionary_path, texture_name, texture_hash)
            .map_err(|e| e.to_string())
    }

    /// Call the semantic `engine.assets.textures` gateway for `assets.textures.entry_runtime_v1`.
    ///
    /// `engine.assets` remains the byte/VFS/codec-dispatch owner, but texture semantics,
    /// selector validation and runtime packet ownership belong to `engine.assets.textures`.
    #[inline]
    pub fn textures_entry_runtime_v1_typed(
        &self,
        dictionary_path: &str,
        texture_name: Option<&str>,
        texture_hash: Option<u64>,
    ) -> AssetResult<RuntimeTextureAsset> {
        let texture_ref = texture_ref_from_parts(dictionary_path, texture_name, texture_hash)?;
        self.textures_entry_runtime_ref_v1_typed(&texture_ref)
    }

    /// Call the semantic `engine.assets.textures` gateway with an authored `.ytd@entry` selector.
    #[inline]
    pub fn textures_entry_runtime_ref_v1_typed(
        &self,
        texture_ref: &str,
    ) -> AssetResult<RuntimeTextureAsset> {
        let reference = require_asset_reference_extension(texture_ref, &["ytd"], true)
            .map_err(AssetError::invalid_request)?;
        let payload =
            Self::json_payload_typed(&serde_json::json!({ "texture_ref": reference.canonical }))?;
        let bytes = self.call_service_typed(
            RString::from(ENGINE_ASSETS_TEXTURES_SERVICE_ID),
            MethodName::from(textures_method::ENTRY_RUNTIME_V1),
            payload,
        )?;
        Self::decode_texture_runtime_wire_v2_typed(bytes)
            .map_err(|e| e.with_logical_path(&reference.canonical))
    }

    /// Call the semantic `engine.assets.textures` gateway for `assets.textures.entry_rgba8_v1` debug/editor packets.
    #[inline]
    pub fn textures_entry_rgba8_v1_typed(
        &self,
        dictionary_path: &str,
        texture_name: Option<&str>,
        texture_hash: Option<u64>,
    ) -> AssetResult<Rgba8TextureAsset> {
        let texture_ref = texture_ref_from_parts(dictionary_path, texture_name, texture_hash)?;
        self.textures_entry_rgba8_ref_v1_typed(&texture_ref)
    }

    /// Call the semantic `engine.assets.textures` gateway with an authored `.ytd@entry` selector.
    #[inline]
    pub fn textures_entry_rgba8_ref_v1_typed(
        &self,
        texture_ref: &str,
    ) -> AssetResult<Rgba8TextureAsset> {
        let reference = require_asset_reference_extension(texture_ref, &["ytd"], true)
            .map_err(AssetError::invalid_request)?;
        let payload =
            Self::json_payload_typed(&serde_json::json!({ "texture_ref": reference.canonical }))?;
        let bytes = self.call_service_typed(
            RString::from(ENGINE_ASSETS_TEXTURES_SERVICE_ID),
            MethodName::from(textures_method::ENTRY_RGBA8_V1),
            payload,
        )?;
        Self::decode_texture_rgba8_wire_v1_typed(bytes)
            .map_err(|e| e.with_logical_path(&reference.canonical))
    }
}

fn texture_ref_from_parts(
    dictionary_path: &str,
    texture_name: Option<&str>,
    texture_hash: Option<u64>,
) -> AssetResult<String> {
    let dictionary_path = dictionary_path.trim().replace('\\', "/");
    if dictionary_path.trim().is_empty() {
        return Err(AssetError::invalid_request(
            "assets.textures.entry_* requires non-empty .ytd dictionary path",
        ));
    }
    if let Some(hash) = texture_hash {
        return Ok(format!("{}@hash:{}", dictionary_path, hash));
    }
    let Some(name) = texture_name.map(str::trim).filter(|it| !it.is_empty()) else {
        return Err(AssetError::invalid_request(
            "assets.textures.entry_* requires .ytd@entry or texture_hash",
        ));
    };
    Ok(format!("{}@{}", dictionary_path, name))
}
