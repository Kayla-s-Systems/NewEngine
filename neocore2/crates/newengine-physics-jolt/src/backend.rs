use newengine_physics_contracts::{PhysicsStepReport, PhysicsWorldDesc};

use crate::world::{JoltInitDesc, PhysicsError, PhysicsWorld};

/// Jolt backend adapter.
///
/// This type is intentionally backend-private: gameplay and scene code use
/// `newengine-physics-contracts` + `newengine-physics-runtime`, never Jolt
/// handles directly.
pub struct JoltPhysicsBackend {
    world: PhysicsWorld,
}

impl JoltPhysicsBackend {
    pub fn new(_desc: PhysicsWorldDesc, init: JoltInitDesc) -> Result<Self, PhysicsError> {
        Ok(Self { world: PhysicsWorld::new(init)? })
    }

    #[inline]
    pub fn step(&mut self, dt: f32, fixed_tick: u64) -> Result<PhysicsStepReport, PhysicsError> {
        self.world.step(dt)?;
        Ok(PhysicsStepReport {
            fixed_tick,
            dt,
            substeps: 1,
            active_bodies: 0,
            static_bodies: 0,
            dynamic_bodies: 0,
            contacts: 0,
            commands_applied: 0,
            events: Vec::new(),
        })
    }
}
