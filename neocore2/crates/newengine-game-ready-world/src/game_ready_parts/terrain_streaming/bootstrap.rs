use super::*;

fn launch_blocking_warm_radius(target_radius: i32) -> i32 {
    const DEFAULT_LAUNCH_WARM_RADIUS: i32 = 1;
    let requested = crate::env_config::var_i32(
        "NEWENGINE_SCENE_TERRAIN_LAUNCH_WARM_RADIUS",
        DEFAULT_LAUNCH_WARM_RADIUS,
        i32::MIN,
        i32::MAX,
    );
    requested.clamp(0, target_radius.max(0))
}

#[inline]
pub(crate) fn spawn_procedural_terrain(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    root: EntityId,
    material: MaterialId,
    spec: &GameReadyTerrainSpec,
    color: [f32; 4],
    initial_center: TerrainChunkCoord,
) -> EntityId {
    newengine_ulog_api::ulog::info!(
        "game-ready: terrain generator id='{}' seed={} cells={}x{} chunk_size={}x{} streaming={} radius={} unload_radius={} surface_mode='multi_textured' layer_count={} heightmap_enabled={} surface_layers=[forest='{}', sand='{}', rock='{}']",
        spec.generator.id,
        spec.seed,
        spec.cells_x,
        spec.cells_z,
        spec.size_x,
        spec.size_z,
        spec.streaming.enabled,
        spec.streaming.chunk_radius,
        spec.streaming.unload_radius,
        spec.surface.layers.len(),
        spec.heightmap.enabled,
        spec.surface.forest_base_texture,
        spec.surface.sand_base_texture,
        spec.surface.rock_base_texture,
    );

    if !spec.surface.layers.is_empty() {
        let summary = spec
            .surface
            .layers
            .iter()
            .map(|layer| {
                format!(
                    "{}:texture='{}':weight={:.3}:uv_scale={:.3}",
                    layer.role, layer.base_texture, layer.weight, layer.uv_scale
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        newengine_ulog_api::ulog::info!(
            "game-ready terrain surface package: declarative_layers={} projection='3-channel terrain shader' layers=[{}]",
            spec.surface.layers.len(),
            summary
        );
    }

    let surface = terrain_surface_layers(spec);
    let heightmap = load_terrain_heightmap(spec);
    let streaming_anchor = spawn_named(world, "Scene/Terrain/StreamingAnchor");
    let _ = set_parent(world, streaming_anchor, Some(root));
    newengine_engine_runtime::gameplay::attach_scene_element_core(
        world,
        streaming_anchor,
        newengine_engine_runtime::gameplay::SceneEntityRole::TerrainStreamingAnchor,
        "Scene/Terrain/StreamingAnchor",
        Vec3::ZERO,
        Vec3::splat(1.0),
    );
    let _ = world.insert(
        streaming_anchor,
        newengine_engine_runtime::gameplay::SceneAnchorFollow::player(),
    );

    let origin = initial_center;
    let record = spawn_streamed_terrain_chunk(
        world,
        root,
        mats,
        material,
        spec,
        &surface,
        color,
        origin,
        heightmap.as_deref(),
    );
    let terrain_entity = record.terrain;
    newengine_ulog_api::ulog::info!(
        "game-ready terrain anchor: terrain_entity={:?} streaming_anchor={:?} parent={:?} policy='terrain streaming target is an ordinary ECS entity anchor'",
        terrain_entity,
        streaming_anchor,
        root
    );

    if spec.streaming.enabled {
        let budget = SceneStreamingBudget {
            resident_radius: spec.streaming.chunk_radius,
            unload_radius: spec.streaming.unload_radius,
            max_commits_per_tick: spec.streaming.max_chunks_per_frame,
        }
        .sanitized();
        let mut state = GameReadyTerrainStreamingState {
            root,
            anchor: streaming_anchor,
            material,
            color,
            spec: spec.clone(),
            surface,
            heightmap: heightmap.clone(),
            chunk_radius: budget.resident_radius,
            unload_radius: budget.unload_radius,
            max_chunks_per_frame: budget.max_commits_per_tick,
            max_pending_jobs: budget.max_commits_per_tick.saturating_mul(4).max(4),
            stream_commit_count: 0,
            last_stream_commit_at: None,
            loaded: std::collections::BTreeMap::new(),
            pending: std::collections::BTreeMap::new(),
        };
        state.loaded.insert(origin, record);

        // Keep the native window responsive during first world handoff. The full
        // streaming target radius remains active, but only a small launch ring is
        // generated synchronously before the public launch gate opens. Remaining
        // chunks are admitted by tick_game_ready_streaming_terrain() through the
        // normal frame-budgeted streaming path.
        let launch_warm_radius = launch_blocking_warm_radius(state.chunk_radius);
        let mut warmed = 1usize;
        for coord in SceneResidencySet::desired_cells(origin, launch_warm_radius) {
            if state.loaded.contains_key(&coord) {
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
            warmed = warmed.saturating_add(1);
        }
        if warmed > 1 || launch_warm_radius != state.chunk_radius {
            let target_chunks = SceneResidencySet::desired_cells(origin, state.chunk_radius).len();
            newengine_ulog_api::ulog::info!(
                "game-ready terrain streaming: initial resident chunks warmed center=[{},{}] launch_radius={} target_radius={} chunks={} target_chunks={} policy='responsive startup; remaining chunks stream after launch'",
                origin.x,
                origin.z,
                launch_warm_radius,
                state.chunk_radius,
                warmed,
                target_chunks
            );
        }

        world.insert_resource(state);
    }

    terrain_entity
}
