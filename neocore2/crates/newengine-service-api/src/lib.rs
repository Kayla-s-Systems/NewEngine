#![forbid(unsafe_op_in_unsafe_fn)]

use core::fmt;
use std::collections::BTreeMap;

use serde::{Serialize, Serializer};

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
    normalize_engine_gateway_id(value).is_some()
}

#[inline]
pub fn normalize_engine_gateway_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || !trimmed.starts_with(ENGINE_SERVICE_GATEWAY_PREFIX) {
        return None;
    }
    let mut out = Vec::new();
    for segment in trimmed.split('.') {
        let segment = segment.trim();
        if segment.is_empty() {
            return None;
        }
        out.push(segment.to_ascii_lowercase());
    }
    Some(out.join("."))
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

/// Typed provider route metadata serialized into backend capability JSON.
///
/// This is the structured form of the descriptor fragment consumed by the
/// gateway registry. Providers should build this from their domain
/// `BackendServiceSpec` instead of hand-writing JSON strings for
/// `service_kind`, `engine_gateway`, `contract` and `backend_priority`.
#[derive(Debug, Clone, Serialize)]
pub struct BackendRouteDescriptor {
    pub service_kind: EngineServiceKind,
    pub engine_gateway: &'static str,
    pub contract: &'static str,
    pub backend_priority: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<&'static str>,
    #[serde(flatten, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<&'static str, serde_json::Value>,
}

