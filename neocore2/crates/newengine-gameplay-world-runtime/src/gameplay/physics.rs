use super::physics_queries::GameplayPhysicsQueryProviderRegistry;
use newengine_core::physics::PhysicsApiRef;
use newengine_ecs::World;

use crate::gameplay::StaticMeshCollider;
use std::collections::BTreeMap;
use std::time::Instant;

use newengine_world_authority_runtime::{
    current_entity_authority_map, current_world_authority_frame, RuntimeWorldAuthorityMode,
};

mod frame_input;
mod frame_output;
mod terrain_colliders;
mod util;

use frame_input::build_frame_input;
use frame_output::apply_frame_output;

#[derive(Clone, Copy, Debug, Default)]
pub struct PhysicsStepTimingTelemetry {
    pub frame_index: u64,
    pub fixed_tick: u64,
    pub input_build_ms: f32,
    pub backend_step_ms: f32,
    pub output_apply_ms: f32,
    pub bodies: u32,
    pub colliders: u32,
    pub commands: u32,
    pub queries: u32,
    pub pose_updates: u32,
    pub velocity_updates: u32,
    pub events: u32,
    pub query_hits: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PhysicsBackendWarmupState {
    pub attempted: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PhysicsStaticColliderSyncProgress {
    pub total: u32,
    pub registered: u32,
    pub pending: u32,
    pub failed: u32,
}

impl PhysicsStaticColliderSyncProgress {
    #[inline]
    pub const fn is_ready(self) -> bool {
        self.pending == 0 && self.failed == 0
    }
}

/// Forces lazy backend/Jolt initialization while the loading projection owns the
/// frame. The packet is intentionally empty and uses fixed_tick=0, so no ECS body
/// state is created or advanced; the first gameplay fixed tick remains tick 1.
pub fn prewarm_service_physics_backend(world: &mut World, physics_api: &PhysicsApiRef) {
    if world
        .resource::<PhysicsBackendWarmupState>()
        .map(|state| state.attempted)
        .unwrap_or(false)
    {
        return;
    }
    world.insert_resource(PhysicsBackendWarmupState { attempted: true });

    let started = Instant::now();
    let result = {
        let mut api = physics_api.lock();
        api.step_frame(newengine_core::physics::PhysicsFrameInput::empty(
            0,
            0,
            1.0 / 60.0,
        ))
    };
    let elapsed_ms = started.elapsed().as_secs_f32() * 1000.0;
    match result {
        Ok(_) => newengine_ulog_api::ulog::info!(
            "physics backend prewarm: completed under loading gate elapsed_ms={:.2} packet='empty fixed_tick=0' policy='no first-gameplay-tick cold init'",
            elapsed_ms
        ),
        Err(error) => newengine_ulog_api::ulog::warn!(
            "physics backend prewarm: failed under loading gate elapsed_ms={:.2} err='{}'; gameplay step will retry normal backend path",
            elapsed_ms,
            error
        ),
    }
}
/// Streams authored static-mesh collision into the external physics provider while the
/// loading screen owns the frame. Dynamic/gameplay bodies are intentionally omitted until
/// `WorldActivationState` becomes ready, so the player cannot integrate gravity before the
/// collision world exists in the provider.
pub fn sync_prelaunch_service_physics(world: &mut World, physics_api: &PhysicsApiRef) {
    let total = world
        .query::<StaticMeshCollider>()
        .count()
        .min(u32::MAX as usize) as u32;
    let mut sync = world
        .remove_resource::<PhysicsSyncModule>()
        .unwrap_or_default();
    let queries = GameplayPhysicsQueryProviderRegistry::new();
    let input = build_frame_input(
        world,
        0,
        0,
        1.0 / 60.0,
        &mut sync.static_mesh_revisions,
        &mut sync.static_mesh_observed_tick,
        &mut sync.static_mesh_backlog_pending,
        &queries,
    );
    let submitted = input.colliders.len() as u32;
    let commands = input.commands.len() as u32;

    if submitted == 0 && commands == 0 {
        let registered = sync.static_mesh_revisions.len().min(u32::MAX as usize) as u32;
        world.insert_resource(sync);
        world.insert_resource(PhysicsStaticColliderSyncProgress {
            total,
            registered: registered.min(total),
            pending: total.saturating_sub(registered),
            failed: 0,
        });
        return;
    }

    let result = {
        let mut api = physics_api.lock();
        api.step_frame(input)
    };
    match result {
        Ok(_) => {
            sync.step_failure_count = 0;
            let registered = sync.static_mesh_revisions.len().min(u32::MAX as usize) as u32;
            let progress = PhysicsStaticColliderSyncProgress {
                total,
                registered: registered.min(total),
                pending: total.saturating_sub(registered),
                failed: 0,
            };
            if submitted > 0 {
                newengine_ulog_api::ulog::info!(
                    "physics prelaunch collision sync: submitted={} registered={}/{} pending={} policy='loading-screen; collision-before-gameplay'",
                    submitted,
                    progress.registered,
                    progress.total,
                    progress.pending,
                );
            }
            world.insert_resource(sync);
            world.insert_resource(progress);
        }
        Err(error) => {
            sync.static_mesh_revisions.clear();
            sync.step_failure_count = sync.step_failure_count.saturating_add(1);
            let failure_count = sync.step_failure_count;
            world.insert_resource(sync);
            world.insert_resource(PhysicsStaticColliderSyncProgress {
                total,
                registered: 0,
                pending: total,
                failed: 1,
            });
            if failure_count <= 3 || failure_count.is_multiple_of(120) {
                newengine_ulog_api::ulog::warn!(
                    "physics prelaunch collision sync failed count={} colliders={} err='{}'; loading gate remains closed",
                    failure_count,
                    total,
                    error,
                );
            }
        }
    }
}

/// ECS-side synchronization layer for service-backed physics.
///
/// The backend receives `PhysicsFrameInput` packets and returns
/// `PhysicsFrameOutput`; all ECS reads/writes remain on the host side.
#[derive(Clone, Debug, Default)]
pub struct PhysicsSyncModule {
    fixed_tick: u64,
    missing_backend_logged: bool,
    /// Last static-mesh revision acknowledged by the service-backed physics world.
    /// Full triangle arrays cross the service boundary only on add/change.
    static_mesh_revisions: BTreeMap<u64, u64>,
    /// Last ECS change-tracking tick inspected for static-mesh membership/transform deltas.
    /// Steady fixed ticks skip the O(N static mesh) revision scan when no relevant component changed.
    static_mesh_observed_tick: u64,
    /// True while bounded static-collider registration still has unsent changed entries.
    static_mesh_backlog_pending: bool,
    step_failure_count: u64,
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
    gameplay_queries: &GameplayPhysicsQueryProviderRegistry,
) {
    let Some(api) = physics_api else {
        let should_log = ensure_sync_module(world)
            .map(|sync| sync.mark_missing_backend_logged())
            .unwrap_or(false);
        if should_log {
            newengine_ulog_api::ulog::warn!(
                "physics sync: no PhysicsApiRef registered; physics step skipped (no hidden in-process fallback)"
            );
        }
        return;
    };

    let frame_index = world
        .resource::<PhysicsRuntimeFrameIndex>()
        .map(|v| v.0)
        .unwrap_or(0);
    let mut sync = world
        .remove_resource::<PhysicsSyncModule>()
        .unwrap_or_default();
    let fixed_tick = sync.next_fixed_tick();
    if let Some(authority) = current_world_authority_frame(world) {
        if matches!(
            authority.mode,
            RuntimeWorldAuthorityMode::PluginEcsEntityAuthority
                | RuntimeWorldAuthorityMode::SplitAuthority
        ) {
            let provider_entities = current_entity_authority_map(world)
                .map(|map| map.native_to_provider.len())
                .unwrap_or(0);
            newengine_ulog_api::ulog::trace!(
                "physics sync: stepping from native component cache under service authority mode='{}' owner='{}' native_entities={} provider_entities={} source='authority-map'",
                authority.mode.as_str(),
                authority.route_snapshot.authority_label(),
                authority.native_entity_count,
                provider_entities
            );
        }
    }

    let input_started = Instant::now();
    let input = build_frame_input(
        world,
        frame_index,
        fixed_tick,
        dt,
        &mut sync.static_mesh_revisions,
        &mut sync.static_mesh_observed_tick,
        &mut sync.static_mesh_backlog_pending,
        gameplay_queries,
    );
    let input_build_ms = input_started.elapsed().as_secs_f32() * 1000.0;
    let bodies = input.bodies.len() as u32;
    let colliders = input.colliders.len() as u32;
    let commands = input.commands.len() as u32;
    let queries = input.queries.len() as u32;
    world.insert_resource(sync);

    let backend_started = Instant::now();
    let output = {
        let mut api = api.lock();
        match api.step_frame(input) {
            Ok(output) => output,
            Err(err) => {
                let error_text = err.to_string();
                let update_error = error_text.contains("Jolt update returned error code")
                    || error_text.contains("body_pair_cache_full")
                    || error_text.contains("contact_constraints_full")
                    || error_text.contains("manifold_cache_full");
                let failure_count = if let Some(sync) = world.resource_mut::<PhysicsSyncModule>() {
                    sync.step_failure_count = sync.step_failure_count.saturating_add(1);
                    // Jolt packet sync happens before PhysicsSystem_Update. Update-capacity
                    // errors do not roll back the already-created bodies, so retaining
                    // revisions prevents catastrophic full geometry resend on the next tick.
                    // For non-update/service errors the revision map is cleared so the
                    // host can safely replay state after recovery.
                    if !update_error {
                        sync.static_mesh_revisions.clear();
                    }
                    sync.step_failure_count
                } else {
                    1
                };
                if failure_count <= 3 || failure_count.is_multiple_of(120) {
                    newengine_ulog_api::ulog::warn!(
                        "physics sync: engine.physics step failed count={} update_error={} retry_policy='{}': {}",
                        failure_count,
                        update_error,
                        if update_error {
                            "retain-static-revisions"
                        } else {
                            "replay-static-state"
                        },
                        error_text
                    );
                }
                return;
            }
        }
    };
    let backend_step_ms = backend_started.elapsed().as_secs_f32() * 1000.0;
    let completed_impulses = world
        .query::<crate::gameplay::PendingPhysicsImpulse>()
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();
    for entity in completed_impulses {
        let _ = world.remove::<crate::gameplay::PendingPhysicsImpulse>(entity);
    }
    if let Some(sync) = world.resource_mut::<PhysicsSyncModule>() {
        sync.step_failure_count = 0;
    }
    let pose_updates = output.pose_updates.len() as u32;
    let velocity_updates = output.velocity_updates.len() as u32;
    let events = output.events.len() as u32;
    let query_hits = output.query_hits.len() as u32;

    let apply_started = Instant::now();
    apply_frame_output(world, output, gameplay_queries);
    let output_apply_ms = apply_started.elapsed().as_secs_f32() * 1000.0;
    world.insert_resource(PhysicsStepTimingTelemetry {
        frame_index,
        fixed_tick,
        input_build_ms,
        backend_step_ms,
        output_apply_ms,
        bodies,
        colliders,
        commands,
        queries,
        pose_updates,
        velocity_updates,
        events,
        query_hits,
    });
    if fixed_tick <= 3
        || (fixed_tick.is_multiple_of(30)
            && newengine_runtime_policy::simulation_runtime_policy().physics_stage_log)
    {
        newengine_ulog_api::ulog::info!(
            "physics.step.profile: frame={} fixed_tick={} input_ms={:.3} backend_ms={:.3} apply_ms={:.3} bodies={} colliders={} commands={} queries={} poses={} velocities={} events={} query_hits={}",
            frame_index,
            fixed_tick,
            input_build_ms,
            backend_step_ms,
            output_apply_ms,
            bodies,
            colliders,
            commands,
            queries,
            pose_updates,
            velocity_updates,
            events,
            query_hits,
        );
    }
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
