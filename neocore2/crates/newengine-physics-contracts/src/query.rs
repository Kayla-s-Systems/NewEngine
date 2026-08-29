use newengine_entity_api::EntityHandle;
use newengine_math::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PhysicsQueryKind {
    Ray { origin: Vec3, dir: Vec3, max_t: f32 },
    Sphere { center: Vec3, radius: f32 },
    Aabb { min: Vec3, max: Vec3 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsQuery {
    pub seq: u64,
    pub kind: PhysicsQueryKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsQueryHit {
    pub entity: EntityHandle,
    pub position: Vec3,
    pub normal: Vec3,
    pub distance: f32,
}
