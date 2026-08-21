use newengine_core::physics::PhysicsBackendCapabilities;
pub use newengine_physics_api::PHYSICS_BACKEND_SERVICE_SPEC;

#[derive(Debug, Clone)]
pub struct ResolvedPhysicsBackendConfig {
    pub backend_id: String,
    pub debug_text: String,
    pub capabilities: PhysicsBackendCapabilities,
}
