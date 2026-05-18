#![forbid(unsafe_op_in_unsafe_fn)]

use core::fmt;

/// Generic JSON-control service method names shared by host and providers.
///
/// Domain API crates may re-export these names, but the literals are owned here
/// so host-side adapters and plugin services do not drift.
pub mod standard_method {
    /// Returns domain-specific backend/provider metadata as JSON.
    pub const INFO_JSON: &str = "info_json";

    /// Invokes a domain-specific JSON request envelope and returns a JSON response envelope.
    pub const INVOKE_JSON: &str = "invoke_json";

    /// Optional explicit shutdown hook called before service unregister/drop.
    pub const SHUTDOWN_V1: &str = "shutdown_v1";
}

pub const SERVICE_METHOD_INFO_JSON: &str = standard_method::INFO_JSON;
pub const SERVICE_METHOD_INVOKE_JSON: &str = standard_method::INVOKE_JSON;
pub const SERVICE_METHOD_SHUTDOWN_V1: &str = standard_method::SHUTDOWN_V1;

/// Required method set for backend services that use the common JSON-control
/// transport: `info_json`, `invoke_json`, `shutdown_v1`.
pub const JSON_CONTROL_SERVICE_METHODS_V1: &[&str] = &[
    SERVICE_METHOD_INFO_JSON,
    SERVICE_METHOD_INVOKE_JSON,
    SERVICE_METHOD_SHUTDOWN_V1,
];

/// Reserved prefix for host-owned facade service gateways.
///
/// This is a namespace convention, not a concrete provider decision. Providers
/// declare a concrete gateway id in capability metadata; the host routes by the
/// descriptor table and never by domain-specific branches.
pub const ENGINE_SERVICE_GATEWAY_PREFIX: &str = "engine.";

#[inline]
pub fn is_engine_service_gateway_id(value: &str) -> bool {
    value.starts_with(ENGINE_SERVICE_GATEWAY_PREFIX)
}

/// Common declaration for a backend service family.
///
/// This intentionally does not describe domain packets. Render, physics, input,
/// UI and future domains still own their DTOs and typed adapters; this spec only
/// tells the host which service id and backend capability must co-exist on the
/// provider plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendServiceSpec {
    /// Human-readable domain label used in diagnostics.
    pub domain: &'static str,
    /// Stable engine-facing gateway id consumers call, e.g. `engine.render`.
    pub engine_gateway_id: &'static str,
    /// First-party/default provider service id, e.g. `render.api`.
    ///
    /// Third-party providers may use a different service id when their backend
    /// capability metadata declares the same `engine_gateway` and points its
    /// `contract` field at the registered provider service.
    pub provider_service_id: &'static str,
    /// Backend capability id declared by provider plugins.
    pub backend_capability_id: &'static str,
}

impl BackendServiceSpec {
    #[inline]
    pub const fn new(
        domain: &'static str,
        engine_gateway_id: &'static str,
        provider_service_id: &'static str,
        backend_capability_id: &'static str,
    ) -> Self {
        Self { domain, engine_gateway_id, provider_service_id, backend_capability_id }
    }
}

/// Engine-side vocabulary for service provider kinds accepted by the host.
///
/// Plugins do not need to import this enum or know the full set. They describe
/// themselves with string metadata such as `service_kind = "render"`; the
/// host validates that string against this vocabulary and ignores unknown kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineServiceKind {
    Assets,
    Render,
    Camera,
    Scene,
    Physics,
    Input,
    Ui,
    Logging,
    Platform,
    Ecs,
}

