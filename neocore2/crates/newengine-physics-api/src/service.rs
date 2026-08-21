/// Engine-facing physics service gateway ids.
pub const ENGINE_PHYSICS_SERVICE_ID: &str = newengine_service_api::ENGINE_PHYSICS_GATEWAY_ID;
pub const ENGINE_PHYSICS_CONTACTS_SERVICE_ID: &str =
    newengine_service_api::ENGINE_PHYSICS_CONTACTS_GATEWAY_ID;
pub const ENGINE_PHYSICS_CONSTRAINTS_SERVICE_ID: &str =
    newengine_service_api::ENGINE_PHYSICS_CONSTRAINTS_GATEWAY_ID;

/// Default/first-party provider service ids and capabilities.
pub const PHYSICS_SERVICE_ID: &str = "physics.api";
pub const PHYSICS_BACKEND_CAPABILITY_ID: &str = "physics.backend";
pub const PHYSICS_PROVIDER_ABI_VERSION: u16 = 1;
pub const PHYSICS_PROVIDER_ABI_ID: &str = "newengine.physics-provider/v1";
pub const PHYSICS_PROVIDER_ABI_CONTRACT_SPEC: newengine_contract_api::ContractSpec =
    newengine_contract_api::ContractSpec::new(
        "physics.provider.abi",
        newengine_contract_api::ContractKind::Abi,
        newengine_contract_api::ContractVersion::major(PHYSICS_PROVIDER_ABI_VERSION),
        newengine_contract_api::ContractCompatibility::Exact,
        "newengine-physics-api",
        Some(PHYSICS_PROVIDER_ABI_ID),
    );
pub const PHYSICS_CONTACTS_SERVICE_ID: &str = "physics.contacts.api";
pub const PHYSICS_CONTACTS_BACKEND_CAPABILITY_ID: &str = "physics.contacts.backend";
pub const PHYSICS_CONSTRAINTS_SERVICE_ID: &str = "physics.constraints.api";
pub const PHYSICS_CONSTRAINTS_BACKEND_CAPABILITY_ID: &str = "physics.constraints.backend";

pub const PHYSICS_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const PHYSICS_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const PHYSICS_SERVICE_METHOD_SHUTDOWN_V1: &str =
    newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;

pub const PHYSICS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "physics",
        ENGINE_PHYSICS_SERVICE_ID,
        PHYSICS_SERVICE_ID,
        PHYSICS_BACKEND_CAPABILITY_ID,
    );

pub const PHYSICS_CONTACTS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "physics.contacts",
        ENGINE_PHYSICS_CONTACTS_SERVICE_ID,
        PHYSICS_CONTACTS_SERVICE_ID,
        PHYSICS_CONTACTS_BACKEND_CAPABILITY_ID,
    );

pub const PHYSICS_CONSTRAINTS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "physics.constraints",
        ENGINE_PHYSICS_CONSTRAINTS_SERVICE_ID,
        PHYSICS_CONSTRAINTS_SERVICE_ID,
        PHYSICS_CONSTRAINTS_BACKEND_CAPABILITY_ID,
    );

pub const PHYSICS_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_PHYSICS_SERVICE_ID,
        "newengine.physics-api >= 0.1.x",
        newengine_service_api::JSON_CONTROL_SERVICE_METHODS_V1,
    );

pub const PHYSICS_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        PHYSICS_RUNTIME_CONTRACT_SPEC,
        Some(PHYSICS_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_PHYSICS_BACKEND"),
    );
