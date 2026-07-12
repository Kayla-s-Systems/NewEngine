use newengine_schema_api::{
    SchemaDiagnosticV1, SchemaPatchValidationRequestV1, SchemaPatchValidationResponseV1,
    SchemaTransactionDtoV1, SchemaTransactionResultV1,
};

use crate::validation::{deterministic_transaction_id, normalize_operation};

use super::SchemaRegistryState;

impl SchemaRegistryState {
    pub fn validate_patch(
        &self,
        request: SchemaPatchValidationRequestV1,
    ) -> SchemaPatchValidationResponseV1 {
        let mut response = SchemaPatchValidationResponseV1::default();
        let mut patch = request.patch;
        let target_type = patch.target_type.trim().to_owned();
        let Some(descriptor) = self.records.get(&target_type) else {
            response.diagnostics.push(SchemaDiagnosticV1::error(
                "SCHEMA_PATCH_TARGET_TYPE_NOT_FOUND",
                format!("patch target_type '{target_type}' is not registered"),
            ));
            return response;
        };
        if patch.transaction_id.trim().is_empty() {
            patch.transaction_id = deterministic_transaction_id(&patch);
        }
        if patch.requester.trim().is_empty() {
            patch.requester = "engine.schema".to_owned();
        }
        if patch.operations.is_empty() {
            response.diagnostics.push(SchemaDiagnosticV1::error(
                "SCHEMA_PATCH_EMPTY",
                "schema patch must contain at least one operation",
            ));
            return response;
        }

        let mut normalized = Vec::with_capacity(patch.operations.len());
        let mut undo = Vec::with_capacity(patch.operations.len());
        let mut diagnostics = Vec::new();
        for operation in &patch.operations {
            match normalize_operation(descriptor, operation) {
                Ok((operation, undo_operation)) => {
                    normalized.push(operation);
                    undo.push(undo_operation);
                }
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
        if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == "error")
        {
            response.diagnostics = diagnostics;
            return response;
        }

        patch.operations = normalized;
        response.accepted = true;
        response.undo_operations = undo;
        response.normalized_patch = Some(patch);
        response.diagnostics = diagnostics;
        response
    }

    pub fn transaction_plan(
        &self,
        transaction: SchemaTransactionDtoV1,
    ) -> SchemaTransactionResultV1 {
        let mut response = SchemaTransactionResultV1 {
            transaction_id: transaction.transaction_id.clone(),
            ..Default::default()
        };
        if !self.records.contains_key(transaction.target_type.trim()) {
            response.diagnostics.push(SchemaDiagnosticV1::error(
                "SCHEMA_TRANSACTION_TARGET_TYPE_NOT_FOUND",
                format!(
                    "transaction target_type '{}' is not registered",
                    transaction.target_type
                ),
            ));
            return response;
        }
        if transaction.operations.is_empty() {
            response.diagnostics.push(SchemaDiagnosticV1::error(
                "SCHEMA_TRANSACTION_EMPTY",
                "schema transaction must contain at least one operation",
            ));
            return response;
        }
        response.accepted = true;
        response.committed = false;
        response.revision = transaction.base_revision;
        response.diagnostics.push(SchemaDiagnosticV1::info(
            "SCHEMA_TRANSACTION_PLAN_READY",
            "transaction DTO is valid for editor undo/redo planning; applying the patch remains owned by the target domain",
        ));
        response
    }
}
