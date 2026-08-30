#![forbid(unsafe_op_in_unsafe_fn)]

mod config;
mod service;
mod state;
mod validation;

pub use service::{register_schema_gateway_best_effort, schema_gateway_service};
pub use state::{SchemaRegistryState, SchemaRuntimeServiceInfoV1};

pub(crate) const OWNER: &str = "newengine-schema-runtime.engine-core-baseline-provider";
pub(crate) const PROVIDER_ROUTE: &str = "engine.schema.registry";

pub const RUNTIME_UNIT_SPEC: newengine_runtime_unit_api::EngineRuntimeUnitSpec =
    newengine_runtime_unit_api::EngineRuntimeUnitSpec::new(
        "engine.runtime.schema",
        1,
        newengine_runtime_unit_api::EngineRuntimeUnitKind::Provider,
        &[newengine_schema_api::SCHEMA_BACKEND_CAPABILITY_ID],
        &[],
        newengine_runtime_unit_api::STATIC_PROVIDER_TAGS,
    );

fn runtime_unit_factory(
    _: &mut newengine_runtime_unit_api::Engine<()>,
    startup: &newengine_runtime_unit_api::StartupConfig,
) -> newengine_runtime_unit_api::EngineResult<Option<Box<dyn newengine_runtime_unit_api::Module<()>>>>
{
    let probe = SchemaRegistryState::from_startup(startup);
    if !probe.registry_diagnostics.is_empty() {
        return Err(newengine_core::EngineError::other(format!(
            "configured schema registry is invalid: {:?}",
            probe.registry_diagnostics
        )));
    }
    let _ = register_schema_gateway_best_effort();
    Ok(None)
}

pub const RUNTIME_UNIT_REGISTRATION: newengine_runtime_unit_api::RuntimeUnitRegistration =
    newengine_runtime_unit_api::RuntimeUnitRegistration::new(
        RUNTIME_UNIT_SPEC,
        runtime_unit_factory,
    );

#[cfg(test)]
mod tests {
    use super::*;
    use newengine_schema_api::{
        SchemaDefaultValueRequestV1, SchemaDescribeTypeRequestV1, SchemaPatchDtoV1,
        SchemaPatchOperationV1, SchemaPatchValidationRequestV1,
    };
    use serde_json::json;

    fn test_state() -> SchemaRegistryState {
        let loaded = crate::config::load_registry_json(
            r#"{
            "$schema": "northstar.engine.schema_registry.v1",
            "records": [
                {"type_id":"newengine.assets.document.asset_document","domain":"assets","kind":"resource","properties":[]},
                {"type_id":"newengine.component.transform.v1","domain":"components","kind":"component","properties":[]},
                {"type_id":"newengine.settings.world_environment.v1","domain":"settings","kind":"settings","properties":[
                    {"property_id":"time_of_day","value_kind":"float","editable":true,"required":true,"json_pointer":"/time_of_day","min":0.0,"max":1.0}
                ]}
            ]
        }"#,
        );
        SchemaRegistryState::from_loaded(loaded)
    }

    #[test]
    fn configured_registry_loads_foundation_domains() {
        let state = test_state();
        assert!(
            state
                .describe_type(SchemaDescribeTypeRequestV1 {
                    type_id: "newengine.assets.document.asset_document".to_owned(),
                    ..Default::default()
                })
                .accepted
        );
        assert!(
            state
                .describe_type(SchemaDescribeTypeRequestV1 {
                    type_id: "newengine.component.transform.v1".to_owned(),
                    ..Default::default()
                })
                .accepted
        );
        assert!(
            state
                .describe_type(SchemaDescribeTypeRequestV1 {
                    type_id: "newengine.settings.world_environment.v1".to_owned(),
                    ..Default::default()
                })
                .accepted
        );
    }

    #[test]
    fn describe_type_returns_properties_from_config() {
        let state = test_state();
        let response = state.describe_type(SchemaDescribeTypeRequestV1 {
            type_id: "newengine.settings.world_environment.v1".to_owned(),
            include_properties: true,
            ..Default::default()
        });
        assert!(response.accepted);
        assert!(response
            .descriptor
            .unwrap()
            .properties
            .iter()
            .any(|prop| prop.property_id == "time_of_day"));
    }

    #[test]
    fn validate_patch_returns_normalized_patch_and_undo() {
        let state = test_state();
        let response = state.validate_patch(SchemaPatchValidationRequestV1 {
            patch: SchemaPatchDtoV1 {
                target_type: "newengine.settings.world_environment.v1".to_owned(),
                target_ref: "settings/world".to_owned(),
                operations: vec![SchemaPatchOperationV1 {
                    property_id: "time_of_day".to_owned(),
                    value: json!(0.5),
                    old_value: Some(json!(0.25)),
                    ..Default::default()
                }],
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(response.accepted);
        assert!(response.normalized_patch.is_some());
        assert_eq!(response.undo_operations[0].value, json!(0.25));
    }

    #[test]
    fn default_value_is_served_from_schema_property() {
        let state = test_state();
        let response = state.default_value(SchemaDefaultValueRequestV1 {
            type_id: "newengine.settings.world_environment.v1".to_owned(),
            property_id: "time_of_day".to_owned(),
            ..Default::default()
        });
        assert!(response.accepted);
    }
}
