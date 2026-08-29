//! Serde helpers for materials.

use crate::api::MaterialDescriptor;

/// Serialize a descriptor to JSON.
#[inline]
pub fn to_json(desc: &MaterialDescriptor) -> Result<String, serde_json::Error> {
    serde_json::to_string(desc)
}

/// Deserialize a descriptor from JSON.
#[inline]
pub fn from_json(json: &str) -> Result<MaterialDescriptor, serde_json::Error> {
    serde_json::from_str(json)
}
