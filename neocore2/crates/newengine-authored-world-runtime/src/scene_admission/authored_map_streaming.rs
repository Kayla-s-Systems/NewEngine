use super::*;
pub(super) use crate::AuthoredMapCellDomain as AuthoredCellDomain;
use crate::{
    AuthoredMapStreamingController, AuthoredMapStreamingRuntimeTuning,
    AuthoredPreparedCellAdmission,
};

use super::streaming::{cancel_static_world_cell_domain, enqueue_static_world_prefabs};
use super::{BOX_COLLISION_WORLD_PROXY, COLLISION_WORLD_PROXY, DYNAMIC_WORLD_PROXY};
use crate::{AuthoredMapStreamingSpec, AuthoredWorldPlacementSpec};
use newengine_core::ThreadPoolHandle;
use std::collections::{BTreeMap, BTreeSet};

type CellCoord = newengine_assets_api::MapCellCoordV1;

#[inline]
pub(super) fn static_world_prefab_domain(
    prefab: &AuthoredWorldPlacementSpec,
) -> AuthoredCellDomain {
    let proxy = prefab.proxy.trim();
    if proxy.eq_ignore_ascii_case(COLLISION_WORLD_PROXY)
        || proxy.eq_ignore_ascii_case(BOX_COLLISION_WORLD_PROXY)
        || proxy.eq_ignore_ascii_case(DYNAMIC_WORLD_PROXY)
    {
        AuthoredCellDomain::Simulation
    } else {
        AuthoredCellDomain::Render
    }
}

/// Authored-world runtime owns ECS/material admission state. Residency planning, prediction,
/// preparation scheduling, job ownership and map-definition caching live in the generic
/// authored-world controller.
pub(super) struct AuthoredMapSceneStreamingState {
    parent: EntityId,
    controller: AuthoredMapStreamingController,
    placement_ids: BTreeMap<CellCoord, Vec<String>>,
}

include!("authored_map_streaming/roots.rs");

fn unload_domain_collect(
    world: &mut newengine_ecs::World,
    state: &mut AuthoredMapSceneStreamingState,
    coord: CellCoord,
    domain: AuthoredCellDomain,
    primitive_candidates: &mut BTreeSet<PrimitiveId>,
) -> usize {
    let cancelled =
        cancel_static_world_cell_domain(world, state.controller.logical_map_ref(), coord, domain);
    let removed = take_domain_root(world, coord, domain)
        .filter(|root| world.exists(*root))
        .map(|root| newengine_transform::despawn_hierarchy(world, root))
        .unwrap_or(0);
    take_primitive_release_candidates(world, coord, domain, primitive_candidates);

    state.controller.mark_domain_unloaded(coord, domain);
    if !state.controller.has_any_resident_domain(coord) {
        if let Some(cell_root) =
            remove_empty_cell_root(world, coord).filter(|root| world.exists(*root))
        {
            let _ = newengine_transform::despawn_hierarchy(world, cell_root);
        }
        state.placement_ids.remove(&coord);
    }

    let diagnostics = state.controller.diagnostics();
    newengine_ulog_api::ulog::debug!(
        "authored map cell domain unloaded map='{}' cell={},{} domain='{}' cancelled_pending={} entities_removed={} render_resident={} simulation_resident={}",
        state.controller.map_ref(),
        coord.x,
        coord.z,
        domain.label(),
        cancelled,
        removed,
        diagnostics.render_resident,
        diagnostics.simulation_resident,
    );
    removed
}

fn replan_residency(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    state: &mut AuthoredMapSceneStreamingState,
    focus: crate::AuthoredMapStreamingFocus,
) {
    let plan = state.controller.replan(focus);
    let mut primitive_candidates = BTreeSet::new();
    for coord in plan.unload_render {
        unload_domain_collect(
            world,
            state,
            coord,
            AuthoredCellDomain::Render,
            &mut primitive_candidates,
        );
    }
    for coord in plan.unload_simulation {
        unload_domain_collect(
            world,
            state,
            coord,
            AuthoredCellDomain::Simulation,
            &mut primitive_candidates,
        );
    }
    let released = finalize_primitive_releases(world, prims, primitive_candidates);
    if released > 0 {
        newengine_ulog_api::ulog::debug!(
            "authored map primitive eviction batch map='{}' released={}",
            state.controller.map_ref(),
            released
        );
    }
}

fn admit_prepared_domains(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    state: &mut AuthoredMapSceneStreamingState,
    admission: AuthoredPreparedCellAdmission,
) -> usize {
    let coord = admission.coord;
    let prepared = admission.prepared;
    let mut admitted_domains = 0usize;

    if admission.wants_render {
        let _ = ensure_domain_root(
            world,
            state.parent,
            state.controller.map_ref(),
            coord,
            AuthoredCellDomain::Render,
        );
        enqueue_static_world_prefabs(world, mats, state.parent, &prepared.render_placements);
        state
            .controller
            .mark_domain_resident(coord, AuthoredCellDomain::Render);
        admitted_domains = admitted_domains.saturating_add(1);
    }

    if admission.wants_simulation {
        let _ = ensure_domain_root(
            world,
            state.parent,
            state.controller.map_ref(),
            coord,
            AuthoredCellDomain::Simulation,
        );
        enqueue_static_world_prefabs(world, mats, state.parent, &prepared.simulation_placements);
        state
            .controller
            .mark_domain_resident(coord, AuthoredCellDomain::Simulation);
        admitted_domains = admitted_domains.saturating_add(1);
    }

    if admitted_domains > 0 {
        state
            .placement_ids
            .entry(coord)
            .or_insert_with(|| prepared.placement_ids.clone());

        if prepared.metadata_only_count > 0 {
            newengine_ulog_api::ulog::warn!(
                "authored map streamed cell contains metadata-only placements map='{}' cell={},{} count={} policy='global domain metadata belongs in map index/startup cell; runtime mutation skipped'",
                state.controller.map_ref(),
                coord.x,
                coord.z,
                prepared.metadata_only_count,
            );
        }
        let diagnostics = state.controller.diagnostics();
        newengine_ulog_api::ulog::info!(
            "authored map cell domains resident map='{}' cell={},{} placements={} render_prefabs={} simulation_prefabs={} render_resident={} simulation_resident={}",
            state.controller.map_ref(),
            coord.x,
            coord.z,
            prepared.authored_placement_count,
            prepared.render_placements.len(),
            prepared.simulation_placements.len(),
            diagnostics.render_resident,
            diagnostics.simulation_resident,
        );
    }
    admitted_domains
}

