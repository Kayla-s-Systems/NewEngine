use super::*;

fn responsive_stream_commit_budget(
    target_budget: usize,
    state: &mut AuthoredTerrainStreamingState,
) -> usize {
    if target_budget == 0 {
        return 0;
    }

    let burst = newengine_runtime_env::var_usize(
        "NEWENGINE_SCENE_TERRAIN_STREAM_BURST",
        1,
        1,
        target_budget.max(1),
    );
    let interval_ms =
        newengine_runtime_env::var_u64("NEWENGINE_SCENE_TERRAIN_STREAM_INTERVAL_MS", 16, 16, 2_000);

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

#[inline]
fn predictive_streaming_center(
    anchor_pos: Vec3,
    velocity: Vec3,
    spec: &AuthoredTerrainSpec,
    budget: SceneStreamingBudget,
) -> (SceneCellCoord, SceneCellCoord, f32, f32, i32) {
    let center = TerrainChunkCoord::from_world_pos(anchor_pos, spec.size_x, spec.size_z);
    let horizontal_velocity = Vec3::new(velocity.x, 0.0, velocity.z);
    let speed = horizontal_velocity.length();
    let forward = if speed > 1.0e-4 {
        horizontal_velocity / speed
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    };
    let read_ahead_sec =
        newengine_runtime_env::var_f32("NEWENGINE_SCENE_TERRAIN_READ_AHEAD_SEC", 0.75, 0.0, 3.0);
    let observer = SceneStreamingObserver::at(anchor_pos).with_motion(
        forward,
        horizontal_velocity,
        read_ahead_sec,
    );
    let raw_prediction = observer.cell(spec.size_x, spec.size_z);

    // A velocity spike, teleport or long camera cut must not create a second full scene far
    // away from the authoritative focus. Mature request-list streamers prefetch nearby future
    // searches, but stop speculative searches across long cuts. Clamp the secondary focus and
    // let the next authoritative frame move the primary center normally.
    let default_max_cells = budget.resident_radius.saturating_add(1).clamp(1, 4);
    let max_read_ahead_cells = newengine_runtime_env::var_i32(
        "NEWENGINE_SCENE_TERRAIN_MAX_READ_AHEAD_CELLS",
        default_max_cells,
        0,
        SceneStreamingBudget::MAX_RESIDENT_RADIUS,
    );
    let dx = (raw_prediction.x - center.x).clamp(-max_read_ahead_cells, max_read_ahead_cells);
    let dz = (raw_prediction.z - center.z).clamp(-max_read_ahead_cells, max_read_ahead_cells);
    let predicted = TerrainChunkCoord {
        x: center.x.saturating_add(dx),
        z: center.z.saturating_add(dz),
    };
    (
        center,
        predicted,
        speed,
        read_ahead_sec,
        max_read_ahead_cells,
    )
}

#[inline]
fn terrain_commit_budget_ms() -> f32 {
    newengine_runtime_env::var_f32("NEWENGINE_SCENE_TERRAIN_COMMIT_BUDGET_MS", 2.0, 0.25, 16.0)
}

#[inline]
fn terrain_task_priority(bucket: newengine_scene::SceneStreamingBucket) -> TaskPriority {
    match bucket {
        newengine_scene::SceneStreamingBucket::ActiveSimulation
        | newengine_scene::SceneStreamingBucket::VisibleNear => TaskPriority::Critical,
        newengine_scene::SceneStreamingBucket::PredictedNear => TaskPriority::Interactive,
        newengine_scene::SceneStreamingBucket::VisibleFar => TaskPriority::Normal,
        _ => TaskPriority::Background,
    }
}

fn preempt_lower_priority_pending(
    state: &mut AuthoredTerrainStreamingState,
    incoming_score: i32,
) -> bool {
    let candidate = state
        .pending
        .iter()
        .filter(|(_, pending)| !pending.ticket.is_complete())
        .filter(|(_, pending)| pending.request_score < incoming_score)
        .min_by_key(|(coord, pending)| (pending.request_score, coord.x, coord.z))
        .map(|(coord, _)| *coord);
    let Some(coord) = candidate else {
        return false;
    };
    let Some(pending) = state.pending.remove(&coord) else {
        return false;
    };
    let _ = pending.ticket.cancel();
    true
}

#[inline]
fn terrain_stream_request_budget(target_commit_budget: usize, max_pending_jobs: usize) -> usize {
    let adaptive_default = target_commit_budget.saturating_mul(2).max(2);
    newengine_runtime_env::var_usize(
        "NEWENGINE_SCENE_TERRAIN_REQUESTS_PER_TICK",
        adaptive_default.min(max_pending_jobs.max(1)),
        1,
        max_pending_jobs.max(1),
    )
}

pub fn tick_authored_streaming_terrain(
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
    let player_velocity = world
        .get::<newengine_sim::Velocity>(player)
        .map(|velocity| velocity.0)
        .unwrap_or(Vec3::ZERO);
    let role_anchor = newengine_engine_runtime::gameplay::scene_entity_by_role(
        world,
        newengine_engine_runtime::gameplay::SceneEntityRole::TerrainStreamingAnchor,
    );

    let Some(mut state) = world.remove_resource::<AuthoredTerrainStreamingState>() else {
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

    let (center, predicted_center, horizontal_speed, read_ahead_sec, max_read_ahead_cells) =
        predictive_streaming_center(anchor_pos, player_velocity, &state.spec, budget);
    let prediction_enabled = predicted_center != center;
    let predictive_radius = newengine_runtime_env::var_i32(
        "NEWENGINE_SCENE_TERRAIN_PREDICT_RADIUS",
        1.min(budget.resident_radius),
        0,
        budget.resident_radius,
    );
    let simulation_predictive_radius = predictive_radius
        .saturating_add(1)
        .min(profile.simulation.resident_radius);

    let render_prediction = prediction_enabled
        .then_some((predicted_center, predictive_radius))
        .into_iter()
        .collect::<Vec<_>>();
    let simulation_prediction = prediction_enabled
        .then_some((predicted_center, simulation_predictive_radius))
        .into_iter()
        .collect::<Vec<_>>();
    let render_desired = SceneResidencySet::desired_cells_for_focuses(
        center,
        profile.render.resident_radius,
        render_prediction.iter().copied(),
    );
    let simulation_desired = SceneResidencySet::desired_cells_for_focuses(
        center,
        profile.simulation.resident_radius,
        simulation_prediction.iter().copied(),
    );
    let predicted_render_cells = if prediction_enabled {
        SceneResidencySet::desired_cells(predicted_center, predictive_radius)
    } else {
        Vec::new()
    };

    let layered_plan = SceneLayeredStreamingPlan::build_from_desired(
        center,
        profile,
        render_desired,
        simulation_desired,
        state.loaded.keys().copied(),
        state.pending.keys().copied(),
        std::iter::empty::<TerrainChunkCoord>(),
        std::iter::empty::<TerrainChunkCoord>(),
    );
    let plan = &layered_plan.render;
    let bucket_plan = SceneBucketedCellPlan::from_desired_sets_with_prediction(
        center,
        layered_plan.render.desired.iter().copied(),
        layered_plan.simulation.desired.iter().copied(),
        predicted_render_cells.iter().copied(),
    );

    // Retire stale speculative work before polling completed jobs. A one-cell hysteresis ring
    // prevents boundary oscillation while still dropping old read-ahead searches much sooner
    // than full loaded-chunk unload hysteresis.
    let render_desired_set = layered_plan
        .render
        .desired
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let pending_keep_radius = budget
        .resident_radius
        .saturating_add(1)
        .min(budget.unload_radius);
    let to_drop_pending = state
        .pending
        .keys()
        .copied()
        .filter(|coord| {
            !render_desired_set.contains(coord)
                && coord.chebyshev_distance(center) > pending_keep_radius
        })
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

    // `loaded` and `pending` are already ordered maps. Query them directly rather than
    // cloning both key sets every frame; large streaming radii otherwise create avoidable
    // allocator traffic and duplicate tree construction on the hot path.
    let completed_ready = state
        .pending
        .iter()
        .filter_map(|(coord, pending)| pending.ticket.is_complete().then_some(*coord))
        .collect::<Vec<_>>();
    let stream_requests_ready = bucket_plan
        .cells
        .iter()
        .copied()
        .filter(|cell| cell.bucket.wants_render_residency())
        .filter(|cell| {
            !state.loaded.contains_key(&cell.coord) && !state.pending.contains_key(&cell.coord)
        })
        .collect::<Vec<_>>();

    // Placement/commit budget is intentionally independent from async request issue. The old
    // path consumed the same budget for both, which starved generation whenever completed work
    // used the burst or the wall-clock commit interval had not elapsed.
    let target_commit_budget = budget.max_commits_per_tick.max(1);
    let needs_commit_budget =
        !completed_ready.is_empty() || (thread_pool.is_none() && !stream_requests_ready.is_empty());
    let commit_budget = if needs_commit_budget {
        responsive_stream_commit_budget(target_commit_budget, &mut state)
    } else {
        0
    };

    let mut created = 0usize;
    let commit_started = Instant::now();
    let commit_budget_ms = terrain_commit_budget_ms();
    for coord in completed_ready.into_iter().take(commit_budget) {
        if created > 0 && commit_started.elapsed().as_secs_f32() * 1000.0 >= commit_budget_ms {
            break;
        }
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

    let request_budget =
        terrain_stream_request_budget(target_commit_budget, state.max_pending_jobs);
    let mut scheduled = 0usize;
    let mut preempted_pending = 0usize;
    if let Some(thread_pool) = thread_pool {
        for request in stream_requests_ready.iter().copied() {
            if scheduled >= request_budget {
                break;
            }
            if state.pending.len() >= state.max_pending_jobs
                && preempt_lower_priority_pending(&mut state, request.score)
            {
                preempted_pending = preempted_pending.saturating_add(1);
            }
            if state.pending.len() >= state.max_pending_jobs {
                continue;
            }
            if enqueue_streamed_terrain_chunk(
                &mut state,
                Some(thread_pool),
                request.coord,
                terrain_task_priority(request.bucket),
                request.score,
            ) {
                scheduled = scheduled.saturating_add(1);
            }
        }
    } else {
        // No worker backend: preserve correctness with synchronous generation, but treat it as
        // placement work and keep it inside the same commit burst/time gate.
        let remaining_commit_budget = commit_budget.saturating_sub(created);
        for request in stream_requests_ready
            .iter()
            .copied()
            .take(remaining_commit_budget)
        {
            let record = spawn_streamed_terrain_chunk(
                world,
                state.root,
                mats,
                state.material,
                &state.spec,
                &state.surface,
                state.color,
                request.coord,
                state.heightmap.as_deref(),
            );
            state.loaded.insert(request.coord, record);
            created = created.saturating_add(1);
        }
    }

    let mut removed = 0usize;
    for request in &plan.unloads {
        let coord = request.coord;
        if let Some(record) = state.loaded.remove(&coord) {
            let _ = world.despawn(record.terrain);
            removed += 1;
        }
    }

    let streaming_changed = created > 0
        || scheduled > 0
        || removed > 0
        || cancelled_pending > 0
        || preempted_pending > 0;
    let render_desired = bucket_plan
        .cells
        .iter()
        .filter(|cell| cell.bucket.wants_render_residency())
        .count();
    let predicted_desired = bucket_plan
        .cells
        .iter()
        .filter(|cell| {
            matches!(
                cell.bucket,
                newengine_scene::SceneStreamingBucket::PredictedNear
            )
        })
        .count();
    let simulation_desired = bucket_plan
        .cells
        .iter()
        .filter(|cell| cell.bucket.wants_simulation_residency())
        .count();
    let reached_render_target = state.loaded.len() >= render_desired && state.pending.is_empty();
    let diagnostics_due = removed > 0
        || cancelled_pending > 0
        || preempted_pending > 0
        || state.stream_commit_count.is_multiple_of(16)
        || reached_render_target;

    if streaming_changed && diagnostics_due {
        newengine_ulog_api::ulog::debug!(
            "game-ready terrain streaming: center=[{},{}] predicted=[{},{}] anchor={:?} follow_player={} speed_mps={:.2} read_ahead_sec={:.2} max_read_ahead_cells={} render_loaded={} render_pending={} created={} scheduled={} removed={} cancelled_pending={} preempted_pending={} commit_budget={} commit_budget_ms={:.2} request_budget={} commit_count={} render_desired={} predicted_desired={} render_unloads={} sim_desired={}",
            center.x,
            center.z,
            predicted_center.x,
            predicted_center.z,
            state.anchor,
            follow_player,
            horizontal_speed,
            read_ahead_sec,
            max_read_ahead_cells,
            state.loaded.len(),
            state.pending.len(),
            created,
            scheduled,
            removed,
            cancelled_pending,
            preempted_pending,
            commit_budget,
            commit_budget_ms,
            request_budget,
            state.stream_commit_count,
            render_desired,
            predicted_desired,
            plan.unloads.len(),
            simulation_desired,
        );
        let _ = validate_scene_objects(world, "world-environment.streaming-terrain");
    }

    world.insert_resource(state);
}
