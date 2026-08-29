use newengine_schema_api::{
    SchemaDiagnosticV1, SchemaPatchDtoV1, SchemaPatchOperationV1, SchemaPropertyDescriptorV1,
    SchemaTypeDescriptorV1, SchemaValueKindV1,
};
use serde_json::{json, Value};

pub(crate) fn normalize_operation(
    descriptor: &SchemaTypeDescriptorV1,
    operation: &SchemaPatchOperationV1,
) -> Result<(SchemaPatchOperationV1, SchemaPatchOperationV1), SchemaDiagnosticV1> {
    let mut op = operation.clone();
    if op.op.trim().is_empty() {
        op.op = "replace".to_owned();
    }
    if !matches!(op.op.as_str(), "add" | "replace" | "remove") {
        return Err(SchemaDiagnosticV1::error(
            "SCHEMA_PATCH_UNSUPPORTED_OP",
            format!("unsupported schema patch op '{}'", op.op),
        ));
    }
    if op.property_id.trim().is_empty() {
        op.property_id = property_id_from_path(&op.path).unwrap_or_default();
    }
    let Some(property) = descriptor
        .properties
        .iter()
        .find(|property| property.property_id == op.property_id.trim())
    else {
        return Err(SchemaDiagnosticV1::error(
            "SCHEMA_PATCH_PROPERTY_NOT_FOUND",
            format!(
                "property '{}' is not registered for type '{}'",
                op.property_id.trim(),
                descriptor.type_id
            ),
        ));
    };
    if !property.editable || property.readonly {
        return Err(SchemaDiagnosticV1::error(
            "SCHEMA_PATCH_PROPERTY_READONLY",
            format!(
                "property '{}' is readonly for type '{}'",
                property.property_id, descriptor.type_id
            ),
        ));
    }
    if op.path.trim().is_empty() {
        op.path = property.json_pointer.clone();
    }
    if op.path != property.json_pointer {
        return Err(SchemaDiagnosticV1::error(
            "SCHEMA_PATCH_PATH_MISMATCH",
            format!(
                "property '{}' must patch path '{}' not '{}'",
                property.property_id, property.json_pointer, op.path
            ),
        ));
    }
    validate_value(property, &op.value)?;
    let undo_value = op
        .old_value
        .clone()
        .unwrap_or_else(|| property.default_value.clone());
    let undo_op = SchemaPatchOperationV1 {
        op: "replace".to_owned(),
        path: property.json_pointer.clone(),
        property_id: property.property_id.clone(),
        value: undo_value,
        old_value: Some(op.value.clone()),
    };
    Ok((op, undo_op))
}

pub(crate) fn deterministic_transaction_id(patch: &SchemaPatchDtoV1) -> String {
    format!(
        "schema.tx.{}.{}.{}",
        sanitize_id(&patch.target_type),
        sanitize_id(&patch.target_ref),
        patch.operations.len()
    )
}

pub(crate) fn default_value_for_kind(kind: SchemaValueKindV1) -> Value {
    match kind {
        SchemaValueKindV1::Null => Value::Null,
        SchemaValueKindV1::Bool => Value::Bool(false),
        SchemaValueKindV1::Int => json!(0),
        SchemaValueKindV1::Float => json!(0.0),
        SchemaValueKindV1::String
        | SchemaValueKindV1::Enum
        | SchemaValueKindV1::AssetRef
        | SchemaValueKindV1::EntityRef => Value::String(String::new()),
        SchemaValueKindV1::StringList | SchemaValueKindV1::Array => Value::Array(Vec::new()),
        SchemaValueKindV1::Vec2 => json!([0.0, 0.0]),
        SchemaValueKindV1::Vec3 => json!([0.0, 0.0, 0.0]),
        SchemaValueKindV1::Vec4 | SchemaValueKindV1::Color => json!([0.0, 0.0, 0.0, 0.0]),
        SchemaValueKindV1::Object => Value::Object(Default::default()),
        SchemaValueKindV1::Json => Value::Null,
    }
}

fn validate_value(
    property: &SchemaPropertyDescriptorV1,
    value: &Value,
) -> Result<(), SchemaDiagnosticV1> {
    if value.is_null() {
        return if property.nullable || matches!(property.value_kind, SchemaValueKindV1::Null) {
            Ok(())
        } else {
            Err(SchemaDiagnosticV1::error(
                "SCHEMA_PATCH_NULL_NOT_ALLOWED",
                format!("property '{}' does not allow null", property.property_id),
            ))
        };
    }
    if !kind_matches(property.value_kind, value) {
        return Err(SchemaDiagnosticV1::error(
            "SCHEMA_PATCH_KIND_MISMATCH",
            format!(
                "property '{}' expects value_kind '{}'",
                property.property_id,
                property.value_kind.as_str()
            ),
        ));
    }
    if let Some(number) = value.as_f64() {
        if property.min.map(|min| number < min).unwrap_or(false) {
            return Err(SchemaDiagnosticV1::error(
                "SCHEMA_PATCH_BELOW_MIN",
                format!(
                    "property '{}' value {} is below min",
                    property.property_id, number
                ),
            ));
        }
        if property.max.map(|max| number > max).unwrap_or(false) {
            return Err(SchemaDiagnosticV1::error(
                "SCHEMA_PATCH_ABOVE_MAX",
                format!(
                    "property '{}' value {} is above max",
                    property.property_id, number
                ),
            ));
        }
    }
    if matches!(property.value_kind, SchemaValueKindV1::Enum) && !property.enum_values.is_empty() {
        let text = value.as_str().unwrap_or_default();
        if !property.enum_values.iter().any(|item| item == text) {
            return Err(SchemaDiagnosticV1::error(
                "SCHEMA_PATCH_ENUM_VALUE_UNKNOWN",
                format!(
                    "property '{}' does not allow enum value '{}'",
                    property.property_id, text
                ),
            ));
        }
    }
    Ok(())
}

fn kind_matches(kind: SchemaValueKindV1, value: &Value) -> bool {
    match kind {
        SchemaValueKindV1::Null => value.is_null(),
        SchemaValueKindV1::Bool => value.is_boolean(),
        SchemaValueKindV1::Int => value.as_i64().is_some() || value.as_u64().is_some(),
        SchemaValueKindV1::Float => value.as_f64().is_some(),
        SchemaValueKindV1::String
        | SchemaValueKindV1::Enum
        | SchemaValueKindV1::AssetRef
        | SchemaValueKindV1::EntityRef => value.as_str().is_some(),
        SchemaValueKindV1::StringList => value
            .as_array()
            .map(|items| items.iter().all(Value::is_string))
            .unwrap_or(false),
        SchemaValueKindV1::Vec2 => array_len(value, 2),
        SchemaValueKindV1::Vec3 => array_len(value, 3),
        SchemaValueKindV1::Vec4 | SchemaValueKindV1::Color => array_len(value, 4),
        SchemaValueKindV1::Object => value.is_object(),
        SchemaValueKindV1::Array => value.is_array(),
        SchemaValueKindV1::Json => true,
    }
}

fn array_len(value: &Value, len: usize) -> bool {
    value
        .as_array()
        .map(|items| items.len() == len && items.iter().all(|item| item.as_f64().is_some()))
        .unwrap_or(false)
}

fn property_id_from_path(path: &str) -> Option<String> {
    path.trim()
        .strip_prefix('/')
        .map(|it| it.replace("~1", "/").replace("~0", "~"))
}

fn sanitize_id(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').to_owned();
    if trimmed.is_empty() {
        "target".to_owned()
    } else {
        trimmed
    }
}
