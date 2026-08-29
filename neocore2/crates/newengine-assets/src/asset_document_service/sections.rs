use super::*;

pub(super) fn identity_section(
    asset_ref: &str,
    logical_path: &str,
    entry_selector: Option<&str>,
    extension: &str,
) -> AssetDocumentSection {
    AssetDocumentSection {
        id: "identity".to_owned(),
        title: "Identity".to_owned(),
        fields: vec![
            string_field("asset_ref", "Asset Ref", asset_ref, false),
            string_field("logical_path", "Logical Path", logical_path, false),
            string_field(
                "entry_selector",
                "Entry Selector",
                entry_selector.unwrap_or("<whole file>"),
                false,
            ),
            string_field("extension", "Extension", extension, false),
        ],
    }
}

pub(super) fn type_descriptor_section(
    descriptor: Option<&AssetFileTypeDescriptor>,
) -> AssetDocumentSection {
    let Some(desc) = descriptor else {
        return AssetDocumentSection {
            id: "type".to_owned(),
            title: "Type descriptor".to_owned(),
            fields: vec![string_field("known", "Known Type", "false", false)],
        };
    };
    AssetDocumentSection {
        id: "type".to_owned(),
        title: "Type descriptor".to_owned(),
        fields: vec![
            string_field("asset_kind", "Asset Kind", &desc.asset_kind, false),
            string_field("codec_type", "Codec Type", &desc.codec_type, false),
            string_field("container", "Container", &desc.container, false),
            string_field("byte_owner", "Byte Owner", &desc.byte_owner, false),
            string_field(
                "semantic_gateway",
                "Semantic Gateway",
                &desc.semantic_gateway,
                false,
            ),
            string_field(
                "selector_syntax",
                "Selector Syntax",
                desc.selector_syntax.as_deref().unwrap_or("<none>"),
                false,
            ),
            json_field(
                "content_kind",
                "Content Kind",
                json!(desc.content_kind),
                false,
            ),
            json_field("outputs", "Outputs", json!(desc.outputs), false),
            json_field(
                "consumer_domains",
                "Consumers",
                json!(desc.consumer_domains),
                false,
            ),
        ],
    }
}

pub(super) fn provider_contract_section(
    descriptor: Option<&AssetFileTypeDescriptor>,
) -> AssetDocumentSection {
    let Some(desc) = descriptor else {
        return AssetDocumentSection {
            id: "provider_contract".to_owned(),
            title: "Provider contracts".to_owned(),
            fields: vec![string_field(
                "contract",
                "Contract",
                "unregistered type",
                false,
            )],
        };
    };
    AssetDocumentSection {
        id: "provider_contract".to_owned(),
        title: "Provider contracts".to_owned(),
        fields: vec![
            string_field(
                "handler_service",
                "Handler Service",
                &desc.handler_service,
                false,
            ),
            string_field("read_method", "Read Method", &desc.read_method, false),
            string_field(
                "inspect_contract",
                "Inspect Contract",
                &desc.inspect_contract,
                false,
            ),
            string_field("edit_contract", "Edit Contract", &desc.edit_contract, false),
            json_field(
                "preview_provider",
                "Preview Provider",
                json!(desc.preview_provider),
                false,
            ),
            json_field(
                "runtime_ready",
                "Runtime Ready",
                json!(desc.runtime_ready),
                false,
            ),
        ],
    }
}

pub(super) fn editable_schema_section(
    title: &str,
    descriptor: Option<&AssetFileTypeDescriptor>,
    can_apply_patch: bool,
) -> AssetDocumentSection {
    let document_kind = descriptor
        .map(|d| d.asset_kind.as_str())
        .unwrap_or("asset_document");
    let mut fields = vec![
        schema_field(
            "display_name",
            "Display Name",
            json!(title),
            SchemaValueKindV1::String,
            true,
        ),
        schema_field(
            "editor_tags",
            "Editor Tags",
            json!([]),
            SchemaValueKindV1::StringList,
            true,
        ),
        schema_field(
            "import_notes",
            "Import Notes",
            json!(""),
            SchemaValueKindV1::String,
            true,
        ),
        string_field("document_kind", "Document Kind", document_kind, false),
        json_field(
            "save_enabled",
            "Save Enabled",
            json!(can_apply_patch),
            false,
        ),
    ];
    if !can_apply_patch {
        fields.push(string_field(
            "save_blocked_by",
            "Save Blocked By",
            "missing format/package writer capability",
            false,
        ));
    }
    AssetDocumentSection {
        id: "editable_schema".to_owned(),
        title: "Editable schema".to_owned(),
        fields,
    }
}

pub(super) fn edit_policy_section(
    descriptor: Option<&AssetFileTypeDescriptor>,
) -> AssetDocumentSection {
    let schema_editable = descriptor
        .map(|d| d.schema_editable || d.editable || !d.edit_contract.trim().is_empty())
        .unwrap_or(false);
    let can_apply_patch = descriptor
        .map(|d| d.write_back_available && !d.writer_capability.trim().is_empty())
        .unwrap_or(false);
    let writer_capability = descriptor
        .map(|d| d.writer_capability.as_str())
        .unwrap_or("");
    AssetDocumentSection {
        id: "edit_policy".to_owned(),
        title: "Edit policy".to_owned(),
        fields: vec![
            json_field(
                "schema_editable",
                "Schema Editable",
                json!(schema_editable),
                false,
            ),
            json_field(
                "can_apply_patch",
                "Can Apply Patch",
                json!(can_apply_patch),
                false,
            ),
            string_field(
                "patch_route",
                "Patch Route",
                ENGINE_ASSETS_EDIT_SERVICE_ID,
                false,
            ),
            string_field(
                "writer_capability",
                "Writer Capability",
                if writer_capability.is_empty() {
                    "missing"
                } else {
                    writer_capability
                },
                false,
            ),
            string_field(
                "write_owner",
                "Write Owner",
                if can_apply_patch {
                    "format/package writer provider"
                } else {
                    "missing format writer provider"
                },
                false,
            ),
        ],
    }
}

pub(super) fn string_field(
    id: &str,
    label: &str,
    value: &str,
    editable: bool,
) -> AssetDocumentField {
    schema_field(id, label, json!(value), SchemaValueKindV1::String, editable)
}

pub(super) fn json_field(
    id: &str,
    label: &str,
    value: Value,
    editable: bool,
) -> AssetDocumentField {
    schema_field(id, label, value, SchemaValueKindV1::Json, editable)
}

fn schema_field(
    id: &str,
    label: &str,
    value: Value,
    value_kind: SchemaValueKindV1,
    editable: bool,
) -> AssetDocumentField {
    AssetDocumentField {
        id: id.to_owned(),
        label: label.to_owned(),
        value_kind: value_kind.as_str().to_owned(),
        value: value.clone(),
        editable,
        schema_property: Some(schema_property(id, label, value_kind, value, editable)),
        ..AssetDocumentField::default()
    }
}

pub(super) fn schema_property(
    id: &str,
    label: &str,
    value_kind: SchemaValueKindV1,
    value: Value,
    editable: bool,
) -> SchemaPropertyDescriptorV1 {
    let mut property = if editable {
        SchemaPropertyDescriptorV1::editable(id, label, value_kind, value)
    } else {
        SchemaPropertyDescriptorV1::readonly(id, label, value_kind, value)
    };
    property.json_pointer = format!("/properties/{id}");
    property.source_domain = "engine.assets.inspect".to_owned();
    property.tags.push(if editable {
        "editable".to_owned()
    } else {
        "readonly".to_owned()
    });
    property
}
