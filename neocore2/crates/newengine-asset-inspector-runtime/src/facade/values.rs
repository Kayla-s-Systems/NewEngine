use super::*;

pub(super) fn parse_field_value(field: &AssetDocumentField, raw: &Value) -> Result<Value, String> {
    let kind = field
        .schema_property
        .as_ref()
        .map(|property| property.value_kind)
        .unwrap_or_else(|| value_kind_from_text(&field.value_kind));
    let text = raw.as_str().map(str::trim);
    match kind {
        SchemaValueKindV1::Null => Ok(Value::Null),
        SchemaValueKindV1::Bool => raw
            .as_bool()
            .map(Value::Bool)
            .or_else(|| text.and_then(parse_bool).map(Value::Bool))
            .ok_or_else(|| format!("field '{}' expects a boolean", field.label)),
        SchemaValueKindV1::Int => raw
            .as_i64()
            .map(|value| json!(value))
            .or_else(|| {
                text.and_then(|value| value.parse::<i64>().ok())
                    .map(|value| json!(value))
            })
            .ok_or_else(|| format!("field '{}' expects an integer", field.label)),
        SchemaValueKindV1::Float => raw
            .as_f64()
            .map(|value| json!(value))
            .or_else(|| {
                text.and_then(|value| value.parse::<f64>().ok())
                    .map(|value| json!(value))
            })
            .filter(|value| value.as_f64().is_some_and(f64::is_finite))
            .ok_or_else(|| format!("field '{}' expects a finite number", field.label)),
        SchemaValueKindV1::String
        | SchemaValueKindV1::Enum
        | SchemaValueKindV1::AssetRef
        | SchemaValueKindV1::EntityRef => {
            let value = raw
                .as_str()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| raw.to_string());
            if kind == SchemaValueKindV1::Enum
                && !field.enum_values.is_empty()
                && !field
                    .enum_values
                    .iter()
                    .any(|candidate| candidate == &value)
            {
                return Err(format!(
                    "field '{}' expects one of: {}",
                    field.label,
                    field.enum_values.join(", ")
                ));
            }
            Ok(Value::String(value))
        }
        SchemaValueKindV1::StringList
        | SchemaValueKindV1::Vec2
        | SchemaValueKindV1::Vec3
        | SchemaValueKindV1::Vec4
        | SchemaValueKindV1::Color
        | SchemaValueKindV1::Object
        | SchemaValueKindV1::Array
        | SchemaValueKindV1::Json => match raw {
            Value::String(value) => serde_json::from_str(value).map_err(|error| {
                format!(
                    "field '{}' expects JSON-compatible input: {error}",
                    field.label
                )
            }),
            other => Ok(other.clone()),
        },
    }
}

fn value_kind_from_text(value: &str) -> SchemaValueKindV1 {
    match value.trim().to_ascii_lowercase().as_str() {
        "null" => SchemaValueKindV1::Null,
        "bool" | "boolean" => SchemaValueKindV1::Bool,
        "int" | "integer" => SchemaValueKindV1::Int,
        "float" | "double" | "number" => SchemaValueKindV1::Float,
        "string_list" => SchemaValueKindV1::StringList,
        "enum" => SchemaValueKindV1::Enum,
        "vec2" => SchemaValueKindV1::Vec2,
        "vec3" => SchemaValueKindV1::Vec3,
        "vec4" => SchemaValueKindV1::Vec4,
        "color" => SchemaValueKindV1::Color,
        "asset_ref" => SchemaValueKindV1::AssetRef,
        "entity_ref" => SchemaValueKindV1::EntityRef,
        "object" => SchemaValueKindV1::Object,
        "array" => SchemaValueKindV1::Array,
        "json" => SchemaValueKindV1::Json,
        _ => SchemaValueKindV1::String,
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}
