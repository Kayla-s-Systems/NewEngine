use super::editing::field_pointer;
use super::listing::{apply_staged_projection, manifest_entry_byte_len};
use super::values::parse_field_value;
use super::*;

#[test]
fn manifest_entry_size_uses_provider_metadata() {
    let metadata =
        std::collections::BTreeMap::from([("payload_len".to_owned(), "4096".to_owned())]);
    assert_eq!(manifest_entry_byte_len(&metadata), Some(4096));
}

#[test]
fn refuses_to_invent_missing_field_pointer() {
    let field = AssetDocumentField {
        id: "name".to_owned(),
        editable: true,
        ..AssetDocumentField::default()
    };
    assert_eq!(field_pointer(&field), None);
}

#[test]
fn parses_schema_driven_boolean_values() {
    let field = AssetDocumentField {
        label: "Enabled".to_owned(),
        value_kind: "bool".to_owned(),
        ..AssetDocumentField::default()
    };
    assert_eq!(
        parse_field_value(&field, &Value::String("true".to_owned())).unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn staged_delete_is_projected_over_container_manifest() {
    let mut entries = vec![
        newengine_assets_api::AssetEntryManifest {
            name: "albedo".to_owned(),
            entry_ref: "textures/world.ytd@albedo".to_owned(),
            ..Default::default()
        },
        newengine_assets_api::AssetEntryManifest {
            name: "normal".to_owned(),
            entry_ref: "textures/world.ytd@normal".to_owned(),
            ..Default::default()
        },
    ];
    let patches = vec![AssetPatch {
        asset_ref: "textures/world.ytd@albedo".to_owned(),
        operations: vec![AssetPatchOperation {
            op: "delete".to_owned(),
            ..AssetPatchOperation::default()
        }],
        ..AssetPatch::default()
    }];
    apply_staged_projection("textures/world.ytd", &mut entries, &patches);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "normal");
}
