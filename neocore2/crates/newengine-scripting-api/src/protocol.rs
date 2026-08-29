/// Engine-facing scripting service gateway id.
///
/// Runtime consumers call this facade. The host resolves it to the active
/// scripting provider by descriptor/capability metadata. The engine does not
/// know which scripting language, VM, graph runtime or bytecode format is used
/// behind this gateway.
pub const ENGINE_SCRIPTING_SERVICE_ID: &str = newengine_service_api::ENGINE_SCRIPTING_GATEWAY_ID;

/// Generic provider service id for the opaque scripting contract.
pub const SCRIPTING_SERVICE_ID: &str = "scripting.api";

/// Generic backend capability root. Provider implementation details stay
/// provider-owned and opaque to core/runtime.
pub const SCRIPTING_BACKEND_CAPABILITY_ID: &str = "scripting.backend";
pub const SCRIPTING_BINARY_PROTOCOL_VERSION: u16 = 1;
pub const SCRIPTING_BINARY_PROTOCOL_ID: &str = "newengine.scripting-api/binary-opaque-v1";
pub const SCRIPTING_BINARY_PROTOCOL_CONTRACT_SPEC: newengine_contract_api::ContractSpec =
    newengine_contract_api::ContractSpec::new(
        "scripting.binary.protocol",
        newengine_contract_api::ContractKind::Protocol,
        newengine_contract_api::ContractVersion::major(SCRIPTING_BINARY_PROTOCOL_VERSION),
        newengine_contract_api::ContractCompatibility::Exact,
        "newengine-scripting-api",
        Some(SCRIPTING_BINARY_PROTOCOL_ID),
    );

pub const SCRIPTING_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const SCRIPTING_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const SCRIPTING_SERVICE_METHOD_SHUTDOWN_V1: &str =
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;

/// Primary hot-path methods. Payloads use the binary wire helpers; JSON is
/// reserved for control/debug surfaces only.
pub const SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1: &str = "scripting.load_module_bytes_v1";
pub const SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1: &str = "scripting.invoke_bytes_v1";
pub const SCRIPTING_SERVICE_METHOD_FRAME_BYTES_V1: &str = "scripting.frame_bytes_v1";

/// Debug/control methods.
pub const SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1: &str = "scripting.dump_state_json_v1";
pub const SCRIPTING_SERVICE_METHOD_VALIDATE_MODULE_REF_JSON_V1: &str =
    "scripting.validate_module_ref_json_v1";
pub const SCRIPTING_SERVICE_METHOD_UNLOAD_MODULE_JSON_V1: &str = "scripting.unload_module_json_v1";
pub const SCRIPTING_SERVICE_METHOD_BINDING_MANIFEST_JSON_V1: &str =
    "scripting.binding_manifest_json_v1";
pub const SCRIPTING_SERVICE_METHOD_COMPLETE_JSON_V1: &str = "scripting.complete_json_v1";
pub const SCRIPTING_SERVICE_METHOD_SIGNATURE_HELP_JSON_V1: &str =
    "scripting.signature_help_json_v1";
pub const SCRIPTING_SERVICE_METHOD_SET_TOOLING_CATALOG_JSON_V1: &str =
    "scripting.set_tooling_catalog_json_v1";

pub const SCRIPTING_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "scripting",
        ENGINE_SCRIPTING_SERVICE_ID,
        SCRIPTING_SERVICE_ID,
        SCRIPTING_BACKEND_CAPABILITY_ID,
    );

pub const SCRIPTING_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_SCRIPTING_SERVICE_ID,
        "newengine.scripting-api >= 0.2.x binary-opaque",
        newengine_service_api::JSON_CONTROL_SERVICE_METHODS_V1,
    );

pub const SCRIPTING_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        SCRIPTING_RUNTIME_CONTRACT_SPEC,
        Some(SCRIPTING_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_SCRIPTING_BACKEND"),
    );

pub const SCRIPTING_SERVICE_METHODS: &[&str] = &[
    SCRIPTING_SERVICE_METHOD_INFO,
    SCRIPTING_SERVICE_METHOD_INVOKE,
    SCRIPTING_SERVICE_METHOD_SHUTDOWN_V1,
    SCRIPTING_SERVICE_METHOD_LOAD_MODULE_BYTES_V1,
    SCRIPTING_SERVICE_METHOD_INVOKE_BYTES_V1,
    SCRIPTING_SERVICE_METHOD_FRAME_BYTES_V1,
    SCRIPTING_SERVICE_METHOD_DUMP_STATE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_VALIDATE_MODULE_REF_JSON_V1,
    SCRIPTING_SERVICE_METHOD_UNLOAD_MODULE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_BINDING_MANIFEST_JSON_V1,
    SCRIPTING_SERVICE_METHOD_COMPLETE_JSON_V1,
    SCRIPTING_SERVICE_METHOD_SIGNATURE_HELP_JSON_V1,
    SCRIPTING_SERVICE_METHOD_SET_TOOLING_CATALOG_JSON_V1,
];

#[inline]
pub const fn scripting_service_methods() -> &'static [&'static str] {
    SCRIPTING_SERVICE_METHODS
}