impl BackendRouteDescriptor {
    #[inline]
    pub fn new(spec: BackendServiceSpec) -> Self {
        let service_kind = EngineServiceKind::parse(spec.domain)
            .or_else(|| EngineServiceKind::parse_engine_gateway_id(spec.engine_gateway_id))
            .expect("BackendServiceSpec domain must map to EngineServiceKind");
        debug_assert!(
            service_kind.matches_engine_gateway_id(spec.engine_gateway_id),
            "BackendServiceSpec domain and engine gateway must describe the same route",
        );
        Self {
            service_kind,
            engine_gateway: spec.engine_gateway_id,
            contract: spec.provider_service_id,
            backend_priority: 0,
            backend: None,
            mode: None,
            features: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    #[inline]
    pub fn contract(mut self, contract: &'static str) -> Self {
        self.contract = contract;
        self
    }

    #[inline]
    pub fn priority(mut self, backend_priority: i32) -> Self {
        self.backend_priority = backend_priority;
        self
    }

    #[inline]
    pub fn backend(mut self, backend: &'static str) -> Self {
        self.backend = Some(backend);
        self
    }

    #[inline]
    pub fn mode(mut self, mode: &'static str) -> Self {
        self.mode = Some(mode);
        self
    }

    #[inline]
    pub fn feature(mut self, feature: &'static str) -> Self {
        self.features.push(feature);
        self
    }

    #[inline]
    pub fn features(mut self, features: impl IntoIterator<Item = &'static str>) -> Self {
        self.features.extend(features);
        self
    }

    #[inline]
    pub fn metadata_json(mut self, key: &'static str, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    #[inline]
    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).expect("BackendRouteDescriptor must serialize to JSON")
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
    Audio,
    Render,
    RenderEffects,
    RenderMaterials,
    Model,
    ModelSkeletons,
    ModelMaterials,
    ModelCollisions,
    Camera,
    CameraModes,
    CameraAnimations,
    Scene,
    Physics,
    PhysicsContacts,
    PhysicsConstraints,
    Input,
    InputBindings,
    InputActions,
    InputContexts,
    Ui,
    Logging,
    Loading,
    Platform,
    Ecs,
    Entity,
    PluginHost,
    Abi,
    GatewayRegistry,
    Security,
    SchedulerCore,
    CapabilityValidator,
}

impl Serialize for EngineServiceKind {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl EngineServiceKind {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Assets => "assets",
            Self::Audio => "audio",
            Self::Render => "render",
            Self::RenderEffects => "render.effects",
            Self::RenderMaterials => "render.materials",
            Self::Model => "model",
            Self::ModelSkeletons => "model.skeletons",
            Self::ModelMaterials => "model.materials",
            Self::ModelCollisions => "model.collisions",
            Self::Camera => "camera",
            Self::CameraModes => "camera.modes",
            Self::CameraAnimations => "camera.animations",
            Self::Scene => "scene",
            Self::Physics => "physics",
            Self::PhysicsContacts => "physics.contacts",
            Self::PhysicsConstraints => "physics.constraints",
            Self::Input => "input",
            Self::InputBindings => "input.bindings",
            Self::InputActions => "input.actions",
            Self::InputContexts => "input.contexts",
            Self::Ui => "ui",
            Self::Logging => "logging",
            Self::Loading => "loading",
            Self::Platform => "platform",
            Self::Ecs => "ecs",
            Self::Entity => "entity",
            Self::PluginHost => "plugin_host",
            Self::Abi => "abi",
            Self::GatewayRegistry => "gateway_registry",
            Self::Security => "security",
            Self::SchedulerCore => "scheduler.core",
            Self::CapabilityValidator => "capability_validator",
        }
    }

    #[inline]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "assets" => Some(Self::Assets),
            "audio" => Some(Self::Audio),
            "render" => Some(Self::Render),
            "render.effects" | "render_effects" => Some(Self::RenderEffects),
            "render.materials" | "render_materials" => Some(Self::RenderMaterials),
            "model" => Some(Self::Model),
            "model.skeletons" | "model_skeletons" => Some(Self::ModelSkeletons),
            "model.materials" | "model_materials" => Some(Self::ModelMaterials),
            "model.collisions" | "model_collisions" => Some(Self::ModelCollisions),
            "camera" => Some(Self::Camera),
            "camera.modes" | "camera_modes" => Some(Self::CameraModes),
            "camera.animations" | "camera_animations" => Some(Self::CameraAnimations),
            "scene" => Some(Self::Scene),
            "physics" => Some(Self::Physics),
            "physics.contacts" | "physics_contacts" => Some(Self::PhysicsContacts),
            "physics.constraints" | "physics_constraints" => Some(Self::PhysicsConstraints),
            "input" => Some(Self::Input),
            "input.bindings" | "input_bindings" => Some(Self::InputBindings),
            "input.actions" | "input_actions" => Some(Self::InputActions),
            "input.contexts" | "input_contexts" => Some(Self::InputContexts),
            "ui" => Some(Self::Ui),
            "logging" | "log" => Some(Self::Logging),
            "loading" => Some(Self::Loading),
            "platform" => Some(Self::Platform),
            "ecs" => Some(Self::Ecs),
            "entity" => Some(Self::Entity),
            "plugin_host" | "plugin-host" | "plugin.host" => Some(Self::PluginHost),
            "abi" => Some(Self::Abi),
            "gateway_registry" | "gateway-registry" | "gateway.registry" => Some(Self::GatewayRegistry),
            "security" => Some(Self::Security),
            "scheduler.core" | "scheduler_core" | "scheduler-core" => Some(Self::SchedulerCore),
            "capability_validator" | "capability-validator" | "capability.validator" => Some(Self::CapabilityValidator),
            _ => None,
        }
    }


    /// Returns the direct parent domain for third-level extension domains.
    ///
    /// Example: `input.bindings -> input`, `render.effects -> render`.
    #[inline]
    pub const fn parent(self) -> Option<Self> {
        match self {
            Self::RenderEffects | Self::RenderMaterials => Some(Self::Render),
            Self::ModelSkeletons | Self::ModelMaterials | Self::ModelCollisions => Some(Self::Model),
            Self::CameraModes | Self::CameraAnimations => Some(Self::Camera),
            Self::PhysicsContacts | Self::PhysicsConstraints => Some(Self::Physics),
            Self::InputBindings | Self::InputActions | Self::InputContexts => Some(Self::Input),
            _ => None,
        }
    }

    #[inline]
    pub const fn root(self) -> Self {
        match self.parent() {
            Some(parent) => parent,
            None => self,
        }
    }

    #[inline]
    pub const fn domain_depth(self) -> u8 {
        match self.parent() {
            Some(_) => 3,
            None => 2,
        }
    }

    #[inline]
    pub const fn engine_gateway_id(self) -> &'static str {
        match self {
            Self::Assets => "engine.assets",
            Self::Audio => "engine.audio",
            Self::Render => "engine.render",
            Self::RenderEffects => "engine.render.effects",
            Self::RenderMaterials => "engine.render.materials",
            Self::Model => "engine.model",
            Self::ModelSkeletons => "engine.model.skeletons",
            Self::ModelMaterials => "engine.model.materials",
            Self::ModelCollisions => "engine.model.collisions",
            Self::Camera => "engine.camera",
            Self::CameraModes => "engine.camera.modes",
            Self::CameraAnimations => "engine.camera.animations",
            Self::Scene => "engine.scene",
            Self::Physics => "engine.physics",
            Self::PhysicsContacts => "engine.physics.contacts",
            Self::PhysicsConstraints => "engine.physics.constraints",
            Self::Input => "engine.input",
            Self::InputBindings => "engine.input.bindings",
            Self::InputActions => "engine.input.actions",
            Self::InputContexts => "engine.input.contexts",
            Self::Ui => "engine.ui",
            Self::Logging => "engine.log",
            Self::Loading => "engine.loading",
            Self::Platform => "engine.platform",
            Self::Ecs => "engine.ecs",
            Self::Entity => "engine.entity",
            Self::PluginHost => "engine.plugin_host",
            Self::Abi => "engine.abi",
            Self::GatewayRegistry => "engine.gateway_registry",
            Self::Security => "engine.security",
            Self::SchedulerCore => "engine.scheduler.core",
            Self::CapabilityValidator => "engine.capability_validator",
        }
    }

    #[inline]
    pub fn matches_engine_gateway_id(self, gateway_id: &str) -> bool {
        self.engine_gateway_id() == normalize_engine_gateway_id(gateway_id).as_deref().unwrap_or("")
    }

    #[inline]
    pub fn parse_engine_gateway_id(gateway_id: &str) -> Option<Self> {
        let normalized = normalize_engine_gateway_id(gateway_id)?;
        let domain = normalized.strip_prefix(ENGINE_SERVICE_GATEWAY_PREFIX)?;
        match domain {
            "log" => Some(Self::Logging),
            other => Self::parse(other),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_domains_parse_with_canonical_gateways() {
        let cases = [
            ("input.bindings", EngineServiceKind::InputBindings, "engine.input.bindings", Some(EngineServiceKind::Input)),
            ("input.actions", EngineServiceKind::InputActions, "engine.input.actions", Some(EngineServiceKind::Input)),
            ("input.contexts", EngineServiceKind::InputContexts, "engine.input.contexts", Some(EngineServiceKind::Input)),
            ("render.effects", EngineServiceKind::RenderEffects, "engine.render.effects", Some(EngineServiceKind::Render)),
            ("render.materials", EngineServiceKind::RenderMaterials, "engine.render.materials", Some(EngineServiceKind::Render)),
            ("model.skeletons", EngineServiceKind::ModelSkeletons, "engine.model.skeletons", Some(EngineServiceKind::Model)),
            ("model.materials", EngineServiceKind::ModelMaterials, "engine.model.materials", Some(EngineServiceKind::Model)),
            ("model.collisions", EngineServiceKind::ModelCollisions, "engine.model.collisions", Some(EngineServiceKind::Model)),
            ("physics.contacts", EngineServiceKind::PhysicsContacts, "engine.physics.contacts", Some(EngineServiceKind::Physics)),
            ("physics.constraints", EngineServiceKind::PhysicsConstraints, "engine.physics.constraints", Some(EngineServiceKind::Physics)),
            ("camera.modes", EngineServiceKind::CameraModes, "engine.camera.modes", Some(EngineServiceKind::Camera)),
            ("camera.animations", EngineServiceKind::CameraAnimations, "engine.camera.animations", Some(EngineServiceKind::Camera)),
        ];

        for (text, kind, gateway, parent) in cases {
            assert_eq!(EngineServiceKind::parse(text), Some(kind));
            assert_eq!(EngineServiceKind::parse_engine_gateway_id(gateway), Some(kind));
            assert_eq!(kind.engine_gateway_id(), gateway);
            assert_eq!(kind.parent(), parent);
            assert!(kind.matches_engine_gateway_id(gateway));
        }
    }

    #[test]
    fn parent_domain_does_not_match_child_gateway() {
        assert!(!EngineServiceKind::Input.matches_engine_gateway_id("engine.input.bindings"));
        assert!(!EngineServiceKind::Render.matches_engine_gateway_id("engine.render.effects"));
        assert!(!EngineServiceKind::Physics.matches_engine_gateway_id("engine.physics.contacts"));
        assert!(!EngineServiceKind::Model.matches_engine_gateway_id("engine.model.skeletons"));
        assert!(!EngineServiceKind::Camera.matches_engine_gateway_id("engine.camera.modes"));
    }

    #[test]
    fn backend_route_descriptor_serializes_registry_fields() {
        let json = BackendRouteDescriptor::new(BackendServiceSpec::new(
            "render",
            "engine.render",
            "render.api",
            "render.backend",
        ))
        .backend("native")
        .mode("graph-draw-list")
        .priority(100)
        .feature("draw-list")
        .metadata_json("shadows", serde_json::json!({ "pcss": true }))
        .to_json_string();

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["service_kind"], "render");
        assert_eq!(value["engine_gateway"], "engine.render");
        assert_eq!(value["contract"], "render.api");
        assert_eq!(value["backend_priority"], 100);
        assert_eq!(value["backend"], "native");
        assert_eq!(value["mode"], "graph-draw-list");
        assert_eq!(value["features"][0], "draw-list");
        assert_eq!(value["shadows"]["pcss"], true);
    }
}
