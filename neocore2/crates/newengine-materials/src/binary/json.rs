use crate::api::MaterialDescriptor;

use super::codec::{decode_asset, encode_asset};
use super::error::{MaterialBinaryError, MaterialBinaryResult};
use super::types::MaterialBinaryAsset;

/// Parses JSON into a [`MaterialDescriptor`] and writes it as a `.nemat` container.
#[inline]
pub fn encode_asset_from_json(name: &str, json: &str) -> MaterialBinaryResult<Vec<u8>> {
    let desc: MaterialDescriptor =
        serde_json::from_str(json).map_err(|_| MaterialBinaryError::InvalidJson)?;

    encode_asset(&MaterialBinaryAsset {
        name: name.to_string(),
        desc,
    })
}

/// Decodes a `.nemat` container and serializes its descriptor as JSON.
#[inline]
pub fn decode_asset_to_json(bytes: &[u8]) -> MaterialBinaryResult<String> {
    let asset = decode_asset(bytes)?;
    serde_json::to_string(&asset.desc).map_err(|_| MaterialBinaryError::JsonSerializeFailed)
}
