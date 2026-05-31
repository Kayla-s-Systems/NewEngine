#![forbid(unsafe_op_in_unsafe_fn)]

//! Runtime-hosted `engine.assets.textures` runtime service.
//!
//! `.ytd` semantic ownership lives here. The service deliberately uses
//! `engine.assets` only as the byte/VFS/codec owner, then returns texture-domain
//! DTOs or stable runtime texture packets to consumers.

use abi_stable::std_types::{RResult, RString};
use newengine_assets::{AssetDecodeRequest, AssetServiceClient, Rgba8TextureAsset, RuntimeTextureAsset, RuntimeTextureFormat, RuntimeTextureMip};
use newengine_assets_api::{
    textures_method, AssetReference, ASSET_LIST_FILE_BODY_OUTPUT, ENGINE_ASSET_SERVICE_ID, ENGINE_ASSETS_TEXTURES_SERVICE_ID,
    TEXTURES_BACKEND_CAPABILITY_ID, TEXTURES_RUNTIME_CONTRACT, TEXTURES_SERVICE_ID,
    TEXTURES_SERVICE_METHODS,
};
use newengine_plugin_api::Blob;
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl, JsonServiceRouter,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const TEXTURES_GATEWAY_OWNER: &str = "newengine-textures-runtime.engine-runtime-provider";

#[derive(Clone)]
pub struct TextureRuntimeState {
    client: AssetServiceClient,
    manifest_cache: HashMap<String, serde_json::Value>,
    validation_cache: HashMap<String, TextureRefValidation>,
    runtime_packet_cache: HashMap<String, RuntimeTextureAsset>,
    rgba8_packet_cache: HashMap<String, Rgba8TextureAsset>,
    runtime_dictionary_cache: HashMap<String, RuntimeTextureDictionaryCache>,
}

