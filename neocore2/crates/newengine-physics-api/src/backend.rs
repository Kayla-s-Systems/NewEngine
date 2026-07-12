use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PhysicsApiVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl PhysicsApiVersion {
    #[inline]
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl Default for PhysicsApiVersion {
    #[inline]
    fn default() -> Self {
        Self::new(1, 0, 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicsBackendClass {
    Null,
    Deterministic,
    Native,
}

impl Default for PhysicsBackendClass {
    #[inline]
    fn default() -> Self {
        Self::Deterministic
    }
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
        Self {
            max_bodies: 100_000,
            max_queries_per_frame: 4096,
            max_substeps: 16,
        }
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
    fn profile(backend_class: PhysicsBackendClass, features: Vec<PhysicsFeature>) -> Self {
        Self {
            backend_class,
            features,
            limits: PhysicsLimits::default(),
        }
    }

    #[inline]
    pub fn deterministic_default() -> Self {
        Self::profile(
            PhysicsBackendClass::Deterministic,
            vec![
                PhysicsFeature::StaticColliders,
                PhysicsFeature::DynamicBodies,
                PhysicsFeature::Contacts,
                PhysicsFeature::DeterministicReplay,
            ],
        )
    }

    #[inline]
    pub fn null_default() -> Self {
        Self::profile(PhysicsBackendClass::Null, Vec::new())
    }

    #[inline]
    pub fn native_backend_default() -> Self {
        Self::profile(
            PhysicsBackendClass::Native,
            vec![
                PhysicsFeature::StaticColliders,
                PhysicsFeature::DynamicBodies,
                PhysicsFeature::KinematicBodies,
                PhysicsFeature::TriggerBodies,
                PhysicsFeature::Queries,
                PhysicsFeature::NativeBackend,
                PhysicsFeature::HeightfieldColliders,
                PhysicsFeature::MeshColliders,
            ],
        )
    }

    #[inline]
    pub fn supports(&self, feature: PhysicsFeature) -> bool {
        self.features.contains(&feature)
    }
}

impl Default for PhysicsBackendCapabilities {
    #[inline]
    fn default() -> Self {
        Self::deterministic_default()
    }
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
        Self {
            code: code.into(),
            message: message.into(),
        }
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
    pub fn new(
        code: impl Into<String>,
        title: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
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
