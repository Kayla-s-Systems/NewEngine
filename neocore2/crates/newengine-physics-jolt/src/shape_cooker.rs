use newengine_physics_contracts::CollisionShapeDesc;

#[derive(Clone, Debug, PartialEq)]
pub struct CookedJoltShapeDesc {
    pub source: CollisionShapeDesc,
}

pub fn cook_shape(desc: CollisionShapeDesc) -> CookedJoltShapeDesc {
    CookedJoltShapeDesc { source: desc.sanitized() }
}
