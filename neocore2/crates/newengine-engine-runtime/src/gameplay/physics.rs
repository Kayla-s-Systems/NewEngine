use newengine_core::physics::PhysicsApiRef;
use newengine_ecs::World;

mod frame_input;
mod frame_output;
mod terrain_colliders;
mod util;

use frame_input::build_frame_input;
use frame_output::apply_frame_output;

/// ECS-side synchronization layer for service-backed physics.
///
/// The backend receives `PhysicsFrameInput` packets and returns
/// `PhysicsFrameOutput`; all ECS reads/writes remain on the host side.
#[derive(Clone, Debug, Default)]
pub struct PhysicsSyncModule {
    fixed_tick: u64,
    missing_backend_logged: bool,
}

impl PhysicsSyncModule {
    #[inline]
    pub fn next_fixed_tick(&mut self) -> u64 {
        self.fixed_tick = self.fixed_tick.wrapping_add(1);
        self.fixed_tick
    }

    #[inline]
    pub fn mark_missing_backend_logged(&mut self) -> bool {
        if self.missing_backend_logged {
            false
        } else {
            self.missing_backend_logged = true;
            true
        }
    }
}

#[inline]
pub(super) fn step_service_physics(
    world: &mut World,
    dt: f32,
    physics_api: Option<&PhysicsApiRef>,
) {
    let Some(api) = physics_api else {
        let should_log = ensure_sync_module(world)
            .map(|sync| sync.mark_missing_backend_logged())
            .unwrap_or(false);
        if should_log {
            log::warn!(
                "physics sync: no PhysicsApiRef registered; physics step skipped (no hidden in-process fallback)"
            );
        }
        return;
    };

    let frame_index = world
        .resource::<PhysicsRuntimeFrameIndex>()
        .map(|v| v.0)
        .unwrap_or(0);
    let fixed_tick = ensure_sync_module(world)
        .map(|sync| sync.next_fixed_tick())
        .unwrap_or(0);
    let input = build_frame_input(world, frame_index, fixed_tick, dt);

    let output = {
        let mut api = api.lock();
        match api.step_frame(input) {
            Ok(output) => output,
            Err(err) => {
                log::warn!("physics sync: engine.physics step failed: {}", err);
                return;
            }
        }
    };

    apply_frame_output(world, output);
}

fn ensure_sync_module(world: &mut World) -> Option<&mut PhysicsSyncModule> {
    if world.resource::<PhysicsSyncModule>().is_none() {
        world.insert_resource(PhysicsSyncModule::default());
    }
    world.resource_mut::<PhysicsSyncModule>()
}

/// Optional resource used by callers that want frame-indexed physics packets.
/// The sync path does not require this resource; absent means frame 0.
#[derive(Clone, Copy, Debug, Default)]
pub struct PhysicsRuntimeFrameIndex(pub u64);
