use newengine_assets::AssetDecodeRequest;
use newengine_assets_api::{
    textures_method, ENGINE_ASSETS_TEXTURES_SERVICE_ID, ENGINE_ASSET_SERVICE_ID,
};

use crate::{
    dto::{
        StableDiagnostic, TextureManifestRequest, TexturePacketSummary, TextureRefRequest,
        TextureRefValidation,
    },
    references::{manifest_source_from_request, resolve_texture_request},
    state::TextureRuntimeState,
};

pub(crate) fn invalid_texture_ref(message: impl Into<String>) -> StableDiagnostic {
    StableDiagnostic {
        ok: false,
        code: "textures.invalid_ref",
        message: message.into(),
        gateway: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
    }
}

pub(crate) fn validate_texture_ref(
    state: &mut TextureRuntimeState,
    request: TextureRefRequest,
) -> Result<TextureRefValidation, String> {
    let resolved = resolve_texture_request(&request)?;
    if let Some(cached) = state.validation_cache.get(&resolved.canonical) {
        newengine_ulog_api::ulog::debug!(
            "assets.textures.validate_ref_v1: cache hit ref='{}' dictionary='{}' policy='manifest-only validation'",
            resolved.canonical,
            resolved.reference.logical_path
        );
        return Ok(cached.clone());
    }

    let manifest = manifest_json(
        state,
        TextureManifestRequest {
            source: resolved.reference.logical_path.clone(),
            ..Default::default()
        },
    )?;
    let summary = texture_summary_from_manifest_entry(
        &manifest,
        resolved.texture_name.as_deref(),
        resolved.texture_hash,
    )
    .map_err(|error| {
        format!(
            "engine.assets.textures validation failed ref='{}' err='{}'",
            resolved.canonical, error
        )
    })?;

    let validation = TextureRefValidation {
        ok: true,
        gateway: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
        byte_owner: ENGINE_ASSET_SERVICE_ID,
        semantic_owner: ENGINE_ASSETS_TEXTURES_SERVICE_ID,
        logical_path: resolved.reference.logical_path,
        entry: resolved.texture_name,
        texture_hash: resolved.texture_hash,
        canonical: resolved.canonical.clone(),
        packet: Some(summary),
        warnings: vec!["validated_by=engine.assets.textures.manifest_only".to_owned()],
    };
    state
        .validation_cache
        .insert(resolved.canonical, validation.clone());
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
            .or_else(|| {
                texture_manifest_metadata_string(entry, "mip_count")
                    .and_then(|value| value.parse::<u32>().ok())
            })
            .unwrap_or(1) as usize,
    })
}

fn texture_manifest_entry_matches(
    entry: &serde_json::Value,
    texture_name: Option<&str>,
    texture_hash: Option<u64>,
) -> bool {
    if let Some(name) = texture_name {
        if entry
            .get("name")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
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

#[inline]
fn texture_manifest_u32(entry: &serde_json::Value, key: &str) -> Option<u32> {
    entry
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

#[inline]
fn texture_manifest_string(entry: &serde_json::Value, key: &str) -> Option<String> {
    entry
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

#[inline]
fn texture_manifest_metadata_string(entry: &serde_json::Value, key: &str) -> Option<String> {
    entry
        .get("metadata")
        .and_then(|metadata| metadata.get(key))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

pub(crate) fn manifest_json(
    state: &mut TextureRuntimeState,
    request: TextureManifestRequest,
) -> Result<serde_json::Value, String> {
    let path = manifest_source_from_request(&request)?;
    if let Some(cached) = state.manifest_cache.get(&path) {
        newengine_ulog_api::ulog::debug!(
            "assets.textures.manifest_v1: cache hit source='{}' policy='manifest-only fast path'",
            path
        );
        return Ok(cached.clone());
    }

    let mut errors = Vec::new();
    for output_kind in [
        textures_method::MANIFEST_JSON_V1,
        "texture_dictionary.manifest_json",
        "texture_dictionary.manifest_json_v1",
        "asset.list_file_manifest",
    ] {
        match state.client.decode_v1(&AssetDecodeRequest {
            logical_path: path.clone(),
            output_kind: output_kind.to_owned(),
            selector: serde_json::Value::Null,
        }) {
            Ok(bytes) => {
                let mut value = serde_json::from_slice::<serde_json::Value>(&bytes).map_err(
                    |error| {
                        format!("engine.assets.textures manifest codec returned non-json path='{path}' output_kind='{output_kind}' err='{error}'")
                    },
                )?;
                annotate_manifest(&mut value, &path);
                state.manifest_cache.insert(path, value.clone());
                return Ok(value);
            }
            Err(error) => errors.push(format!("{output_kind}: {error}")),
        }
    }

    Err(format!(
        "engine.assets.textures manifest decode failed path='{path}' errors=[{}]",
        errors.join(" | ")
    ))
}

fn annotate_manifest(value: &mut serde_json::Value, path: &str) {
    let Some(map) = value.as_object_mut() else {
        return;
    };
    map.entry("schema".to_owned()).or_insert_with(|| {
        serde_json::Value::String("newengine.assets.textures.manifest.v1".to_owned())
    });
    map.insert(
        "gateway".to_owned(),
        serde_json::Value::String(ENGINE_ASSETS_TEXTURES_SERVICE_ID.to_owned()),
    );
    map.insert(
        "byte_owner".to_owned(),
        serde_json::Value::String(ENGINE_ASSET_SERVICE_ID.to_owned()),
    );
    map.insert(
        "semantic_owner".to_owned(),
        serde_json::Value::String(ENGINE_ASSETS_TEXTURES_SERVICE_ID.to_owned()),
    );
    map.insert(
        "source".to_owned(),
        serde_json::Value::String(path.to_owned()),
    );
}
