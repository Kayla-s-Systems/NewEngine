use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;

use crate::{DefinitionEntryV1, DefinitionRefRequest};

/// Resolve one typed Definition Entry through the canonical engine.assets.definitions gateway.
/// The definitions runtime owns the YTYP semantic DTO; callers do not need to deserialize the
/// provider response into ad-hoc JSON objects.
pub fn load_definition_entry_v1(definition_ref: &str) -> Result<DefinitionEntryV1, String> {
    let definition_ref = definition_ref.trim();
    if definition_ref.is_empty() {
        return Err("definition_ref must not be empty".to_owned());
    }
    let payload = serde_json::to_vec(&DefinitionRefRequest {
        definition_ref: definition_ref.to_owned(),
        ..Default::default()
    })
    .map_err(|error| error.to_string())?;
    match newengine_plugin_host::call_service_v1(
        RString::from(newengine_assets_api::ENGINE_ASSETS_DEFINITIONS_SERVICE_ID),
        RString::from(newengine_assets_api::definitions_method::ENTRY_JSON_V1),
        Blob::from(payload),
    ) {
        RResult::ROk(bytes) => serde_json::from_slice(bytes.as_slice()).map_err(|error| {
            format!("definitions entry decode failed ref='{definition_ref}' err='{error}'")
        }),
        RResult::RErr(error) => Err(error.to_string()),
    }
}

/// Return one authored metadata namespace from the normalized DefinitionEntryV1 envelope.
/// Projection stores both source `metadata` and `namespaces` under `arbitrary_metadata`.
pub fn definition_metadata_namespace<'a>(
    entry: &'a DefinitionEntryV1,
    namespace: &str,
) -> Option<&'a serde_json::Value> {
    let namespace = namespace.trim();
    if namespace.is_empty() {
        return None;
    }
    entry
        .arbitrary_metadata
        .get("metadata")
        .and_then(|value| value.get(namespace))
        .or_else(|| {
            entry
                .arbitrary_metadata
                .get("namespaces")
                .and_then(|value| value.get(namespace))
        })
        .or_else(|| entry.arbitrary_metadata.get(namespace))
}

#[inline]
pub fn metadata_value_string(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.replace('\\', "/"))
}

#[inline]
pub fn metadata_value_f32(value: &serde_json::Value) -> Option<f32> {
    value
        .as_f64()
        .map(|value| value as f32)
        .or_else(|| {
            value
                .as_str()
                .and_then(|value| value.trim().parse::<f32>().ok())
        })
        .filter(|value| value.is_finite())
}

#[inline]
pub fn metadata_value_bool(value: &serde_json::Value) -> Option<bool> {
    value.as_bool().or_else(|| {
        let raw = value.as_str()?.trim().to_ascii_lowercase();
        match raw.as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn metadata_namespace_reads_normalized_definition_envelope() {
        let entry = DefinitionEntryV1 {
            arbitrary_metadata: BTreeMap::from([
                (
                    "metadata".to_owned(),
                    serde_json::json!({"newengine.game_ready": {"player": {"hair_enabled": true}}}),
                ),
                (
                    "namespaces".to_owned(),
                    serde_json::json!({"newengine.audio": {"enabled": true}}),
                ),
            ]),
            ..Default::default()
        };
        assert_eq!(
            definition_metadata_namespace(&entry, "newengine.game_ready")
                .and_then(|value| value.get("player"))
                .and_then(|value| value.get("hair_enabled"))
                .and_then(metadata_value_bool),
            Some(true)
        );
        assert!(definition_metadata_namespace(&entry, "newengine.audio").is_some());
    }
}