impl TextureRuntimeState {
    #[inline]
    pub fn new(client: AssetServiceClient) -> Self {
        Self {
            client,
            manifest_cache: HashMap::default(),
            validation_cache: HashMap::default(),
            runtime_packet_cache: HashMap::default(),
            rgba8_packet_cache: HashMap::default(),
            runtime_dictionary_cache: HashMap::default(),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct RuntimeTextureDictionaryCache {
    entries_by_name: HashMap<String, RuntimeTextureAsset>,
    entry_hash_to_name: HashMap<u64, String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TexturesServiceInfo {
    pub id: &'static str,
    pub gateway: &'static str,
    pub provider: &'static str,
    pub contract: &'static str,
    pub byte_owner: &'static str,
    pub semantic_owner: &'static str,
    pub methods: &'static [&'static str],
    pub validation_policy: &'static str,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TextureRefRequest {
    pub texture_ref: String,
    pub dictionary_path: String,
    pub texture_name: Option<String>,
    pub texture_hash: Option<u64>,
}

impl Default for TextureRefRequest {
    #[inline]
    fn default() -> Self {
        Self { texture_ref: String::new(), dictionary_path: String::new(), texture_name: None, texture_hash: None }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TextureManifestRequest {
    pub source: String,
    pub dictionary_path: String,
    pub texture_ref: String,
}

impl Default for TextureManifestRequest {
    #[inline]
    fn default() -> Self {
        Self { source: String::new(), dictionary_path: String::new(), texture_ref: String::new() }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TextureRefValidation {
    pub ok: bool,
    pub gateway: &'static str,
    pub byte_owner: &'static str,
    pub semantic_owner: &'static str,
    pub logical_path: String,
    pub entry: Option<String>,
    pub texture_hash: Option<u64>,
    pub canonical: String,
    pub packet: Option<TexturePacketSummary>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TexturePacketSummary {
    pub width: u32,
    pub height: u32,
    pub pixel_format: String,
    pub color_space: String,
    pub mip_count: usize,
}

#[derive(Clone, Debug, Serialize)]
struct StableDiagnostic {
    ok: bool,
    code: &'static str,
    message: String,
    gateway: &'static str,
    byte_owner: &'static str,
    semantic_owner: &'static str,
}

pub fn textures_service_info() -> TexturesServiceInfo {
    TexturesServiceInfo {
        id: TEXTURES_SERVICE_ID,
        gateway: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
        provider: "StarVaultTexturesRuntimeProvider",
        contract: TEXTURES_RUNTIME_CONTRACT,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
        methods: TEXTURES_SERVICE_METHODS,
        validation_policy: "accept .ytd@entry and .ytd@hash:<u64>; reject raw images and .ytd without @entry",
    }
}

fn invalid_texture_ref(message: impl Into<String>) -> StableDiagnostic {
    StableDiagnostic {
        ok: false,
        code: "textures.invalid_ref",
        message: message.into(),
        gateway: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
    }
}

fn texture_ref_from_request(request: &TextureRefRequest) -> Result<String, String> {
    if !request.texture_ref.trim().is_empty() {
        return Ok(normalize_logical_ref(&request.texture_ref));
    }
    let dict = normalize_logical_ref(&request.dictionary_path);
    if dict.is_empty() {
        return Err("textures API requires texture_ref='.ytd@entry' or dictionary_path + texture_name/hash".to_owned());
    }
    if let Some(hash) = request.texture_hash {
        return Ok(format!("{dict}@hash:{hash}"));
    }
    let entry = request.texture_name.as_deref().unwrap_or("").trim();
    if entry.is_empty() {
        return Err("textures API requires .ytd@entry; .ytd without @entry is not a runtime texture ref".to_owned());
    }
    Ok(format!("{dict}@{entry}"))
}

fn manifest_source_from_request(request: &TextureManifestRequest) -> Result<String, String> {
    let raw = if !request.source.trim().is_empty() {
        request.source.trim()
    } else if !request.dictionary_path.trim().is_empty() {
        request.dictionary_path.trim()
    } else if !request.texture_ref.trim().is_empty() {
        request.texture_ref.split('@').next().unwrap_or(request.texture_ref.trim())
    } else {
        return Err("assets.textures.manifest_v1 requires source or dictionary_path ending in .ytd".to_owned());
    };
    let normalized = normalize_logical_ref(raw);
    reject_non_texture_ref_shape(&normalized, false)?;
    let reference = newengine_assets_api::require_asset_reference_extension(&normalized, &["ytd"], false)
        .map_err(|e| e.to_string())?;
    Ok(reference.logical_path)
}

fn texture_ref_request_from_payload(payload: &[u8], method: &str) -> Result<TextureRefRequest, String> {
    let trimmed = std::str::from_utf8(payload)
        .map(str::trim)
        .map_err(|e| format!("{method} invalid utf-8 request: {e}"))?;
    if trimmed.is_empty() {
        return Err(format!("{method} requires texture_ref='.ytd@entry'"));
    }
    if trimmed.starts_with('{') {
        serde_json::from_str::<TextureRefRequest>(trimmed)
            .map_err(|e| format!("{method} invalid json request: {e}"))
    } else {
        Ok(TextureRefRequest { texture_ref: trimmed.trim_matches('"').to_owned(), ..Default::default() })
    }
}

fn texture_manifest_request_from_payload(payload: &[u8], method: &str) -> Result<TextureManifestRequest, String> {
    let trimmed = std::str::from_utf8(payload)
        .map(str::trim)
        .map_err(|e| format!("{method} invalid utf-8 request: {e}"))?;
    if trimmed.is_empty() {
        return Err(format!("{method} requires source='textures/foo.ytd'"));
    }
    if trimmed.starts_with('{') {
        serde_json::from_str::<TextureManifestRequest>(trimmed)
            .map_err(|e| format!("{method} invalid json request: {e}"))
    } else {
        Ok(TextureManifestRequest { source: trimmed.trim_matches('"').to_owned(), ..Default::default() })
    }
}

fn normalize_logical_ref(value: &str) -> String {
    let mut s = value.trim().replace('\\', "/");
    while let Some(rest) = s.strip_prefix("./") { s = rest.to_owned(); }
    s = s.trim_start_matches('/').to_owned();
    while s.contains("//") { s = s.replace("//", "/"); }
    s
}

fn reject_non_texture_ref_shape(value: &str, entry_required: bool) -> Result<(), String> {
    let lower = value.to_ascii_lowercase();
    let path_part = lower.split('@').next().unwrap_or(&lower);
    if [".png", ".jpg", ".jpeg", ".dds", ".tga", ".webp", ".bmp"].iter().any(|ext| path_part.ends_with(ext)) {
        return Err("engine.assets.textures rejects raw source image paths; importers must compile them into .ytd entries".to_owned());
    }
    if entry_required && !lower.contains('@') {
        return Err("engine.assets.textures runtime refs require .ytd@entry or .ytd@hash:<u64>; .ytd alone is a dictionary, not a texture entry".to_owned());
    }
    Ok(())
}

fn split_hash_selector(entry: &str) -> Result<Option<u64>, String> {
    let Some(rest) = entry.strip_prefix("hash:") else { return Ok(None); };
    rest.parse::<u64>()
        .map(Some)
        .map_err(|_| format!("invalid .ytd hash selector '{entry}'; expected hash:<u64>"))
}

fn parse_texture_reference(value: &str) -> Result<(AssetReference, Option<String>, Option<u64>), String> {
    let normalized = normalize_logical_ref(value);
    reject_non_texture_ref_shape(&normalized, true)?;
    let reference = newengine_assets_api::require_asset_reference_extension(&normalized, &["ytd"], true)
        .map_err(|e| e.to_string())?;
    let entry = reference.entry.clone().unwrap_or_default();
    let texture_hash = split_hash_selector(&entry)?;
    let texture_name = if texture_hash.is_some() { None } else { Some(entry) };
    Ok((reference, texture_name, texture_hash))
}

fn validate_texture_ref(state: &mut TextureRuntimeState, request: TextureRefRequest) -> Result<TextureRefValidation, String> {
    let texture_ref = texture_ref_from_request(&request)?;
    let (reference, texture_name, texture_hash_from_ref) = parse_texture_reference(&texture_ref)?;
    let texture_hash = request.texture_hash.or(texture_hash_from_ref);
    let canonical = reference.canonical.clone();
    if let Some(cached) = state.validation_cache.get(&canonical).cloned() {
        log::debug!(
            "assets.textures.validate_ref_v1: cache hit ref='{}' dictionary='{}' policy='manifest-only validation'",
            canonical,
            reference.logical_path
        );
        return Ok(cached);
    }

    let manifest = manifest_json(
        state,
        TextureManifestRequest { source: reference.logical_path.clone(), ..Default::default() },
    )?;
    let summary = texture_summary_from_manifest_entry(&manifest, texture_name.as_deref(), texture_hash)
        .map_err(|e| format!("engine.assets.textures validation failed ref='{}' err='{}'", canonical, e))?;

    let validation = TextureRefValidation {
        ok: true,
        gateway: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
        logical_path: reference.logical_path,
        entry: texture_name,
        texture_hash,
        canonical: canonical.clone(),
        packet: Some(summary),
        warnings: vec!["validated_by=engine.assets.textures.manifest_only".to_owned()],
    };
    state.validation_cache.insert(canonical, validation.clone());
    Ok(validation)
}

fn texture_summary_from_manifest_entry(
    manifest: &serde_json::Value,
    texture_name: Option<&str>,
    texture_hash: Option<u64>,
) -> Result<TexturePacketSummary, String> {
    let entries = manifest
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "texture manifest has no entries[]".to_owned())?;
    let entry = entries
        .iter()
        .find(|entry| texture_manifest_entry_matches(entry, texture_name, texture_hash))
        .ok_or_else(|| match (texture_name, texture_hash) {
            (Some(name), _) => format!("texture entry '{name}' is not present in manifest"),
            (_, Some(hash)) => format!("texture hash '{hash}' is not present in manifest"),
            _ => "texture reference did not provide entry name/hash".to_owned(),
        })?;
    Ok(TexturePacketSummary {
        width: texture_manifest_u32(entry, "width").unwrap_or(1),
        height: texture_manifest_u32(entry, "height").unwrap_or(1),
        pixel_format: texture_manifest_string(entry, "pixel_format")
            .or_else(|| texture_manifest_metadata_string(entry, "format"))
            .unwrap_or_else(|| "UNKNOWN".to_owned()),
        color_space: texture_manifest_string(entry, "color_space")
            .or_else(|| texture_manifest_metadata_string(entry, "color_space"))
            .unwrap_or_else(|| "linear".to_owned()),
        mip_count: texture_manifest_u32(entry, "mip_count")
            .or_else(|| texture_manifest_metadata_string(entry, "mip_count").and_then(|value| value.parse::<u32>().ok()))
            .unwrap_or(1) as usize,
    })
}

fn texture_manifest_entry_matches(entry: &serde_json::Value, texture_name: Option<&str>, texture_hash: Option<u64>) -> bool {
    if let Some(name) = texture_name {
        if entry
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(|candidate| candidate.eq_ignore_ascii_case(name))
            .unwrap_or(false)
        {
            return true;
        }
    }
    if let Some(hash) = texture_hash {
        if entry.get("name_hash").and_then(serde_json::Value::as_u64) == Some(hash) {
            return true;
        }
        if entry
            .get("stable_id")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| u64::from_str_radix(value.trim_start_matches("0x"), 16).ok())
            == Some(hash)
        {
            return true;
        }
    }
    false
}

fn texture_manifest_u32(entry: &serde_json::Value, key: &str) -> Option<u32> {
    entry
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

fn texture_manifest_string(entry: &serde_json::Value, key: &str) -> Option<String> {
    entry
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

fn texture_manifest_metadata_string(entry: &serde_json::Value, key: &str) -> Option<String> {
    entry
        .get("metadata")
        .and_then(|metadata| metadata.get(key))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

fn manifest_json(state: &mut TextureRuntimeState, request: TextureManifestRequest) -> Result<serde_json::Value, String> {
    let path = manifest_source_from_request(&request)?;
    if let Some(cached) = state.manifest_cache.get(&path).cloned() {
        log::debug!(
            "assets.textures.manifest_v1: cache hit source='{}' policy='manifest-only fast path'",
            path
        );
        return Ok(cached);
    }
    let mut errors = Vec::new();
    for output_kind in [
        textures_method::MANIFEST_JSON_V1,
        "texture_dictionary.manifest_json",
        "texture_dictionary.manifest_json_v1",
        "asset.list_file_manifest_v1",
    ] {
        match state.client.decode_v1(&AssetDecodeRequest {
            logical_path: path.clone(),
            output_kind: output_kind.to_owned(),
            selector: serde_json::Value::Null,
        }) {
            Ok(bytes) => {
                let mut value = serde_json::from_slice::<serde_json::Value>(&bytes)
                    .map_err(|e| format!("engine.assets.textures manifest codec returned non-json path='{path}' output_kind='{output_kind}' err='{e}'"))?;
                annotate_manifest(&mut value, &path);
                state.manifest_cache.insert(path.clone(), value.clone());
                return Ok(value);
            }
            Err(e) => errors.push(format!("{output_kind}: {e}")),
        }
    }
    Err(format!("engine.assets.textures manifest decode failed path='{path}' errors=[{}]", errors.join(" | ")))
}

fn annotate_manifest(value: &mut serde_json::Value, path: &str) {
    let Some(map) = value.as_object_mut() else { return; };
    map.entry("schema".to_owned()).or_insert_with(|| serde_json::Value::String("newengine.assets.textures.manifest.v1".to_owned()));
    map.insert("gateway".to_owned(), serde_json::Value::String(ENGINE_ASSETS_TEXTURES_SERVICE_ID.to_owned()));
    map.insert("byte_owner".to_owned(), serde_json::Value::String(ENGINE_ASSET_SERVICE_ID.to_owned()));
    map.insert("semantic_owner".to_owned(), serde_json::Value::String(ENGINE_ASSETS_TEXTURES_SERVICE_ID.to_owned()));
    map.insert("source".to_owned(), serde_json::Value::String(path.to_owned()));
}

fn runtime_texture_packet_from_dictionary_cache(
    state: &mut TextureRuntimeState,
    dictionary_path: &str,
    texture_name: Option<&str>,
    texture_hash: Option<u64>,
) -> Result<RuntimeTextureAsset, String> {
    ensure_runtime_dictionary_cache(state, dictionary_path)?;
    let cache = state
        .runtime_dictionary_cache
        .get(dictionary_path)
        .ok_or_else(|| format!("runtime texture dictionary cache missing after load path='{dictionary_path}'"))?;
    if let Some(hash) = texture_hash {
        let name = cache
            .entry_hash_to_name
            .get(&hash)
            .ok_or_else(|| format!("texture hash '{hash}' is not present in dictionary '{dictionary_path}'"))?;
        return cache
            .entries_by_name
            .get(name)
            .cloned()
            .ok_or_else(|| format!("texture entry '{name}' missing from dictionary cache '{dictionary_path}'"));
    }
    let name = texture_name.ok_or_else(|| format!("runtime texture request requires .ytd@entry path='{dictionary_path}'"))?;
    cache
        .entries_by_name
        .get(&name.to_ascii_lowercase())
        .cloned()
        .ok_or_else(|| format!("texture entry '{name}' is not present in dictionary '{dictionary_path}'"))
}

fn ensure_runtime_dictionary_cache(state: &mut TextureRuntimeState, dictionary_path: &str) -> Result<(), String> {
    if state.runtime_dictionary_cache.contains_key(dictionary_path) {
        return Ok(());
    }
    let body = state
        .client
        .decode_v1(&AssetDecodeRequest {
            logical_path: dictionary_path.to_owned(),
            output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
            selector: serde_json::Value::Null,
        })
        .map_err(|e| format!("engine.assets listfile body decode failed path='{dictionary_path}' output='{ASSET_LIST_FILE_BODY_OUTPUT}' err='{e}'"))?;
    let dictionary = newengine_texture_container::parse(&body)
        .map_err(|e| format!("engine.assets.textures dictionary parse failed path='{dictionary_path}' err='{e}'"))?;
    let mut cache = RuntimeTextureDictionaryCache::default();
    for meta in dictionary.entries() {
        let format = RuntimeTextureFormat::from_name(&meta.format)
            .ok_or_else(|| format!("unsupported runtime texture format path='{dictionary_path}' entry='{}' format='{}'", meta.name, meta.format))?;
        let view = dictionary
            .entry(&meta.name)
            .map_err(|e| format!("texture entry lookup failed path='{dictionary_path}' entry='{}' err='{e}'", meta.name))?;
        let mut mips = Vec::with_capacity(meta.mips.len());
        for mip in &meta.mips {
            let bytes = view
                .mip_bytes(mip.level)
                .ok_or_else(|| format!("missing mip bytes path='{dictionary_path}' entry='{}' level={}", meta.name, mip.level))?;
            mips.push(RuntimeTextureMip { level: mip.level, width: mip.width, height: mip.height, bytes: bytes.to_vec() });
        }
        let name_key = meta.name.to_ascii_lowercase();
        cache.entry_hash_to_name.insert(meta.name_hash, name_key.clone());
        cache.entries_by_name.insert(name_key, RuntimeTextureAsset { width: meta.width, height: meta.height, format, mips });
    }
    log::debug!(
        "assets.textures.entry_runtime_v1: dictionary cache loaded path='{}' entries={} policy='decode .ytd once, select many @entries'",
        dictionary_path,
        cache.entries_by_name.len()
    );
    state.runtime_dictionary_cache.insert(dictionary_path.to_owned(), cache);
    Ok(())
}

fn rgba8_packet_from_runtime(packet: &RuntimeTextureAsset) -> Result<Rgba8TextureAsset, String> {
    let base = packet
        .mips
        .iter()
        .find(|mip| mip.level == 0)
        .or_else(|| packet.mips.first())
        .ok_or_else(|| "runtime texture packet has no mip levels".to_owned())?;
    let rgba = match packet.format {
        RuntimeTextureFormat::Rgba8Unorm | RuntimeTextureFormat::Rgba8Srgb => base.bytes.clone(),
        _ => newengine_texture_container::decode_bcn_to_rgba8(packet.format.as_str(), packet.width, packet.height, &base.bytes)
            .map_err(|e| format!("runtime texture RGBA8 debug decode failed format='{}' err='{e}'", packet.format.as_str()))?,
    };
    Rgba8TextureAsset::new(packet.width, packet.height, rgba)
}

fn texture_runtime_wire(packet: RuntimeTextureAsset) -> Vec<u8> {
    let (payload, layout) = packet.concatenated_payload_and_layout();
    let header_len = newengine_assets_api::texture_wire::RUNTIME_HEADER_LEN;
    let record_len = newengine_assets_api::texture_wire::RUNTIME_MIP_RECORD_LEN;
    let mut out = Vec::with_capacity(header_len + layout.len() * record_len + payload.len());
    out.extend_from_slice(&newengine_assets_api::texture_wire::MAGIC);
    out.extend_from_slice(&newengine_assets_api::texture_wire::VERSION_RUNTIME_V2.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&packet.format.as_wire_id().to_le_bytes());
    out.extend_from_slice(&(layout.len() as u16).to_le_bytes());
    out.extend_from_slice(&packet.width.to_le_bytes());
    out.extend_from_slice(&packet.height.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for mip in layout {
        out.extend_from_slice(&(mip.level as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&mip.width.to_le_bytes());
        out.extend_from_slice(&mip.height.to_le_bytes());
        out.extend_from_slice(&(mip.offset as u32).to_le_bytes());
        out.extend_from_slice(&(mip.byte_len as u32).to_le_bytes());
    }
    out.extend_from_slice(&payload);
    out
}

fn texture_rgba8_wire(packet: Rgba8TextureAsset) -> Vec<u8> {
    let mut out = Vec::with_capacity(newengine_assets_api::texture_wire::HEADER_LEN + packet.rgba.len());
    out.extend_from_slice(&newengine_assets_api::texture_wire::MAGIC);
    out.extend_from_slice(&newengine_assets_api::texture_wire::VERSION_RGBA8_V1.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&packet.width.to_le_bytes());
    out.extend_from_slice(&packet.height.to_le_bytes());
    out.extend_from_slice(&(packet.rgba.len() as u32).to_le_bytes());
    out.extend_from_slice(&packet.rgba);
    out
}

fn entry_runtime_blob(state: &mut TextureRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    let request = match texture_ref_request_from_payload(payload.as_slice(), textures_method::ENTRY_RUNTIME_V1) {
        Ok(request) => request,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let texture_ref = match texture_ref_from_request(&request) {
        Ok(texture_ref) => texture_ref,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let (reference, texture_name, texture_hash_from_ref) = match parse_texture_reference(&texture_ref) {
        Ok(parsed) => parsed,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let texture_hash = request.texture_hash.or(texture_hash_from_ref);
    let canonical = reference.canonical.clone();
    if let Some(packet) = state.runtime_packet_cache.get(&canonical).cloned() {
        log::debug!(
            "assets.textures.entry_runtime_v1: cache hit ref='{}' dictionary='{}'",
            canonical,
            reference.logical_path
        );
        return RResult::ROk(Blob::from(texture_runtime_wire(packet)));
    }
    match runtime_texture_packet_from_dictionary_cache(state, &reference.logical_path, texture_name.as_deref(), texture_hash) {
        Ok(packet) => {
            state.runtime_packet_cache.insert(canonical, packet.clone());
            RResult::ROk(Blob::from(texture_runtime_wire(packet)))
        }
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

fn entry_rgba8_blob(state: &mut TextureRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    let request = match texture_ref_request_from_payload(payload.as_slice(), textures_method::ENTRY_RGBA8_V1) {
        Ok(request) => request,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let texture_ref = match texture_ref_from_request(&request) {
        Ok(texture_ref) => texture_ref,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let (reference, texture_name, texture_hash_from_ref) = match parse_texture_reference(&texture_ref) {
        Ok(parsed) => parsed,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let texture_hash = request.texture_hash.or(texture_hash_from_ref);
    let canonical = reference.canonical.clone();
    if let Some(packet) = state.rgba8_packet_cache.get(&canonical).cloned() {
        log::debug!(
            "assets.textures.entry_rgba8_v1: cache hit ref='{}' dictionary='{}'",
            canonical,
            reference.logical_path
        );
        return RResult::ROk(Blob::from(texture_rgba8_wire(packet)));
    }
    match runtime_texture_packet_from_dictionary_cache(state, &reference.logical_path, texture_name.as_deref(), texture_hash)
        .and_then(|packet| rgba8_packet_from_runtime(&packet))
    {
        Ok(packet) => {
            state.rgba8_packet_cache.insert(canonical, packet.clone());
            RResult::ROk(Blob::from(texture_rgba8_wire(packet)))
        }
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

fn manifest_blob(state: &mut TextureRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    if payload.is_empty() {
        return ok_json(serde_json::json!({
            "schema": "newengine.assets.textures.service_manifest.v1",
            "gateway": ENGINE_ASSETS_TEXTURES_SERVICE_ID,
            "provider": "StarVaultTexturesRuntimeProvider",
            "byte_owner": ENGINE_ASSET_SERVICE_ID,
            "semantic_owner": ENGINE_ASSETS_TEXTURES_SERVICE_ID,
            "methods": TEXTURES_SERVICE_METHODS,
            "entry_schema": {
                "required_fields": ["name", "stable_hash", "pixel_format", "color_space", "dimensions", "mip_count"]
            }
        }));
    }
    let request = match texture_manifest_request_from_payload(payload.as_slice(), textures_method::MANIFEST_JSON_V1) {
        Ok(request) => request,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    match manifest_json(state, request) {
        Ok(value) => ok_json(value),
        Err(e) => RResult::RErr(RString::from(e)),
    }
}

fn invoke_json(state: &mut TextureRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let method = value.get("method").and_then(|v| v.as_str()).unwrap_or(textures_method::DESCRIBE_REF_JSON_V1);
    match method {
        textures_method::DESCRIBE_REF_JSON_V1 | textures_method::VALIDATE_REF_V1 => {
            let request = serde_json::from_value::<TextureRefRequest>(value.get("request").cloned().unwrap_or_default())
                .unwrap_or_default();
            match validate_texture_ref(state, request) {
                Ok(v) => ok_json(v),
                Err(e) => ok_json(invalid_texture_ref(e)),
            }
        }
        textures_method::MANIFEST_JSON_V1 => {
            let request = serde_json::from_value::<TextureManifestRequest>(value.get("request").cloned().unwrap_or_default())
                .unwrap_or_default();
            match manifest_json(state, request) {
                Ok(v) => ok_json(v),
                Err(e) => ok_json(StableDiagnostic { ok: false, code: "textures.manifest_unavailable", message: e, gateway: ENGINE_ASSETS_TEXTURES_SERVICE_ID, byte_owner: ENGINE_ASSET_SERVICE_ID, semantic_owner: ENGINE_ASSETS_TEXTURES_SERVICE_ID }),
            }
        }
        other => RResult::RErr(RString::from(format!("engine.assets.textures: invoke_json cannot return binary packet for '{other}'; call the method directly"))),
    }
}

pub fn textures_gateway_service(client: AssetServiceClient) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        TEXTURES_SERVICE_ID,
        TEXTURES_GATEWAY_OWNER,
        TEXTURES_BACKEND_CAPABILITY_ID,
        TEXTURES_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_ASSETS_TEXTURES_SERVICE_ID)
    .protocol(TEXTURES_RUNTIME_CONTRACT)
    .features(["ytd-manifest", "runtime-texture-packet", "rgba8-debug-packet", "strict-ref-validation"])
    .notes("Engine texture runtime service. .ytd semantics live in engine.assets.textures; VFS/raw bytes/codec dispatch remain in engine.assets.");

    JsonServiceRouter::with_state(TEXTURES_SERVICE_ID, TextureRuntimeState::new(client))
        .describe_json(&description)
        .info(textures_service_info)
        .blob(textures_method::MANIFEST_JSON_V1, manifest_blob)
        .post_json_result::<TextureRefRequest, TextureRefValidation, _>(textures_method::VALIDATE_REF_V1, |state, request| validate_texture_ref(state, request))
        .post_json_result::<TextureRefRequest, TextureRefValidation, _>(textures_method::DESCRIBE_REF_JSON_V1, |state, request| validate_texture_ref(state, request))
        .blob(textures_method::ENTRY_RUNTIME_V1, entry_runtime_blob)
        .blob(textures_method::ENTRY_RGBA8_V1, entry_rgba8_blob)
        .blob(textures_method::INVOKE_JSON, invoke_json)
        .blob(textures_method::SHUTDOWN_V1, |_state, _payload| ok_empty_blob())
        .into_service_v1()
}

pub fn register_textures_gateway_best_effort(client: AssetServiceClient) -> bool {
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
        service_kind: EngineServiceKind::Textures,
        provider_service: TEXTURES_SERVICE_ID,
        provider_route: "engine.assets.starvault.textures",
        capability: TEXTURES_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: TEXTURES_GATEWAY_OWNER,
        service: textures_gateway_service(client),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_raw_image_ref_before_vfs() {
        assert!(parse_texture_reference("textures/foo.png").is_err());
        assert!(parse_texture_reference("textures/foo.jpg").is_err());
        assert!(parse_texture_reference("textures/foo.dds").is_err());
    }

    #[test]
    fn rejects_retired_texture_dictionary_ref_before_vfs() {
        assert!(parse_texture_reference("textures/foo.rawtex@bar").is_err());
    }

    #[test]
    fn rejects_ytd_without_entry() {
        assert!(parse_texture_reference("textures/foo.ytd").is_err());
    }

    #[test]
    fn accepts_ytd_entry_and_hash() {
        let (_, name, hash) = parse_texture_reference("textures/foo.ytd@bar").unwrap();
        assert_eq!(name.as_deref(), Some("bar"));
        assert_eq!(hash, None);
        let (_, name, hash) = parse_texture_reference("textures/foo.ytd@hash:123456").unwrap();
        assert_eq!(name, None);
        assert_eq!(hash, Some(123456));
    }
}
