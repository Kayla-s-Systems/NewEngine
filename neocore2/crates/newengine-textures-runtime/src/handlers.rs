use abi_stable::std_types::{RResult, RString};
use newengine_assets_api::{
    textures_method, ENGINE_ASSETS_TEXTURES_SERVICE_ID, ENGINE_ASSET_SERVICE_ID,
    TEXTURES_SERVICE_METHODS,
};
use newengine_plugin_api::Blob;
use newengine_service_kit::{ok_json, payload_json};
use serde::Deserialize;

use crate::{
    dictionary_cache::{rgba8_packet_from_runtime, runtime_texture_packet_from_dictionary_cache},
    dto::{StableDiagnostic, TextureManifestRequest, TextureRefRequest},
    manifest::{invalid_texture_ref, manifest_json, validate_texture_ref},
    references::{
        resolve_texture_request, texture_manifest_request_from_payload,
        texture_ref_request_from_payload,
    },
    service::TEXTURES_PROVIDER_NAME,
    state::TextureRuntimeState,
    wire::{texture_rgba8_wire, texture_runtime_wire},
};

pub(crate) fn entry_runtime_blob(
    state: &mut TextureRuntimeState,
    payload: Blob,
) -> RResult<Blob, RString> {
    let request = match texture_ref_request_from_payload(
        payload.as_slice(),
        textures_method::ENTRY_RUNTIME_V1,
    ) {
        Ok(request) => request,
        Err(error) => return RResult::RErr(RString::from(error)),
    };
    let resolved = match resolve_texture_request(&request) {
        Ok(resolved) => resolved,
        Err(error) => return RResult::RErr(RString::from(error)),
    };

    match runtime_texture_packet_from_dictionary_cache(
        state,
        &resolved.reference.logical_path,
        resolved.texture_name.as_deref(),
        resolved.texture_hash,
    ) {
        Ok(packet) => RResult::ROk(Blob::from(texture_runtime_wire(packet))),
        Err(error) => RResult::RErr(RString::from(error)),
    }
}

pub(crate) fn entry_rgba8_blob(
    state: &mut TextureRuntimeState,
    payload: Blob,
) -> RResult<Blob, RString> {
    let request =
        match texture_ref_request_from_payload(payload.as_slice(), textures_method::ENTRY_RGBA8_V1)
        {
            Ok(request) => request,
            Err(error) => return RResult::RErr(RString::from(error)),
        };
    let resolved = match resolve_texture_request(&request) {
        Ok(resolved) => resolved,
        Err(error) => return RResult::RErr(RString::from(error)),
    };

    if let Some(packet) = state.rgba8_packet_cache.get(&resolved.canonical) {
        newengine_ulog_api::ulog::debug!(
            "assets.textures.entry_rgba8_v1: cache hit ref='{}' dictionary='{}'",
            resolved.canonical,
            resolved.reference.logical_path
        );
        return RResult::ROk(Blob::from(texture_rgba8_wire(packet)));
    }

    let packet = match runtime_texture_packet_from_dictionary_cache(
        state,
        &resolved.reference.logical_path,
        resolved.texture_name.as_deref(),
        resolved.texture_hash,
    )
    .and_then(rgba8_packet_from_runtime)
    {
        Ok(packet) => packet,
        Err(error) => return RResult::RErr(RString::from(error)),
    };
    let bytes = texture_rgba8_wire(&packet);
    state.rgba8_packet_cache.insert(resolved.canonical, packet);
    RResult::ROk(Blob::from(bytes))
}

pub(crate) fn manifest_blob(
    state: &mut TextureRuntimeState,
    payload: Blob,
) -> RResult<Blob, RString> {
    if payload.is_empty() {
        return ok_json(serde_json::json!({
            "schema": "newengine.assets.textures.service_manifest.v1",
            "gateway": ENGINE_ASSETS_TEXTURES_SERVICE_ID,
            "provider": TEXTURES_PROVIDER_NAME,
            "byte_owner": ENGINE_ASSET_SERVICE_ID,
            "semantic_owner": ENGINE_ASSETS_TEXTURES_SERVICE_ID,
            "methods": TEXTURES_SERVICE_METHODS,
            "entry_schema": {
                "required_fields": ["name", "stable_hash", "pixel_format", "color_space", "dimensions", "mip_count"]
            }
        }));
    }

    let request = match texture_manifest_request_from_payload(
        payload.as_slice(),
        textures_method::MANIFEST_JSON_V1,
    ) {
        Ok(request) => request,
        Err(error) => return RResult::RErr(RString::from(error)),
    };
    match manifest_json(state, request) {
        Ok(value) => ok_json(value),
        Err(error) => RResult::RErr(RString::from(error)),
    }
}

#[derive(Deserialize)]
struct TextureInvokeRequest {
    #[serde(default = "default_invoke_method")]
    method: String,
    #[serde(default)]
    request: serde_json::Value,
}

fn default_invoke_method() -> String {
    textures_method::DESCRIBE_REF_JSON_V1.to_owned()
}

pub(crate) fn invoke_json(
    state: &mut TextureRuntimeState,
    payload: Blob,
) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,
        Err(error) => return RResult::RErr(RString::from(error)),
    };
    let invoke = match serde_json::from_value::<TextureInvokeRequest>(value) {
        Ok(invoke) => invoke,
        Err(error) => return RResult::RErr(RString::from(error.to_string())),
    };

    match invoke.method.as_str() {
        textures_method::DESCRIBE_REF_JSON_V1 | textures_method::VALIDATE_REF_V1 => {
            let request = serde_json::from_value::<TextureRefRequest>(invoke.request)
                .unwrap_or_default();
            match validate_texture_ref(state, request) {
                Ok(validation) => ok_json(validation),
                Err(error) => ok_json(invalid_texture_ref(error)),
            }
        }
        textures_method::MANIFEST_JSON_V1 => {
            let request = serde_json::from_value::<TextureManifestRequest>(invoke.request)
                .unwrap_or_default();
            match manifest_json(state, request) {
                Ok(value) => ok_json(value),
                Err(error) => ok_json(StableDiagnostic {
                    ok: false,
                    code: "textures.manifest_unavailable",
                    message: error,
                    gateway: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
                    byte_owner: ENGINE_ASSET_SERVICE_ID,
                    semantic_owner: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
                }),
            }
        }
        other => RResult::RErr(RString::from(format!(
            "engine.assets.textures: invoke_json cannot return binary packet for '{other}'; call the method directly"
        ))),
    }
}