impl EngineServiceKind {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Assets => "assets",
            Self::Render => "render",
            Self::Camera => "camera",
            Self::Scene => "scene",
            Self::Physics => "physics",
            Self::Input => "input",
            Self::Ui => "ui",
            Self::Logging => "logging",
            Self::Platform => "platform",
            Self::Ecs => "ecs",
        }
    }

    #[inline]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "assets" => Some(Self::Assets),
            "render" => Some(Self::Render),
            "camera" => Some(Self::Camera),
            "scene" => Some(Self::Scene),
            "physics" => Some(Self::Physics),
            "input" => Some(Self::Input),
            "ui" => Some(Self::Ui),
            "logging" | "log" => Some(Self::Logging),
            "platform" => Some(Self::Platform),
            "ecs" => Some(Self::Ecs),
            _ => None,
        }
    }
}

/// Startup contract for a runtime service.
///
/// Domain crates may expose constants of this type; startup validation can then
/// walk specs instead of hard-coding a resolver per backend family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeServiceContractSpec {
    pub service_id: &'static str,
    pub expected_contract: &'static str,
    pub required_methods: &'static [&'static str],
}

impl RuntimeServiceContractSpec {
    #[inline]
    pub const fn new(
        service_id: &'static str,
        expected_contract: &'static str,
        required_methods: &'static [&'static str],
    ) -> Self {
        Self { service_id, expected_contract, required_methods }
    }
}


/// Declarative startup policy for a runtime service gateway or direct host service.
///
/// This is intentionally data-only. The engine startup validator walks a catalog
/// of these specs; it must not branch on individual domains. Missing providers
/// degrade by default and become fatal only when `required_env` is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeServiceRequirementSpec {
    pub contract: RuntimeServiceContractSpec,
    pub required_capability_id: Option<&'static str>,
    pub required_env: Option<&'static str>,
}

impl RuntimeServiceRequirementSpec {
    #[inline]
    pub const fn new(
        contract: RuntimeServiceContractSpec,
        required_capability_id: Option<&'static str>,
        required_env: Option<&'static str>,
    ) -> Self {
        Self { contract, required_capability_id, required_env }
    }
}

/// Stable identifier of a service provided through `newengine-core`'s service registry.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ServiceKey(pub u128);

impl ServiceKey {
    #[inline]
    pub const fn new(v: u128) -> Self {
        Self(v)
    }
}

impl fmt::Debug for ServiceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ServiceKey(0x{:032x})", self.0)
    }
}

/// Stable identifier of an interface (vtable contract) exposed by a service.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct InterfaceId(pub u128);

impl InterfaceId {
    #[inline]
    pub const fn new(v: u128) -> Self {
        Self(v)
    }
}

impl fmt::Debug for InterfaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InterfaceId(0x{:032x})", self.0)
    }
}

/// Deterministic, compile-time friendly 128-bit hash for stable IDs.
///
/// Implementation: two independent FNV-1a 64-bit hashes (different offsets), concatenated into `u128`.
/// This is *not* a crypto hash; it is used only for stable identifiers.
#[inline]
pub const fn hash_u128(s: &str) -> u128 {
    const FNV_PRIME: u64 = 1099511628211;
    const OFFSET_1: u64 = 14695981039346656037;
    const OFFSET_2: u64 = 7809847782465536322;

    let bytes = s.as_bytes();
    let mut h1 = OFFSET_1;
    let mut h2 = OFFSET_2;

    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i] as u64;
        h1 ^= b;
        h1 = h1.wrapping_mul(FNV_PRIME);

        // cheap decorrelation: fold index in second stream
        h2 ^= b.wrapping_add((i as u64).wrapping_mul(0x9e3779b97f4a7c15));
        h2 = h2.wrapping_mul(FNV_PRIME);

        i += 1;
    }

    ((h1 as u128) << 64) | (h2 as u128)
}

/// Trait implemented by typed interface wrappers in `*-api` crates.
pub trait ServiceInterface: Sized {
    type VTable;
    const INTERFACE_ID: InterfaceId;

    /// # Safety
    /// `instance` must be a valid instance pointer for the service, and `vtable` must match `Self::VTable`.
    unsafe fn from_raw(instance: *mut (), vtable: *const Self::VTable) -> Self;
}
