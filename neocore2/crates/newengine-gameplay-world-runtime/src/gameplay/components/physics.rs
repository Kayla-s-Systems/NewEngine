/// Provider-neutral physics-world parameters consumed by the host physics bridge.
/// Product/gameplay packages may project their authored policy into this resource;
/// the reusable engine runtime must never read product-specific tuning directly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsWorldSettings {
    pub gravity: f32,
    pub contact_skin: f32,
    /// Maximum changed/new static triangle-mesh colliders registered per fixed tick.
    /// Bounding registration prevents large authored worlds from creating a single startup spike.
    pub static_collider_batch_size: usize,
}

impl PhysicsWorldSettings {
    #[inline]
    pub fn sanitized(self) -> Self {
        Self {
            gravity: if self.gravity.is_finite() {
                self.gravity.clamp(0.0, 80.0)
            } else {
                9.81
            },
            contact_skin: if self.contact_skin.is_finite() {
                self.contact_skin.clamp(0.0, 0.50)
            } else {
                0.035
            },
            static_collider_batch_size: self.static_collider_batch_size.clamp(1, 4096),
        }
    }
}

impl Default for PhysicsWorldSettings {
    #[inline]
    fn default() -> Self {
        Self {
            gravity: 9.81,
            contact_skin: 0.035,
            static_collider_batch_size: 128,
        }
    }
}

// Compatibility facade: ownership lives at the ECS-facing physics/world boundary.
pub use newengine_physics_world_api::{PhysicsSurface, StaticMeshCollider};