fn admit_ready_cells(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    state: &mut AuthoredMapSceneStreamingState,
) -> usize {
    let mut admitted_cells = 0usize;
    for _ in 0..state.controller.max_cells_per_tick() {
        let Some(admission) = state.controller.take_next_prepared() else {
            break;
        };
        if admit_prepared_domains(world, mats, state, admission) > 0 {
            admitted_cells = admitted_cells.saturating_add(1);
        }
    }
    admitted_cells
}

pub fn begin_authored_map_streaming(
    world: &mut newengine_ecs::World,
    parent: EntityId,
    spec: Option<&AuthoredMapStreamingSpec>,
) {
    let Some(spec) = spec else {
        return;
    };

    world.insert_resource(AuthoredMapCellRoots::default());
    world.insert_resource(AuthoredMapPrimitiveResidency::default());

    for coord in spec.initial_render_cells.iter().copied() {
        let _ = ensure_domain_root(
            world,
            parent,
            &spec.map_ref,
            coord,
            AuthoredCellDomain::Render,
        );
    }
    for coord in spec.initial_simulation_cells.iter().copied() {
        let _ = ensure_domain_root(
            world,
            parent,
            &spec.map_ref,
            coord,
            AuthoredCellDomain::Simulation,
        );
    }

    let tuning = AuthoredMapStreamingRuntimeTuning::from_host_environment(spec);
    let controller = AuthoredMapStreamingController::new(spec, tuning);
    world.insert_resource(AuthoredMapSceneStreamingState {
        parent,
        controller,
        placement_ids: spec.initial_placement_ids.clone(),
    });

    newengine_ulog_api::ulog::info!(
        "authored map streaming initialized map='{}' cells_total={} render_resident={} simulation_resident={} render_radius={} simulation_radius={} render_unload_radius={} simulation_unload_radius={} max_cells_per_tick={} max_pending_jobs={} policy='generic authored-world controller owns prediction/residency/preparation; authored-world runtime owns ECS/material admission'",
        spec.map_ref,
        spec.index.cells.len(),
        spec.initial_render_cells.len(),
        spec.initial_simulation_cells.len(),
        spec.render_radius,
        spec.simulation_radius,
        spec.render_unload_radius,
        spec.simulation_unload_radius,
        spec.max_cells_per_tick,
        tuning.max_pending_jobs,
    );
}

pub fn tick_authored_map_streaming(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    thread_pool: Option<&ThreadPoolHandle>,
) {
    // The profile's initial cells are the launch working set. Do not let the steady-state
    // controller expand render/simulation residency while WorldAssemblyProgress is still
    // gating public Play, otherwise post-launch cells become launch-critical by accident.
    let activation_ready = world
        .resource::<newengine_engine_runtime::gameplay::WorldActivationState>()
        .map(|gate| gate.is_ready())
        .unwrap_or(true);
    if !activation_ready {
        return;
    }
    let Some(player) = newengine_engine_runtime::gameplay::first_player(world) else {
        return;
    };
    let player_position = world
        .get::<Transform>(player)
        .map(|transform| transform.position)
        .unwrap_or(Vec3::ZERO);
    let player_velocity = world
        .get::<newengine_sim::Velocity>(player)
        .map(|velocity| velocity.0)
        .unwrap_or(Vec3::ZERO);

    let Some(mut state) = world.remove_resource::<AuthoredMapSceneStreamingState>() else {
        return;
    };
    let Some(focus) = state.controller.focus_for_world_motion(
        [player_position.x, player_position.y, player_position.z],
        [player_velocity.x, player_velocity.y, player_velocity.z],
    ) else {
        world.insert_resource(state);
        return;
    };

    let focus_changed = state.controller.focus_changed(focus);
    if focus_changed {
        replan_residency(world, prims, &mut state, focus);
    }

    state.controller.process_preparation(thread_pool);
    let admitted = admit_ready_cells(world, mats, &mut state);

    if focus_changed || admitted > 0 {
        let diagnostics = state.controller.diagnostics();
        newengine_ulog_api::ulog::debug!(
            "authored map streaming tick map='{}' center={},{} predicted={},{} speed_mps={:.2} read_ahead_sec={:.2} max_read_ahead_cells={} admitted_cells={} render_resident={} simulation_resident={} queued={} jobs={} ready={} prepared={} failed={}",
            state.controller.map_ref(),
            focus.center.x,
            focus.center.z,
            focus.predicted_center.x,
            focus.predicted_center.z,
            focus.speed_mps,
            state.controller.read_ahead_sec(),
            state.controller.max_read_ahead_cells(),
            admitted,
            diagnostics.render_resident,
            diagnostics.simulation_resident,
            diagnostics.queued,
            diagnostics.jobs,
            diagnostics.ready,
            diagnostics.prepared,
            diagnostics.failed,
        );
    }

    world.insert_resource(state);
}

#[cfg(test)]
#[path = "authored_map_streaming/tests.rs"]
mod tests;
