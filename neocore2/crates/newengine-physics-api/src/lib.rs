#![forbid(unsafe_op_in_unsafe_fn)]

//! Stable service protocol for replaceable NewEngine physics backends.
//!
//! `newengine-physics-api` is intentionally DTO-oriented: packets contain only
//! stable values and never expose ECS `World`, component storage or native
//! backend handles across the service boundary.

use serde::{Deserialize, Serialize};

/// Engine-facing physics service gateway id. Consumers call this facade; the host
/// resolves it to the active physics provider service by descriptor metadata.
pub const ENGINE_PHYSICS_SERVICE_ID: &str = "engine.physics";

/// Default/first-party provider service id for physics backends.
pub const PHYSICS_SERVICE_ID: &str = "physics.api";
pub const PHYSICS_BACKEND_CAPABILITY_ID: &str = "physics.backend";
pub const PHYSICS_SERVICE_METHOD_INFO: &str = newengine_service_api::SERVICE_METHOD_INFO_JSON;
pub const PHYSICS_SERVICE_METHOD_INVOKE: &str = newengine_service_api::SERVICE_METHOD_INVOKE_JSON;
pub const PHYSICS_SERVICE_METHOD_SHUTDOWN_V1: &str = newengine_service_api::SERVICE_METHOD_SHUTDOWN_V1;

/// Generic backend-family declaration for physics providers.
pub const PHYSICS_BACKEND_SERVICE_SPEC: newengine_service_api::BackendServiceSpec =
    newengine_service_api::BackendServiceSpec::new(
        "physics",
        ENGINE_PHYSICS_SERVICE_ID,
        PHYSICS_SERVICE_ID,
        PHYSICS_BACKEND_CAPABILITY_ID,
    );

/// Startup validation contract for the engine-facing physics gateway.
pub const PHYSICS_RUNTIME_CONTRACT_SPEC: newengine_service_api::RuntimeServiceContractSpec =
    newengine_service_api::RuntimeServiceContractSpec::new(
        ENGINE_PHYSICS_SERVICE_ID,
        "newengine.physics-api >= 0.1.x",
        newengine_service_api::JSON_CONTROL_SERVICE_METHODS_V1,
    );

/// Declarative startup requirement for physics. Missing physics degrades unless
/// the explicit env switch is enabled by a strict test/runtime profile.
pub const PHYSICS_RUNTIME_REQUIREMENT_SPEC: newengine_service_api::RuntimeServiceRequirementSpec =
    newengine_service_api::RuntimeServiceRequirementSpec::new(
        PHYSICS_RUNTIME_CONTRACT_SPEC,
        Some(PHYSICS_BACKEND_CAPABILITY_ID),
        Some("NEWENGINE_REQUIRE_PHYSICS_BACKEND"),
    );

pub type PhysicsEntityKey = u64;
pub type PhysicsVec3 = [f32; 3];
pub type PhysicsQuat = [f32; 4];

#[inline]
pub fn encode_json<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|e| e.to_string())
}

#[inline]
pub fn decode_json<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PhysicsApiVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl PhysicsApiVersion {
    #[inline]
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self { major, minor, patch }
    }
}

