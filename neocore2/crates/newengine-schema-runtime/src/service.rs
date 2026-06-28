use std::sync::{Arc, OnceLock};

use abi_stable::std_types::{RResult, RString};
use newengine_plugin_api::Blob;
use newengine_schema_api::{
    schema_method, SchemaDefaultValueRequestV1, SchemaDescribePropertiesRequestV1,
    SchemaDescribeTypeRequestV1, SchemaPatchValidationRequestV1, SchemaTransactionDtoV1,
    ENGINE_SCHEMA_SERVICE_ID, SCHEMA_BACKEND_CAPABILITY_ID, SCHEMA_RUNTIME_CONTRACT,
    SCHEMA_SERVICE_ID, SCHEMA_SERVICE_METHODS,
};
use newengine_service_kit::{
    engine_gateway_provider_service_description, ok_json, payload_json,
    register_engine_gateway_provider_service_best_effort, EngineGatewayProviderDecl,
    JsonServiceRouter,
};
use parking_lot::Mutex;
use serde_json::Value;

use crate::state::SchemaRegistryState;

static SCHEMA_REGISTRY: OnceLock<Arc<Mutex<SchemaRegistryState>>> = OnceLock::new();

fn state() -> Arc<Mutex<SchemaRegistryState>> {
    Arc::clone(SCHEMA_REGISTRY.get_or_init(|| Arc::new(Mutex::new(SchemaRegistryState::default()))))
}

fn invoke(state: &mut SchemaRegistryState, payload: Blob) -> RResult<Blob, RString> {
    let value = match payload_json(&payload) {
        Ok(value) => value,
        Err(e) => return RResult::RErr(RString::from(e)),
    };
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or(schema_method::DESCRIBE_TYPE_V1);
    let request = value.get("request").cloned().unwrap_or(Value::Null);
    match method {
        schema_method::INFO_JSON => ok_json(state.info()),
        schema_method::DESCRIBE_TYPE_V1 => {
            match serde_json::from_value::<SchemaDescribeTypeRequestV1>(request) {
                Ok(request) => ok_json(state.describe_type(request)),
                Err(e) => RResult::RErr(RString::from(format!(
                    "schema.api: invalid describe_type request: {e}"
                ))),
            }
        }
        schema_method::DESCRIBE_PROPERTIES_V1 => {
            match serde_json::from_value::<SchemaDescribePropertiesRequestV1>(request) {
                Ok(request) => ok_json(state.describe_properties(request)),
                Err(e) => RResult::RErr(RString::from(format!(
                    "schema.api: invalid describe_properties request: {e}"
                ))),
            }
        }
        schema_method::VALIDATE_PATCH_V1 => {
            match serde_json::from_value::<SchemaPatchValidationRequestV1>(request) {
                Ok(request) => ok_json(state.validate_patch(request)),
                Err(e) => RResult::RErr(RString::from(format!(
                    "schema.api: invalid validate_patch request: {e}"
                ))),
            }
        }
        schema_method::DEFAULT_VALUE_V1 => {
            match serde_json::from_value::<SchemaDefaultValueRequestV1>(request) {
                Ok(request) => ok_json(state.default_value(request)),
                Err(e) => RResult::RErr(RString::from(format!(
                    "schema.api: invalid default_value request: {e}"
                ))),
            }
        }
        schema_method::BINDING_MANIFEST_V1 => ok_json(state.binding_manifest_from_value(request)),
        schema_method::TRANSACTION_PLAN_V1 => {
            match serde_json::from_value::<SchemaTransactionDtoV1>(request) {
                Ok(request) => ok_json(state.transaction_plan(request)),
                Err(e) => RResult::RErr(RString::from(format!(
                    "schema.api: invalid transaction_plan request: {e}"
                ))),
            }
        }
        other => RResult::RErr(RString::from(format!(
            "schema.api: unknown invoke method '{other}'"
        ))),
    }
}

pub fn schema_gateway_service() -> newengine_plugin_api::ServiceV1Dyn<'static> {
    let description = engine_gateway_provider_service_description(
        SCHEMA_SERVICE_ID,
        crate::OWNER,
        SCHEMA_BACKEND_CAPABILITY_ID,
        SCHEMA_SERVICE_METHODS.iter().copied(),
    )
    .gateway("engine.schema registry baseline provider; replaceable through gateway/capability routing")
    .protocol(SCHEMA_RUNTIME_CONTRACT)
    .features([
        "core-owned-baseline-provider",
        "replaceable-gateway-route",
        "config-schema-registry-v1",
        "property-descriptors-v1",
        "patch-validation-v1",
        "undo-operation-planning-v1",
        "default-values-v1",
        "scripting-binding-manifest-v1",
    ])
    .notes("Baseline schema provider is compiled with the core runtime and registers as an ordinary engine.schema route. It is not a hidden singleton and not an externally required plugin; plugin/mod providers can override it through the same descriptor/capability priority model.");

    JsonServiceRouter::with_shared_state(SCHEMA_SERVICE_ID, state())
        .describe_json(&description)
        .get_json(schema_method::INFO_JSON, |state| state.info())
        .post_json(schema_method::DESCRIBE_TYPE_V1, |state, request| {
            state.describe_type(request)
        })
        .post_json(schema_method::DESCRIBE_PROPERTIES_V1, |state, request| {
            state.describe_properties(request)
        })
        .post_json(schema_method::VALIDATE_PATCH_V1, |state, request| {
            state.validate_patch(request)
        })
        .post_json(schema_method::DEFAULT_VALUE_V1, |state, request| {
            state.default_value(request)
        })
        .json_value_result(schema_method::BINDING_MANIFEST_V1, |state, request| {
            Ok(
                serde_json::to_value(state.binding_manifest_from_value(request))
                    .unwrap_or(Value::Null),
            )
        })
        .post_json(schema_method::TRANSACTION_PLAN_V1, |state, request| {
            state.transaction_plan(request)
        })
        .get_json("schema.dump_registry_v1", |state| state.dump_registry())
        .blob(schema_method::INVOKE_JSON, invoke)
        .get_json(schema_method::SHUTDOWN_V1, |state| state.shutdown())
        .into_service_v1()
}

pub fn register_schema_gateway_best_effort() -> bool {
    register_engine_gateway_provider_service_best_effort(EngineGatewayProviderDecl {
        gateway: ENGINE_SCHEMA_SERVICE_ID,
        service_kind: newengine_service_api::EngineServiceKind::Schema,
        provider_service: SCHEMA_SERVICE_ID,
        provider_route: crate::PROVIDER_ROUTE,
        capability: SCHEMA_BACKEND_CAPABILITY_ID,
        priority: 0,
        owner: crate::OWNER,
        service: schema_gateway_service(),
    })
}
