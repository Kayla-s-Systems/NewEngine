#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ecs::World;

use super::types::{PhysicsCtx, PhysicsDebugStats, PhysicsInitDesc, PhysicsSettings, PhysicsStepState};

#[inline]
pub fn physics_set_interpolation_alpha(world: &mut World, alpha: f32) {
    if world.resource::<PhysicsCtx>().is_none() {
        return;
    }
    if let Some(s) = world.resource_mut::<PhysicsStepState>() {
        s.alpha = alpha.clamp(0.0, 1.0);
    }
}

pub fn physics_bootstrap(world: &mut World, desc: PhysicsInitDesc) -> bool {
    if world.resource::<PhysicsCtx>().is_some() {
        if world.resource::<PhysicsSettings>().is_none() {
            world.insert_resource(desc.settings);
        }
        if world.resource::<PhysicsStepState>().is_none() {
            world.insert_resource(PhysicsStepState::default());
        }
        return true;
    }

    world.insert_resource(desc.settings);
    world.insert_resource(PhysicsStepState::default());

    match PhysicsCtx::new(desc.jolt) {
        Ok(ctx) => {
            world.insert_resource(ctx);
            true
        }
        Err(_) => false,
    }
}

pub fn physics_bootstrap_default(world: &mut World, _frame: super::super::SimFrame) {
    let _ = physics_bootstrap(world, PhysicsInitDesc::default());
}

#[inline]
pub fn physics_debug_stats(world: &World) -> Option<PhysicsDebugStats> {
    let _ = world.resource::<PhysicsCtx>()?;
    let s = world.resource::<PhysicsStepState>()?;
    let bodies_total = world.query::<super::types::PhysicsBody>().count() as u32;
    Some(PhysicsDebugStats {
        tick: s.tick,
        alpha: s.alpha,
        steps_last: s.steps_last,
        bodies_total,
    })
}