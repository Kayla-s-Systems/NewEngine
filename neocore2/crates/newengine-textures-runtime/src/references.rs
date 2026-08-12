use newengine_assets_api::AssetReference;
use serde::de::DeserializeOwned;

use crate::dto::{TextureManifestRequest, TextureRefRequest};

pub(crate) struct ResolvedTextureReference {
    pub(crate) reference: AssetReference,
    pub(crate) texture_name: Option<String>,
    pub(crate) texture_hash: Option<u64>,
    pub(crate) canonical: String,
}

pub(crate) fn texture_ref_from_request(request: &TextureRefRequest) -> Result<String, String> {
    if !request.texture_ref.trim().is_empty() {
        return Ok(normalize_logical_ref(&request.texture_ref));
    }

    let dictionary = normalize_logical_ref(&request.dictionary_path);
    if dictionary.is_empty() {
        return Err(
            "textures API requires texture_ref='.ytd@entry' or dictionary_path + texture_name/hash"
                .to_owned(),
        );
    }
    if let Some(hash) = request.texture_hash {
        return Ok(format!("{dictionary}@hash:{hash}"));
    }

    let entry = request.texture_name.as_deref().unwrap_or("").trim();
    if entry.is_empty() {
        return Err(
            "textures API requires .ytd@entry; .ytd without @entry is not a runtime texture ref"
                .to_owned(),
        );
    }
    Ok(format!("{dictionary}@{entry}"))
}

pub(crate) fn manifest_source_from_request(
    request: &TextureManifestRequest,
) -> Result<String, String> {
    let raw = if !request.source.trim().is_empty() {
        request.source.trim()
    } else if !request.dictionary_path.trim().is_empty() {
        request.dictionary_path.trim()
    } else if !request.texture_ref.trim().is_empty() {
        request
            .texture_ref
            .split('@')
            .next()
            .unwrap_or(request.texture_ref.trim())
    } else {
        return Err(
            "assets.textures.manifest_v1 requires source or dictionary_path ending in .ytd"
                .to_owned(),
        );
    };

    let normalized = normalize_logical_ref(raw);
    reject_non_texture_ref_shape(&normalized, false)?;
    let reference =
        newengine_assets_api::require_asset_reference_extension(&normalized, &["ytd"], false)
            .map_err(|error| error.to_string())?;
    Ok(reference.logical_path)
}

pub(crate) fn texture_ref_request_from_payload(
    payload: &[u8],
    method: &str,
) -> Result<TextureRefRequest, String> {
    request_from_payload(payload, method, "texture_ref='.ytd@entry'", |raw| {
        TextureRefRequest {
            texture_ref: raw.to_owned(),
            ..Default::default()
        }
    })
}

pub(crate) fn texture_manifest_request_from_payload(
    payload: &[u8],
    method: &str,
) -> Result<TextureManifestRequest, String> {
    request_from_payload(payload, method, "source='textures/foo.ytd'", |raw| {
        TextureManifestRequest {
            source: raw.to_owned(),
            ..Default::default()
        }
    })
}

fn request_from_payload<T, F>(
    payload: &[u8],
    method: &str,
    required: &str,
    from_raw: F,
) -> Result<T, String>
where
    T: DeserializeOwned,
    F: FnOnce(&str) -> T,
{
    let trimmed = std::str::from_utf8(payload)
        .map(str::trim)
        .map_err(|error| format!("{method} invalid utf-8 request: {error}"))?;
    if trimmed.is_empty() {
        return Err(format!("{method} requires {required}"));
    }
    if trimmed.starts_with('{') {
        serde_json::from_str(trimmed)
            .map_err(|error| format!("{method} invalid json request: {error}"))
    } else {
        Ok(from_raw(trimmed.trim_matches('"')))
    }
}

pub(crate) fn normalize_logical_ref(value: &str) -> String {
    let mut value = value.trim();
    while value.starts_with("./") || value.starts_with(".\\") {
        value = &value[2..];
    }
    value = value.trim_start_matches(['/', '\\']);

    let mut out = String::with_capacity(value.len());
    let mut previous_slash = false;
    for character in value.chars() {
        if character == '/' || character == '\\' {
            if !previous_slash {
                out.push('/');
            }
            previous_slash = true;
        } else {
            out.push(character);
            previous_slash = false;
        }
    }
    out
}

fn reject_non_texture_ref_shape(value: &str, entry_required: bool) -> Result<(), String> {
    let lower = value.to_ascii_lowercase();
    let path_part = lower.split('@').next().unwrap_or(&lower);
    if [".png", ".jpg", ".jpeg", ".dds", ".tga", ".webp", ".bmp"]
        .iter()
        .any(|extension| path_part.ends_with(extension))
    {
        return Err("engine.assets.textures rejects raw source image paths; importers must compile them into .ytd entries".to_owned());
    }
    if entry_required && !lower.contains('@') {
        return Err("engine.assets.textures runtime refs require .ytd@entry or .ytd@hash:<u64>; .ytd alone is a dictionary, not a texture entry".to_owned());
    }
    Ok(())
}

fn split_hash_selector(entry: &str) -> Result<Option<u64>, String> {
    let Some(rest) = entry.strip_prefix("hash:") else {
        return Ok(None);
    };
    rest.parse::<u64>()
        .map(Some)
        .map_err(|_| format!("invalid .ytd hash selector '{entry}'; expected hash:<u64>"))
}

pub(crate) fn parse_texture_reference(value: &str) -> Result<ResolvedTextureReference, String> {
    let normalized = normalize_logical_ref(value);
    reject_non_texture_ref_shape(&normalized, true)?;
    let reference =
        newengine_assets_api::require_asset_reference_extension(&normalized, &["ytd"], true)
            .map_err(|error| error.to_string())?;
    let entry = reference.entry.clone().unwrap_or_default();
    let texture_hash = split_hash_selector(&entry)?;
    let texture_name = texture_hash.is_none().then(|| entry.to_ascii_lowercase());
    let canonical = reference.canonical.clone();

    Ok(ResolvedTextureReference {
        reference,
        texture_name,
        texture_hash,
        canonical,
    })
}

pub(crate) fn resolve_texture_request(
    request: &TextureRefRequest,
) -> Result<ResolvedTextureReference, String> {
    let texture_ref = texture_ref_from_request(request)?;
    let mut resolved = parse_texture_reference(&texture_ref)?;
    resolved.texture_hash = request.texture_hash.or(resolved.texture_hash);
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_collapses_slashes_in_one_pass() {
        assert_eq!(
            normalize_logical_ref(r".\\textures\\foo//bar.ytd@x"),
            "textures/foo/bar.ytd@x"
        );
    }
}
