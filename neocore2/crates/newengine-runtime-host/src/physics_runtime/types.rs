use newengine_core::physics::PhysicsBackendCapabilities;
use newengine_physics_api::{PHYSICS_BACKEND_CAPABILITY_ID, PHYSICS_SERVICE_ID};
use newengine_service_api::BackendServiceSpec;

pub const PHYSICS_BACKEND_SERVICE_SPEC: BackendServiceSpec = BackendServiceSpec::new(
    "physics",
    PHYSICS_SERVICE_ID,
    PHYSICS_BACKEND_CAPABILITY_ID,
);

#[derive(Debug, Clone)]
pub struct ResolvedPhysicsBackendConfig {
    pub backend_id: String,
    pub debug_text: String,
    pub capabilities: PhysicsBackendCapabilities,
}
