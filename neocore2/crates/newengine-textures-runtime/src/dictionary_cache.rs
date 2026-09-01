use newengine_assets::{
    AssetDecodeRequest, Rgba8TextureAsset, RuntimeTextureAsset, RuntimeTextureFormat,
    RuntimeTextureMip,
};
use newengine_assets_api::ASSET_LIST_FILE_BODY_OUTPUT;

use crate::{state::RuntimeTextureDictionaryCache, TextureRuntimeState};

pub(crate) fn runtime_texture_packet_from_dictionary_cache<'a>(
    state: &'a mut TextureRuntimeState,
    dictionary_path: &str,
    texture_name: Option<&str>,
    texture_hash: Option<u64>,
) -> Result<&'a RuntimeTextureAsset, String> {
    ensure_runtime_dictionary_cache(state, dictionary_path)?;
    let cache = state
        .runtime_dictionary_cache
        .get(dictionary_path)
        .ok_or_else(|| {
            format!("runtime texture dictionary cache missing after load path='{dictionary_path}'")
        })?;

    if let Some(hash) = texture_hash {
        let name = cache.entry_hash_to_name.get(&hash).ok_or_else(|| {
            format!("texture hash '{hash}' is not present in dictionary '{dictionary_path}'")
        })?;
        return cache.entries_by_name.get(name).ok_or_else(|| {
            format!("texture entry '{name}' missing from dictionary cache '{dictionary_path}'")
        });
    }

    let name = texture_name.ok_or_else(|| {
        format!("runtime texture request requires .ytd@entry path='{dictionary_path}'")
    })?;
    cache.entries_by_name.get(name).ok_or_else(|| {
        format!("texture entry '{name}' is not present in dictionary '{dictionary_path}'")
    })
}

fn ensure_runtime_dictionary_cache(
    state: &mut TextureRuntimeState,
    dictionary_path: &str,
) -> Result<(), String> {
    if state.runtime_dictionary_cache.contains_key(dictionary_path) {
        return Ok(());
    }

    let body = state
        .client
        .decode_v1(&AssetDecodeRequest {
            logical_path: dictionary_path.to_owned(),
            output_kind: ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
            selector: serde_json::Value::Null,
                    format_descriptor: None,
})
        .map_err(|error| {
            format!("engine.assets listfile body decode failed path='{dictionary_path}' output='{ASSET_LIST_FILE_BODY_OUTPUT}' err='{error}'")
        })?;
    let dictionary = newengine_texture_container::parse(&body).map_err(|error| {
        format!(
            "engine.assets.textures dictionary parse failed path='{dictionary_path}' err='{error}'"
        )
    })?;
    let mut cache = RuntimeTextureDictionaryCache::default();
    cache.entries_by_name.reserve(dictionary.entries().len());
    cache.entry_hash_to_name.reserve(dictionary.entries().len());

    for meta in dictionary.entries() {
        let format = RuntimeTextureFormat::from_name(&meta.format).ok_or_else(|| {
            format!(
                "unsupported runtime texture format path='{dictionary_path}' entry='{}' format='{}'",
                meta.name, meta.format
            )
        })?;
        let view = dictionary.entry(&meta.name).map_err(|error| {
            format!(
                "texture entry lookup failed path='{dictionary_path}' entry='{}' err='{error}'",
                meta.name
            )
        })?;
        let mut mips = Vec::with_capacity(meta.mips.len());
        for mip in &meta.mips {
            let bytes = view.mip_bytes(mip.level).ok_or_else(|| {
                format!(
                    "missing mip bytes path='{dictionary_path}' entry='{}' level={}",
                    meta.name, mip.level
                )
            })?;
            mips.push(RuntimeTextureMip {
                level: mip.level,
                width: mip.width,
                height: mip.height,
                bytes: bytes.to_vec(),
            });
        }

        let name_key = meta.name.to_ascii_lowercase();
        cache
            .entry_hash_to_name
            .insert(meta.name_hash, name_key.clone());
        cache.entries_by_name.insert(
            name_key,
            RuntimeTextureAsset {
                width: meta.width,
                height: meta.height,
                format,
                mips,
            },
        );
    }

    newengine_ulog_api::ulog::debug!(
        "assets.textures.entry_runtime_v1: dictionary cache loaded path='{}' entries={} policy='decode .ytd once, select many @entries'",
        dictionary_path,
        cache.entries_by_name.len()
    );
    state
        .runtime_dictionary_cache
        .insert(dictionary_path.to_owned(), cache);
    Ok(())
}

pub(crate) fn rgba8_packet_from_runtime(
    packet: &RuntimeTextureAsset,
) -> Result<Rgba8TextureAsset, String> {
    let base = packet
        .mips
        .iter()
        .find(|mip| mip.level == 0)
        .or_else(|| packet.mips.first())
        .ok_or_else(|| "runtime texture packet has no mip levels".to_owned())?;
    let rgba = match packet.format {
        RuntimeTextureFormat::Rgba8Unorm | RuntimeTextureFormat::Rgba8Srgb => base.bytes.clone(),
        _ => newengine_texture_container::decode_bcn_to_rgba8(
            packet.format.as_str(),
            packet.width,
            packet.height,
            &base.bytes,
        )
        .map_err(|error| {
            format!(
                "runtime texture RGBA8 debug decode failed format='{}' err='{error}'",
                packet.format.as_str()
            )
        })?,
    };
    Rgba8TextureAsset::new(packet.width, packet.height, rgba)
}
