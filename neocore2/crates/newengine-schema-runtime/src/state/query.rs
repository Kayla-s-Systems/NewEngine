use newengine_schema_api::{
    SchemaDefaultValueRequestV1, SchemaDefaultValueResponseV1, SchemaDescribePropertiesRequestV1,
    SchemaDescribePropertiesResponseV1, SchemaDescribeTypeRequestV1, SchemaDescribeTypeResponseV1,
    SchemaDiagnosticV1, SchemaPropertyDescriptorV1,
};

use super::SchemaRegistryState;

impl SchemaRegistryState {
    pub fn describe_type(
        &self,
        request: SchemaDescribeTypeRequestV1,
    ) -> SchemaDescribeTypeResponseV1 {
        let mut response = SchemaDescribeTypeResponseV1::default();
        let type_id = request.type_id.trim();
        if type_id.is_empty() {
            response.diagnostics.push(SchemaDiagnosticV1::error(
                "SCHEMA_TYPE_ID_REQUIRED",
                "schema.describe_type_v1 requires type_id",
            ));
            return response;
        }
        let Some(descriptor) = self.records.get(type_id) else {
            response.diagnostics.push(SchemaDiagnosticV1::error(
                "SCHEMA_TYPE_NOT_FOUND",
                format!("schema type '{type_id}' is not registered"),
            ));
            return response;
        };
        let mut descriptor = descriptor.clone();
        descriptor.resource_ref = request.resource_ref;
        if !request.include_properties {
            descriptor.properties.clear();
        }
        response.accepted = true;
        response.descriptor = Some(descriptor);
        response
    }

    pub fn describe_properties(
        &self,
        request: SchemaDescribePropertiesRequestV1,
    ) -> SchemaDescribePropertiesResponseV1 {
        let mut response = SchemaDescribePropertiesResponseV1::default();
        let type_id = request.type_id.trim();
        response.type_id = type_id.to_owned();
        if type_id.is_empty() {
            response.diagnostics.push(SchemaDiagnosticV1::error(
                "SCHEMA_TYPE_ID_REQUIRED",
                "schema.describe_properties_v1 requires type_id",
            ));
            return response;
        }
        let Some(descriptor) = self.records.get(type_id) else {
            response.diagnostics.push(SchemaDiagnosticV1::error(
                "SCHEMA_TYPE_NOT_FOUND",
                format!("schema type '{type_id}' is not registered"),
            ));
            return response;
        };
        response.accepted = true;
        response.properties = descriptor.properties.clone();
        response
    }

    pub fn default_value(
        &self,
        request: SchemaDefaultValueRequestV1,
    ) -> SchemaDefaultValueResponseV1 {
        let mut response = SchemaDefaultValueResponseV1::default();
        let type_id = request.type_id.trim();
        let property_id = request.property_id.trim();
        response.type_id = type_id.to_owned();
        response.property_id = property_id.to_owned();
        match self.property(type_id, property_id) {
            Ok(property) => {
                response.accepted = true;
                response.value = property.default_value.clone();
            }
            Err(diagnostic) => response.diagnostics.push(diagnostic),
        }
        response
    }

    fn property(
        &self,
        type_id: &str,
        property_id: &str,
    ) -> Result<&SchemaPropertyDescriptorV1, SchemaDiagnosticV1> {
        let Some(descriptor) = self.records.get(type_id.trim()) else {
            return Err(SchemaDiagnosticV1::error(
                "SCHEMA_TYPE_NOT_FOUND",
                format!("schema type '{}' is not registered", type_id.trim()),
            ));
        };
        descriptor
            .properties
            .iter()
            .find(|property| property.property_id == property_id.trim())
            .ok_or_else(|| {
                SchemaDiagnosticV1::error(
                    "SCHEMA_PROPERTY_NOT_FOUND",
                    format!(
                        "property '{}' is not registered for type '{}'",
                        property_id.trim(),
                        type_id.trim()
                    ),
                )
            })
    }
}
