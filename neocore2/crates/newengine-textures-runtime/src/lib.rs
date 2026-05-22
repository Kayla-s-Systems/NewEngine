#![forbid(unsafe_op_in_unsafe_fn)]

//! Engine-owned `engine.textures` runtime service.
//!
//! `.ytd` semantic ownership lives here. The service deliberately uses
//! `engine.assets` only as the byte/VFS/codec owner, then returns texture-domain
//! DTOs or stable runtime texture packets to consumers.

use abi_stable::std_types::{RResult, RString};
use newengine_assets::{AssetDecodeRequest, AssetServiceClient, Rgba8TextureAsset, RuntimeTextureAsset};
use newengine_assets_api::{
    textures_method, AssetReference, ENGINE_ASSET_SERVICE_ID, ENGINE_TEXTURES_SERVICE_ID,
    TEXTURES_BACKEND_CAPABILITY_ID, TEXTURES_RUNTIME_CONTRACT, TEXTURES_SERVICE_ID,
    TEXTURES_SERVICE_METHODS,
};
use newengine_plugin_api::Blob;
use newengine_service_api::EngineServiceKind;
use newengine_service_kit::{
    engine_owned_service_description, ok_empty_blob, ok_json, payload_json,
    register_engine_owned_gateway_service_best_effort, EngineOwnedGatewayDecl, JsonServiceRouter,
};
use serde::{Deserialize, Serialize};

pub const TEXTURES_GATEWAY_OWNER: &str = "newengine-textures-runtime.engine-owned-provider";

#[derive(Clone)]
pub struct TextureRuntimeState {
    client: AssetServiceClient,
}

impl TextureRuntimeState {
    #[inline]
    pub fn new(client: AssetServiceClient) -> Self { Self { client } }
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
        gateway: ENGINE_TEXTURES_SERVICE_ID,
        provider: "EngineOwnedTexturesRuntimeProvider",
        contract: TEXTURES_RUNTIME_CONTRACT,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_TEXTURES_SERVICE_ID,
        methods: TEXTURES_SERVICE_METHODS,
        validation_policy: "accept .ytd@entry and .ytd@hash:<u64>; reject raw images, .neytd and .ytd without @entry",
    }
}

fn invalid_texture_ref(message: impl Into<String>) -> StableDiagnostic {
    StableDiagnostic {
        ok: false,
        code: "textures.invalid_ref",
        message: message.into(),
        gateway: ENGINE_TEXTURES_SERVICE_ID,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_TEXTURES_SERVICE_ID,
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
        return Err("textures.manifest_json_v1 requires source or dictionary_path ending in .ytd".to_owned());
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
    if lower.contains(".neytd") {
        return Err("engine.textures rejects .neytd; authored/runtime texture refs must use .ytd@entry".to_owned());
    }
    let path_part = lower.split('@').next().unwrap_or(&lower);
    if [".png", ".jpg", ".jpeg", ".dds", ".tga", ".webp", ".bmp"].iter().any(|ext| path_part.ends_with(ext)) {
        return Err("engine.textures rejects raw source image paths; importers must compile them into .ytd entries".to_owned());
    }
    if entry_required && !lower.contains('@') {
        return Err("engine.textures runtime refs require .ytd@entry or .ytd@hash:<u64>; .ytd alone is a dictionary, not a texture entry".to_owned());
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

fn runtime_packet_summary(packet: &RuntimeTextureAsset) -> TexturePacketSummary {
    let color_space = if packet.format.as_str().contains("SRGB") { "srgb" } else { "linear" };
    TexturePacketSummary {
        width: packet.width,
        height: packet.height,
        pixel_format: packet.format.as_str().to_owned(),
        color_space: color_space.to_owned(),
        mip_count: packet.mips.len(),
    }
}

fn validate_texture_ref(state: &TextureRuntimeState, request: TextureRefRequest) -> Result<TextureRefValidation, String> {
    let texture_ref = texture_ref_from_request(&request)?;
    let (reference, texture_name, texture_hash_from_ref) = parse_texture_reference(&texture_ref)?;
    let texture_hash = request.texture_hash.or(texture_hash_from_ref);
    let packet = state
        .client
        .texture_dictionary_runtime_v1_typed(&reference.logical_path, texture_name.as_deref(), texture_hash)
        .map_err(|e| format!("engine.textures validation failed ref='{}' err='{}'", reference.canonical, e))?;
    Ok(TextureRefValidation {
        ok: true,
        gateway: ENGINE_TEXTURES_SERVICE_ID,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_TEXTURES_SERVICE_ID,
        logical_path: reference.logical_path,
        entry: texture_name,
        texture_hash,
        canonical: reference.canonical,
        packet: Some(runtime_packet_summary(&packet)),
        warnings: Vec::new(),
    })
}

fn manifest_json(state: &TextureRuntimeState, request: TextureManifestRequest) -> Result<serde_json::Value, String> {
    let path = manifest_source_from_request(&request)?;
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
                    .map_err(|e| format!("engine.textures manifest codec returned non-json path='{path}' output_kind='{output_kind}' err='{e}'"))?;
                annotate_manifest(&mut value, &path);
                return Ok(value);
            }
            Err(e) => errors.push(format!("{output_kind}: {e}")),
        }
    }
    Err(format!("engine.textures manifest decode failed path='{path}' errors=[{}]", errors.join(" | ")))
}

