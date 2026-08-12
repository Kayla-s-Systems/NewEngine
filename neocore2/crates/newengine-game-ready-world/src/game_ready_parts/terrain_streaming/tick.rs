use super::*;

fn responsive_stream_commit_budget(
    target_budget: usize,
    state: &mut GameReadyTerrainStreamingState,
) -> usize {
    if target_budget == 0 {
        return 0;
    }

    let burst = crate::env_config::var_usize(
        "NEWENGINE_SCENE_TERRAIN_STREAM_BURST",
        1,
        1,
        target_budget.max(1),
    );
    let interval_ms =
        crate::env_config::var_u64("NEWENGINE_SCENE_TERRAIN_STREAM_INTERVAL_MS", 140, 16, 2_000);

    let now = Instant::now();
    if let Some(last) = state.last_stream_commit_at {
        if now.duration_since(last) < Duration::from_millis(interval_ms) {
            return 0;
        }
    }

    state.last_stream_commit_at = Some(now);
    state.stream_commit_count = state.stream_commit_count.saturating_add(1);
    burst
}

pub(crate) fn tick_game_ready_streaming_terrain(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    thread_pool: Option<&ThreadPoolHandle>,
) {
    let Some(player) = newengine_engine_runtime::gameplay::first_player(world) else {
        return;
    };
    let player_pos = world
        .get::<Transform>(player)
        .map(|t| t.position)
        .unwrap_or(Vec3::ZERO);
    let role_anchor = newengine_engine_runtime::gameplay::scene_entity_by_role(
        world,
        newengine_engine_runtime::gameplay::SceneEntityRole::TerrainStreamingAnchor,
    );

    let Some(mut state) = world.remove_resource::<GameReadyTerrainStreamingState>() else {
        return;
    };
    state.anchor = role_anchor.unwrap_or(state.anchor);
    let follow_player = world
        .get::<newengine_engine_runtime::gameplay::SceneAnchorFollow>(state.anchor)
        .map(|follow| follow.enabled)
        .unwrap_or(false);
    if follow_player {
        if let Some(t) = world.get_mut_tracked::<Transform>(state.anchor) {
            t.position = player_pos;
        }
    }
    let anchor_pos = world
        .get::<Transform>(state.anchor)
        .map(|t| t.position)
        .unwrap_or(player_pos);

    let center =
        TerrainChunkCoord::from_world_pos(anchor_pos, state.spec.size_x, state.spec.size_z);
    let budget = SceneStreamingBudget {
        resident_radius: state.chunk_radius,
        unload_radius: state.unload_radius,
        max_commits_per_tick: state.max_chunks_per_frame,
    }
    .sanitized();
    state.chunk_radius = budget.resident_radius;
    state.unload_radius = budget.unload_radius;
    state.max_chunks_per_frame = budget.max_commits_per_tick;
    state.max_pending_jobs = state
        .max_pending_jobs
        .max(budget.max_commits_per_tick.saturating_mul(4).max(4));

    let profile = SceneStreamingProfile {
        render: budget,
        simulation: SceneStreamingBudget {
            resident_radius: budget.resident_radius.saturating_add(2),
            unload_radius: budget.unload_radius.saturating_add(2),
            // Coarse simulation is intentionally cheaper than render residency.
            max_commits_per_tick: budget.max_commits_per_tick.saturating_div(2).max(1),
        },
    }
    .sanitized();
    let layered_plan = SceneLayeredStreamingPlan::build(
        center,
        profile,
        state.loaded.keys().copied(),
        state.pending.keys().copied(),
        std::iter::empty::<TerrainChunkCoord>(),
        std::iter::empty::<TerrainChunkCoord>(),
    );
    let plan = &layered_plan.render;
    let bucket_plan = SceneBucketedCellPlan::from_desired_sets(
        center,
        layered_plan.render.desired.iter().copied(),
        layered_plan.simulation.desired.iter().copied(),
    );

    let loaded_coords = state
        .loaded
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let pending_coords = state
        .pending
        .keys()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let completed_ready = state
        .pending
        .keys()
        .copied()
        .filter(|coord| {
            state
                .pending
                .get(coord)
                .map(|pending| pending.ticket.is_complete())
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    let stream_requests_ready = bucket_plan
        .cells
        .iter()
        .filter(|cell| cell.bucket.wants_render_residency())
        .map(|cell| cell.coord)
        .filter(|coord| !loaded_coords.contains(coord) && !pending_coords.contains(coord))
        .collect::<Vec<_>>();

    let target_commit_budget = budget.max_commits_per_tick.max(1);
    let commit_budget = if completed_ready.is_empty() && stream_requests_ready.is_empty() {
        0
    } else {
        responsive_stream_commit_budget(target_commit_budget, &mut state)
    };
    let mut created = 0usize;
    for coord in completed_ready.into_iter().take(commit_budget) {
        let Some(pending) = state.pending.remove(&coord) else {
            continue;
        };
        let generated = pending.result.lock().ok().and_then(|mut slot| slot.take());
        if let Some(generated) = generated {
            let record = spawn_generated_terrain_chunk(
                world,
                state.root,
                mats,
                state.material,
                &state.spec,
                &state.surface,
                state.color,
                coord,
                generated,
            );
            state.loaded.insert(coord, record);
            created += 1;
        }
    }

    let remaining_commit_budget = commit_budget.saturating_sub(created);
    let stream_requests = stream_requests_ready
        .into_iter()
        .take(remaining_commit_budget)
        .collect::<Vec<_>>();

    let mut scheduled = 0usize;
    for coord in stream_requests {
        if enqueue_streamed_terrain_chunk(&mut state, thread_pool, coord) {
            scheduled += 1;
            continue;
        }

        let record = spawn_streamed_terrain_chunk(
            world,
            state.root,
            mats,
            state.material,
            &state.spec,
            &state.surface,
            state.color,
            coord,
            state.heightmap.as_deref(),
        );
        state.loaded.insert(coord, record);
        created += 1;
        scheduled += 1;
    }

    let mut removed = 0usize;
    for request in &plan.unloads {
        let coord = request.coord;
        if let Some(record) = state.loaded.remove(&coord) {
            let _ = world.despawn(record.terrain);
            removed += 1;
        }
    }

    let to_drop_pending = state
        .pending
        .keys()
        .copied()
        .filter(|coord| coord.chebyshev_distance(center) > budget.unload_radius)
        .collect::<Vec<_>>();
    let mut cancelled_pending = 0usize;
    for coord in to_drop_pending {
        if let Some(pending) = state.pending.remove(&coord) {
            if !pending.ticket.is_complete() {
                let _ = pending.ticket.cancel();
            }
            cancelled_pending = cancelled_pending.saturating_add(1);
        }
    }

    let streaming_changed = created > 0 || scheduled > 0 || removed > 0 || cancelled_pending > 0;
    let render_desired = bucket_plan
        .cells
        .iter()
        .filter(|cell| cell.bucket.wants_render_residency())
        .count();
    let simulation_desired = bucket_plan
        .cells
        .iter()
        .filter(|cell| cell.bucket.wants_simulation_residency())
        .count();
    let reached_render_target = state.loaded.len() >= render_desired && state.pending.is_empty();
    let diagnostics_due = removed > 0
        || cancelled_pending > 0
        || state.stream_commit_count.is_multiple_of(16)
        || reached_render_target;

    if streaming_changed && diagnostics_due {
        newengine_ulog_api::ulog::debug!(
            "game-ready terrain streaming: center=[{},{}] anchor={:?} follow_player={} render_loaded={} render_pending={} created={} scheduled={} removed={} cancelled_pending={} commit_budget={} commit_count={} render_desired={} render_unloads={} sim_desired={}",
            center.x,
            center.z,
            state.anchor,
            follow_player,
            state.loaded.len(),
            state.pending.len(),
            created,
            scheduled,
            removed,
            cancelled_pending,
            commit_budget,
            state.stream_commit_count,
            render_desired,
            plan.unloads.len(),
            simulation_desired,
        );
        let _ = validate_scene_objects(world, "game-ready.streaming-terrain");
    }

    world.insert_resource(state);
}
