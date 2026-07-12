/// Engine-facing schema/property registry gateway id.
pub const ENGINE_SCHEMA_SERVICE_ID: &str = "engine.schema";

/// Generic provider service id for schema registry implementations.
pub const SCHEMA_SERVICE_ID: &str = "schema.api";

/// Backend capability id declared by schema registry providers.
pub const SCHEMA_BACKEND_CAPABILITY_ID: &str = "schema.registry";

/// Stable runtime contract string for schema registry DTOs.
pub const SCHEMA_RUNTIME_CONTRACT: &str = "newengine.schema.registry.v1";

pub(crate) const DEFAULT_SCHEMA_REQUESTER: &str = "engine.editor";

pub mod schema_method {
    pub const INFO_JSON: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
    pub const INVOKE_JSON: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
    pub const SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;
    pub const DESCRIBE_TYPE_V1: &str = "schema.describe_type_v1";
    pub const DESCRIBE_PROPERTIES_V1: &str = "schema.describe_properties_v1";
    pub const VALIDATE_PATCH_V1: &str = "schema.validate_patch_v1";
    pub const DEFAULT_VALUE_V1: &str = "schema.default_value_v1";
    pub const BINDING_MANIFEST_V1: &str = "schema.binding_manifest_v1";
    pub const TRANSACTION_PLAN_V1: &str = "schema.transaction_plan_v1";
}

pub const SCHEMA_SERVICE_METHODS: &[&str] = &[
    schema_method::INFO_JSON,
    schema_method::INVOKE_JSON,
    schema_method::SHUTDOWN_V1,
    schema_method::DESCRIBE_TYPE_V1,
    schema_method::DESCRIBE_PROPERTIES_V1,
    schema_method::VALIDATE_PATCH_V1,
    schema_method::DEFAULT_VALUE_V1,
    schema_method::BINDING_MANIFEST_V1,
    schema_method::TRANSACTION_PLAN_V1,
];

pub const SCHEMA_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "schema",
        ENGINE_SCHEMA_SERVICE_ID,
        SCHEMA_SERVICE_ID,
        SCHEMA_BACKEND_CAPABILITY_ID,
    );

pub const SCHEMA_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_SCHEMA_SERVICE_ID,
        SCHEMA_RUNTIME_CONTRACT,
        SCHEMA_SERVICE_METHODS,
    );

pub const SCHEMA_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        SCHEMA_RUNTIME_CONTRACT_SPEC,
        Some(SCHEMA_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_SCHEMA_REGISTRY"),
    );