fn annotate_manifest(value: &mut serde_json::Value, path: &str) {
    let Some(map) = value.as_object_mut() else { return; };
    map.entry("schema".to_owned()).or_insert_with(|| serde_json::Value::String("newengine.textures.manifest.v1".to_owned()));
    map.insert("gateway".to_owned(), serde_json::Value::String(ENGINE_TEXTURES_SERVICE_ID.to_owned()));
    map.insert("byte_owner".to_owned(), serde_json::Value::String(ENGINE_ASSET_SERVICE_ID.to_owned()));
    map.insert("semantic_owner".to_owned(), serde_json::Value::String(ENGINE_TEXTURES_SERVICE_ID.to_owned()));
    map.insert("source".to_owned(), serde_json::Value::String(path.to_owned()));
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
    match state.client.texture_dictionary_runtime_v1_typed(&reference.logical_path, texture_name.as_deref(), texture_hash) {
        Ok(packet) => RResult::ROk(Blob::from(texture_runtime_wire(packet))),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
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
    match state.client.texture_dictionary_rgba8_v1_typed(&reference.logical_path, texture_name.as_deref(), texture_hash) {
        Ok(packet) => RResult::ROk(Blob::from(texture_rgba8_wire(packet))),
        Err(e) => RResult::RErr(RString::from(e.to_string())),
    }
}

fn manifest_blob(state: &mut TextureRuntimeState, payload: Blob) -> RResult<Blob, RString> {
    if payload.is_empty() {
        return ok_json(serde_json::json!({
            "schema": "newengine.textures.service_manifest.v1",
            "gateway": ENGINE_TEXTURES_SERVICE_ID,
            "provider": "EngineOwnedTexturesRuntimeProvider",
            "byte_owner": ENGINE_ASSET_SERVICE_ID,
            "semantic_owner": ENGINE_TEXTURES_SERVICE_ID,
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
                Err(e) => ok_json(StableDiagnostic { ok: false, code: "textures.manifest_unavailable", message: e, gateway: ENGINE_TEXTURES_SERVICE_ID, byte_owner: ENGINE_ASSET_SERVICE_ID, semantic_owner: ENGINE_TEXTURES_SERVICE_ID }),
            }
        }
        other => RResult::RErr(RString::from(format!("engine.textures: invoke_json cannot return binary packet for '{other}'; call the method directly"))),
    }
}

pub fn textures_gateway_service(client: AssetServiceClient) -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_owned_service_description(
        TEXTURES_SERVICE_ID,
        TEXTURES_GATEWAY_OWNER,
        TEXTURES_BACKEND_CAPABILITY_ID,
        TEXTURES_SERVICE_METHODS.iter().copied(),
    )
    .gateway(ENGINE_TEXTURES_SERVICE_ID)
    .protocol(TEXTURES_RUNTIME_CONTRACT)
    .features(["ytd-manifest", "runtime-texture-packet", "rgba8-debug-packet", "strict-ref-validation"])
    .notes("Engine texture runtime service. .ytd semantics live in engine.textures; VFS/raw bytes/codec dispatch remain in engine.assets.");

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
    register_engine_owned_gateway_service_best_effort(EngineOwnedGatewayDecl {
        gateway: ENGINE_TEXTURES_SERVICE_ID,
        service_kind: EngineServiceKind::Textures,
        provider_service: TEXTURES_SERVICE_ID,
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
    fn rejects_neytd_ref_before_vfs() {
        assert!(parse_texture_reference("textures/foo.neytd@bar").is_err());
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
