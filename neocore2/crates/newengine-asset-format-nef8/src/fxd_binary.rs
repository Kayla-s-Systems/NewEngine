use flate2::{write::DeflateEncoder, Compression};
use newengine_assets_api::{
    decode_list_file_envelope, encode_list_file, ListFileEncodeRequest,
    LIST_FILE_FULL_HASH_BODY_THRESHOLD,
};
use newengine_vfx_api::FxdDictionaryV1;
#[cfg(test)]
use newengine_vfx_api::FXD_VERSION_V1;
use std::io::Write;

/// Encodes a project-authored FX dictionary into the canonical NEF8 envelope.
pub fn encode_fxd_nef8(
    dictionary: &FxdDictionaryV1,
    _logical_path: &str,
    content_kind: u32,
    content_schema_version: u16,
) -> Result<Vec<u8>, String> {
    dictionary.validate()?;
    let body = serde_json::to_vec(dictionary)
        .map_err(|error| format!("FXD JSON encode failed: {error}"))?;
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&body)
        .map_err(|error| format!("FXD deflate write failed: {error}"))?;
    let compressed = encoder
        .finish()
        .map_err(|error| format!("FXD deflate finish failed: {error}"))?;
    let body_hash =
        (body.len() >= LIST_FILE_FULL_HASH_BODY_THRESHOLD).then(|| *blake3::hash(&body).as_bytes());
    let entry_count = dictionary
        .textures
        .len()
        .saturating_add(dictionary.effects.len())
        .min(u32::MAX as usize) as u32;

    encode_list_file(ListFileEncodeRequest {
        content_kind,
        content_schema_version,
        entry_count,
        additional_flags: 0,
        min_size_class: 4,
        header_metadata: &[],
        body_stored: &compressed,
        body_uncompressed_len: body.len() as u64,
        body_raw_hash: body_hash,
        stable_file_id: None,
        import_settings_hash: None,
    })
}

/// Decodes the canonical project-owned `.fxd` dictionary.
pub fn decode_fxd_nef8(
    bytes: &[u8],
    content_kind: u32,
    content_schema_version: u16,
) -> Result<FxdDictionaryV1, String> {
    let envelope = decode_list_file_envelope(bytes, content_kind, "<fxd>")?;
    if envelope.header.content_schema_version != content_schema_version {
        return Err(format!(
            "FXD content schema mismatch: got={} expected={}",
            envelope.header.content_schema_version, content_schema_version
        ));
    }
    let dictionary: FxdDictionaryV1 = serde_json::from_slice(&envelope.body)
        .map_err(|error| format!("FXD JSON decode failed: {error}"))?;
    dictionary.validate()?;
    Ok(dictionary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_vfx_api::{FxdEffectV1, FxdLayerKindV1, FxdLayerV1, FxdRenderRoleV1};

    #[test]
    fn fxd_nef8_roundtrip_preserves_project_effect_data() {
        let dictionary = FxdDictionaryV1 {
            effects: vec![FxdEffectV1 {
                id: "weapon.shot".to_owned(),
                layers: vec![FxdLayerV1::Burst {
                    kind: FxdLayerKindV1::Spark,
                    primitive: "cube".to_owned(),
                    role: FxdRenderRoleV1::Transparent,
                    texture: String::new(),
                    billboard: Default::default(),
                    emission_axis: Default::default(),
                    count: 12,
                    scale: [0.004, 0.004, 0.05],
                    color: [1.0, 0.7, 0.2, 1.0],
                    speed_min: 3.0,
                    speed_max: 9.0,
                    cone_angle_degrees: 70.0,
                    size_variance: 0.3,
                    lifetime_variance: 0.2,
                    acceleration: [0.0, -9.81, 0.0],
                    drag_per_second: 0.1,
                    rotation_random_radians: 3.14159,
                    spin_radians_per_second: 4.0,
                    spin_variance: 2.0,
                    lifetime_seconds: 0.25,
                    fade_start_fraction: 0.4,
                    fade_in_fraction: 0.0,
                    depth_softness_m: 0.0,
                }],
                ..FxdEffectV1::default()
            }],
            ..FxdDictionaryV1::default()
        };
        let encoded = encode_fxd_nef8(
            &dictionary,
            "effects/weapons/rifle.fxd",
            35,
            FXD_VERSION_V1 as u16,
        )
        .unwrap();
        let decoded = decode_fxd_nef8(&encoded, 35, FXD_VERSION_V1 as u16).unwrap();
        assert_eq!(decoded, dictionary);
    }
}
