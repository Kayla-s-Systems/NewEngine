use newengine_core::physics::PhysicsBackendCapabilities;

pub const PHYSICS_BACKEND_CAPABILITY_ID: &str = newengine_physics_api::PHYSICS_BACKEND_CAPABILITY_ID;

#[derive(Debug, Clone)]
pub struct ResolvedPhysicsBackendConfig {
    pub backend_id: String,
    pub debug_text: String,
    pub capabilities: PhysicsBackendCapabilities,
}