impl Default for PhysicsApiVersion {
    #[inline]
    fn default() -> Self { Self::new(1, 0, 0) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicsBackendClass {
    Null,
    Deterministic,
    Native,
}

impl Default for PhysicsBackendClass {
    #[inline]
    fn default() -> Self { Self::Deterministic }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PhysicsFeature {
    StaticColliders,
    DynamicBodies,
    KinematicBodies,
    TriggerBodies,
    Contacts,
    Queries,
    DeterministicReplay,
    NativeBackend,
    HeightfieldColliders,
    MeshColliders,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsLimits {
    pub max_bodies: u32,
    pub max_queries_per_frame: u32,
    pub max_substeps: u32,
}

impl Default for PhysicsLimits {
    #[inline]
    fn default() -> Self {
        Self { max_bodies: 100_000, max_queries_per_frame: 4096, max_substeps: 16 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsBackendCapabilities {
    pub backend_class: PhysicsBackendClass,
    #[serde(default)]
    pub features: Vec<PhysicsFeature>,
    #[serde(default)]
    pub limits: PhysicsLimits,
}

impl PhysicsBackendCapabilities {
    #[inline]
    pub fn deterministic_default() -> Self {
        Self {
            backend_class: PhysicsBackendClass::Deterministic,
            features: vec![
                PhysicsFeature::StaticColliders,
                PhysicsFeature::DynamicBodies,
                PhysicsFeature::Contacts,
                PhysicsFeature::DeterministicReplay,
            ],
            limits: PhysicsLimits::default(),
        }
    }

    #[inline]
    pub fn null_default() -> Self {
        Self {
            backend_class: PhysicsBackendClass::Null,
            features: Vec::new(),
            limits: PhysicsLimits::default(),
        }
    }

    #[inline]
    pub fn native_backend_default() -> Self {
        Self {
            backend_class: PhysicsBackendClass::Native,
            features: vec![
                PhysicsFeature::StaticColliders,
                PhysicsFeature::DynamicBodies,
                PhysicsFeature::KinematicBodies,
                PhysicsFeature::TriggerBodies,
                PhysicsFeature::Queries,
                PhysicsFeature::NativeBackend,
                PhysicsFeature::HeightfieldColliders,
                PhysicsFeature::MeshColliders,
            ],
            limits: PhysicsLimits::default(),
        }
    }

    #[inline]
    pub fn supports(&self, feature: PhysicsFeature) -> bool { self.features.contains(&feature) }
}

impl Default for PhysicsBackendCapabilities {
    #[inline]
    fn default() -> Self { Self::deterministic_default() }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsBackendInfo {
    pub backend_id: String,
    pub backend_name: String,
    pub backend_version: String,
    pub debug_text: String,
    #[serde(default)]
    pub capabilities: PhysicsBackendCapabilities,
    #[serde(default)]
    pub protocol_version: PhysicsApiVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsProtocolNotice {
    pub code: String,
    pub message: String,
}

impl PhysicsProtocolNotice {
    #[inline]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsCapabilityNegotiationRequest {
    pub preferred_version: PhysicsApiVersion,
    #[serde(default)]
    pub required_features: Vec<PhysicsFeature>,
    #[serde(default)]
    pub optional_features: Vec<PhysicsFeature>,
}

impl Default for PhysicsCapabilityNegotiationRequest {
    #[inline]
    fn default() -> Self {
        Self {
            preferred_version: PhysicsApiVersion::default(),
            required_features: Vec::new(),
            optional_features: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsCapabilityNegotiationResponse {
    pub accepted_version: PhysicsApiVersion,
    pub backend_version: PhysicsApiVersion,
    pub ok: bool,
    pub enabled_features: Vec<PhysicsFeature>,
    pub missing_required_features: Vec<PhysicsFeature>,
    #[serde(default)]
    pub notices: Vec<PhysicsProtocolNotice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsProblemDetails {
    pub code: String,
    pub title: String,
    pub detail: String,
    pub backend: Option<String>,
    pub phase: Option<String>,
    #[serde(default)]
    pub recoverable: bool,
}

impl PhysicsProblemDetails {
    #[inline]
    pub fn new(code: impl Into<String>, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            title: title.into(),
            detail: detail.into(),
            backend: None,
            phase: None,
            recoverable: true,
        }
    }

    #[inline]
    pub fn with_backend(mut self, backend: impl Into<String>) -> Self {
        self.backend = Some(backend.into());
        self
    }

    #[inline]
    pub fn with_phase(mut self, phase: impl Into<String>) -> Self {
        self.phase = Some(phase.into());
        self
    }

    #[inline]
    pub fn fatal(mut self) -> Self {
        self.recoverable = false;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicsBodyKindDto {
    Static,
    Dynamic,
    Kinematic,
}

impl Default for PhysicsBodyKindDto {
    #[inline]
    fn default() -> Self { Self::Static }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CollisionShapeDto {
    Box { half_extents: PhysicsVec3 },
    Sphere { radius: f32 },
    Capsule { radius: f32, half_height: f32 },
}

impl Default for CollisionShapeDto {
    #[inline]
    fn default() -> Self { Self::Box { half_extents: [0.5, 0.5, 0.5] } }
}

/// Static terrain heightfield packet.
///
/// Samples are row-major: `heights[z * sample_count_x + x]`. The local point
/// for a sample is `[x * spacing[0], height, z * spacing[1]] + local_origin`.
/// Backends that require square heightfields may reject non-square packets or
/// internally map them through `MeshColliderDto`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeightfieldColliderDto {
    pub sample_count_x: u32,
    pub sample_count_z: u32,
    pub spacing: [f32; 2],
    pub local_origin: PhysicsVec3,
    #[serde(default)]
    pub heights: Vec<f32>,
    pub min_height: f32,
    pub max_height: f32,
}

impl HeightfieldColliderDto {
    #[inline]
    pub fn sample_count(&self) -> Option<u32> {
        (self.sample_count_x == self.sample_count_z).then_some(self.sample_count_x)
    }

    #[inline]
    pub fn expected_height_len(&self) -> usize {
        self.sample_count_x as usize * self.sample_count_z as usize
    }

    #[inline]
    pub fn is_square_for_native_heightfield(&self) -> bool {
        self.sample_count().is_some() && self.heights.len() == self.expected_height_len()
    }
}

/// Static triangle mesh collider packet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshColliderDto {
    #[serde(default)]
    pub vertices: Vec<PhysicsVec3>,
    #[serde(default)]
    pub triangles: Vec<[u32; 3]>,
    #[serde(default)]
    pub material_indices: Vec<u32>,
}

impl MeshColliderDto {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() || self.triangles.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PhysicsColliderDto {
    Heightfield(HeightfieldColliderDto),
    Mesh(MeshColliderDto),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicsFrameColliderSnapshot {
    pub entity: PhysicsEntityKey,
    pub collider: PhysicsColliderDto,
    pub flags: PhysicsBodyFlagsDto,
    pub material: PhysicsMaterialDto,
    pub position: PhysicsVec3,
    pub rotation: PhysicsQuat,
    pub bounds_min: PhysicsVec3,
    pub bounds_max: PhysicsVec3,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsMaterialDto {
    pub friction: f32,
    pub restitution: f32,
    pub density: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsBodyFlagsDto {
    pub is_trigger: bool,
    pub participates_in_queries: bool,
    pub casts_contacts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsFrameBodySnapshot {
    pub entity: PhysicsEntityKey,
    pub kind: PhysicsBodyKindDto,
    pub shape: CollisionShapeDto,
    pub flags: PhysicsBodyFlagsDto,
    pub material: PhysicsMaterialDto,
    pub position: PhysicsVec3,
    pub rotation: PhysicsQuat,
    pub linear_velocity: PhysicsVec3,
    pub bounds_min: PhysicsVec3,
    pub bounds_max: PhysicsVec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PhysicsCommandKindDto {
    SetBodyPose { entity: PhysicsEntityKey, position: PhysicsVec3, rotation: PhysicsQuat },
    SetLinearVelocity { entity: PhysicsEntityKey, velocity: PhysicsVec3 },
    DestroyBody { entity: PhysicsEntityKey },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsCommandDto {
    pub seq: u64,
    pub kind: PhysicsCommandKindDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PhysicsQueryKindDto {
    Ray { origin: PhysicsVec3, dir: PhysicsVec3, max_t: f32 },
    Sphere { center: PhysicsVec3, radius: f32 },
    Aabb { min: PhysicsVec3, max: PhysicsVec3 },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsQueryDto {
    pub seq: u64,
    pub kind: PhysicsQueryKindDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsFrameInput {
    pub frame_index: u64,
    pub fixed_tick: u64,
    pub dt: f32,
    pub gravity: f32,
    pub contact_skin: f32,
    #[serde(default)]
    pub bodies: Vec<PhysicsFrameBodySnapshot>,
    #[serde(default)]
    pub colliders: Vec<PhysicsFrameColliderSnapshot>,
    #[serde(default)]
    pub commands: Vec<PhysicsCommandDto>,
    #[serde(default)]
    pub queries: Vec<PhysicsQueryDto>,
}

impl PhysicsFrameInput {
    #[inline]
    pub fn empty(frame_index: u64, fixed_tick: u64, dt: f32) -> Self {
        Self {
            frame_index,
            fixed_tick,
            dt,
            gravity: 9.81,
            contact_skin: 0.035,
            bodies: Vec::new(),
            colliders: Vec::new(),
            commands: Vec::new(),
            queries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsBodyPoseUpdate {
    pub entity: PhysicsEntityKey,
    pub position: PhysicsVec3,
    pub rotation: PhysicsQuat,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsBodyVelocityUpdate {
    pub entity: PhysicsEntityKey,
    pub linear_velocity: PhysicsVec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsContactEventDto {
    pub a: PhysicsEntityKey,
    pub b: PhysicsEntityKey,
    pub point: PhysicsVec3,
    pub normal: PhysicsVec3,
    pub impulse: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PhysicsEventDto {
    ContactBegin(PhysicsContactEventDto),
    ContactPersist(PhysicsContactEventDto),
    ContactEnd { a: PhysicsEntityKey, b: PhysicsEntityKey },
    BodyCreated { entity: PhysicsEntityKey },
    BodyDestroyed { entity: PhysicsEntityKey },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhysicsQueryHitDto {
    pub seq: u64,
    pub entity: PhysicsEntityKey,
    pub position: PhysicsVec3,
    pub normal: PhysicsVec3,
    pub distance: f32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsStepReportDto {
    pub fixed_tick: u64,
    pub dt: f32,
    pub substeps: u32,
    pub active_bodies: usize,
    pub static_bodies: usize,
    pub dynamic_bodies: usize,
    pub contacts: usize,
    pub commands_applied: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicsFrameOutput {
    pub fixed_tick: u64,
    #[serde(default)]
    pub pose_updates: Vec<PhysicsBodyPoseUpdate>,
    #[serde(default)]
    pub velocity_updates: Vec<PhysicsBodyVelocityUpdate>,
    #[serde(default)]
    pub events: Vec<PhysicsEventDto>,
    #[serde(default)]
    pub query_hits: Vec<PhysicsQueryHitDto>,
    pub report: PhysicsStepReportDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhysicsServiceRequest {
    Negotiate(PhysicsCapabilityNegotiationRequest),
    StepFrame(PhysicsFrameInput),
    DiagnosticsSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhysicsServiceResponse {
    Unit,
    Negotiation(PhysicsCapabilityNegotiationResponse),
    FrameOutput(PhysicsFrameOutput),
    BackendInfo(PhysicsBackendInfo),
    DiagnosticsSnapshot(PhysicsBackendInfo),
    Problem(PhysicsProblemDetails),
}
