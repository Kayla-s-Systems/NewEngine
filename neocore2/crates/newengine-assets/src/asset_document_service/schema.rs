use super::*;

pub(super) fn asset_document_schema_type(document: &AssetDocument) -> SchemaTypeDescriptorV1 {
    let mut properties = Vec::new();
    for section in &document.sections {
        for field in &section.fields {
            if let Some(mut property) = field.schema_property.clone() {
                property.tags.push(format!("section:{}", section.id));
                properties.push(property);
            }
        }
    }
    SchemaTypeDescriptorV1 {
        type_id: format!(
            "newengine.assets.document.{}",
            document.asset_kind.replace([' ', '/'], "_")
        ),
        display_name: document.title.clone(),
        domain: "engine.assets.inspect".to_owned(),
        kind: document.document_kind.clone(),
        resource_ref: Some(document.asset_ref.clone()),
        properties,
        capabilities: [document.writer_capability.clone()]
            .into_iter()
            .filter(|cap| !cap.trim().is_empty())
            .collect(),
        tags: vec!["asset-document".to_owned(), document.asset_kind.clone()],
        ..SchemaTypeDescriptorV1::default()
    }
}

pub(super) fn asset_action_input_schema(
    type_id: &str,
    display_name: &str,
    properties: Vec<SchemaPropertyDescriptorV1>,
) -> SchemaTypeDescriptorV1 {
    SchemaTypeDescriptorV1 {
        type_id: type_id.to_owned(),
        display_name: display_name.to_owned(),
        domain: "engine.assets.edit".to_owned(),
        kind: "action_input".to_owned(),
        properties,
        tags: vec!["asset-action-input".to_owned()],
        ..SchemaTypeDescriptorV1::default()
    }
}

pub(super) fn schema_patch_for_document(
    document: &AssetDocument,
    target_ref: &str,
    reason: &str,
    operations: Vec<SchemaPatchOperationV1>,
) -> SchemaPatchDtoV1 {
    SchemaPatchDtoV1 {
        target_type: document
            .schema_type
            .as_ref()
            .map(|schema| schema.type_id.clone())
            .unwrap_or_else(|| document.document_kind.clone()),
        target_ref: target_ref.to_owned(),
        requester: "ui.assets.catalog".to_owned(),
        transaction_id: format!("{}:{}", reason, target_ref),
        operations,
        ..SchemaPatchDtoV1::default()
    }
}

pub(super) fn schema_transaction_for_document(
    document: &AssetDocument,
    target_ref: &str,
    reason: &str,
    operations: Vec<SchemaPatchOperationV1>,
    undo_operations: Vec<SchemaPatchOperationV1>,
) -> SchemaTransactionDtoV1 {
    SchemaTransactionDtoV1 {
        transaction_id: format!("{}:{}", reason, target_ref),
        target_type: document
            .schema_type
            .as_ref()
            .map(|schema| schema.type_id.clone())
            .unwrap_or_else(|| document.document_kind.clone()),
        target_ref: target_ref.to_owned(),
        requester: "ui.assets.catalog".to_owned(),
        reason: reason.to_owned(),
        operations,
        undo_operations,
        ..SchemaTransactionDtoV1::default()
    }
}
