use super::*;
use newengine_engine_runtime::gameplay::PreparedRenderMesh;

pub(super) fn terrain_surface_layers(spec: &GameReadyTerrainSpec) -> TerrainMaterialLayers {
    TerrainMaterialLayers {
        forest_base_texture: spec.surface.forest_base_texture.clone(),
        sand_base_texture: spec.surface.sand_base_texture.clone(),
        rock_base_texture: spec.surface.rock_base_texture.clone(),
        patch_scale: spec.surface.patch_scale,
        blend_softness: spec.surface.blend_softness,
    }
}

#[inline]
pub(super) fn terrain_graph_for_chunk(
    spec: &GameReadyTerrainSpec,
    coord: TerrainChunkCoord,
) -> NoiseGraph2D {
    let center = coord.center(spec.size_x, spec.size_z);

    // GameFirst terrain is intentionally not a mountain generator. The profile
    // produces traversable land: shallow depressions, low ridges, dry creek-like
    // cuts, and broad biome patches. Vertical relief stays modest; visual
    // diversity comes from surface masks, foliage density, and local terrain
    // character rather than endless hills.
    let mut terrain_graph = NoiseGraph2D::new(NoiseDomain2D {
        seed: spec.seed,
        frequency: 0.018,
        offset_x: 0.0,
        offset_z: 0.0,
        warp: Some(DomainWarp2D {
            seed_offset: 0x91e7_70ad,
            frequency: 0.045,
            strength: 3.8,
            octaves: 3,
        }),
    })
    .with_layer(
        NoiseLayer2D::new(NoiseAlgorithm::Value)
            .combine(NoiseCombineMode::Replace)
            .frequency(1.0)
            .amplitude(0.34),
    )
    .with_layer(
        NoiseLayer2D::new(NoiseAlgorithm::Cellular)
            .seed_offset(spec.seed ^ 0x6c8e_9cf5)
            .frequency(0.42)
            .amplitude(0.18)
            .shape(NoiseShape::SmoothStep {
                edge0: -0.72,
                edge1: 0.42,
            })
            .combine(NoiseCombineMode::Add),
    )
    .with_layer(
        NoiseLayer2D::new(NoiseAlgorithm::Billow)
            .seed_offset(spec.seed ^ 0x2f4d_31aa)
            .frequency(2.75)
            .amplitude(0.08)
            .combine(NoiseCombineMode::Add),
    )
    .with_layer(
        NoiseLayer2D::new(NoiseAlgorithm::Ridged)
            .seed_offset(spec.seed ^ spec.generator.ridged_seed_xor)
            .frequency(spec.generator.ridged_frequency)
            .amplitude(spec.generator.ridged_amplitude)
            .shape(NoiseShape::SmoothStep {
                edge0: spec.generator.ridged_shape_edge0,
                edge1: spec.generator.ridged_shape_edge1,
            })
            .combine(NoiseCombineMode::Add),
    )
    .with_layer(
        NoiseLayer2D::new(NoiseAlgorithm::Veins)
            .seed_offset(spec.seed ^ spec.generator.veins_seed_xor)
            .frequency(spec.generator.veins_frequency)
            .amplitude(-spec.generator.veins_amplitude.abs())
            .shape(NoiseShape::SmoothStep {
                edge0: 0.12,
                edge1: 0.95,
            })
            .combine(NoiseCombineMode::Add),
    )
    .with_remap(NoiseRemap {
        input_min: -0.55,
        input_max: 0.65,
        output_min: -0.45,
        output_max: 0.68,
        clamp: true,
    });

    terrain_graph.domain.offset_x += center.x * terrain_graph.domain.frequency;
    terrain_graph.domain.offset_z += center.z * terrain_graph.domain.frequency;
    terrain_graph
}

pub(super) fn generate_terrain_for_chunk(
    spec: &GameReadyTerrainSpec,
    coord: TerrainChunkCoord,
    color: [f32; 4],
    heightmap: Option<&TerrainHeightmapRuntime>,
) -> GeneratedTerrainChunk {
    let center = coord.center(spec.size_x, spec.size_z);
    let descriptor = TerrainHeightfieldDescriptor {
        cells_x: spec.cells_x,
        cells_z: spec.cells_z,
        size_x: spec.size_x,
        size_z: spec.size_z,
        base_height: spec.base_height,
        height_scale: spec.height_scale,
        graph: terrain_graph_for_chunk(spec, coord),
        smoothing_passes: spec.generator.smoothing_passes,
        smoothing_strength: spec.generator.smoothing_strength,
    };
    let terrain = if let Some(heightmap) = heightmap {
        ProceduralTerrain::generate_descriptor_with_world_height_modifier(
            descriptor,
            color,
            heightmap.revision_key(),
            |local_x, local_z, procedural_height| {
                heightmap.apply_world_height(
                    center.x + local_x,
                    center.z + local_z,
                    procedural_height,
                )
            },
        )
    } else {
        ProceduralTerrain::generate_descriptor(descriptor, color)
    };
    // Build the renderable primitive mesh on the generation lane as well.
    // Previously every committed streamed chunk did this conversion inside the
    // render draw-list extraction path; in debug/profile-dev this cost dominated
    // the frame and made the FPS overlay report ~3 FPS while the Vulkan backend
    // itself was idle.
    let mesh = Arc::new(terrain.heightfield.to_primitive_mesh());
    GeneratedTerrainChunk { terrain, mesh }
}

pub(super) fn spawn_generated_terrain_chunk(
    world: &mut newengine_ecs::World,
    root: EntityId,
    mats: &MaterialRegistry,
    material: MaterialId,
    spec: &GameReadyTerrainSpec,
    surface: &TerrainMaterialLayers,
    color: [f32; 4],
    coord: TerrainChunkCoord,
    generated: GeneratedTerrainChunk,
) -> TerrainChunkRecord {
    let center = coord.center(spec.size_x, spec.size_z);
    let terrain = generated.terrain;
    let sampler = TerrainSurfaceSampler {
        origin: center,
        heightfield: Arc::clone(&terrain.heightfield),
    };
    let bounds = Bounds::from_local_aabb(terrain.heightfield.local_bounds());
    let entity = spawn_named(world, format!("Terrain/Chunk[{:+},{:+}]", coord.x, coord.z));
    let _ = newengine_transform::set_parent(world, entity, Some(root));
    let _ = world.insert(
        entity,
        Transform {
            position: center,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    let _ = world.insert(entity, terrain);
    let _ = world.insert(
        entity,
        PreparedRenderMesh {
            mesh: generated.mesh,
        },
    );
    let terrain_half_extents = bounds.local_aabb.half_extents();
    let _ = world.insert(entity, bounds);
    newengine_engine_runtime::gameplay::attach_scene_object_core(
        world,
        entity,
        center,
        terrain_half_extents,
    );
    let _ = world.insert(entity, surface.clone());
    let _ = apply_exact_material(world, mats, entity, material, material, color);

    TerrainChunkRecord {
        terrain: entity,
        sampler,
    }
}

pub(super) fn spawn_streamed_terrain_chunk(
    world: &mut newengine_ecs::World,
    root: EntityId,
    mats: &MaterialRegistry,
    material: MaterialId,
    spec: &GameReadyTerrainSpec,
    surface: &TerrainMaterialLayers,
    color: [f32; 4],
    coord: TerrainChunkCoord,
    heightmap: Option<&TerrainHeightmapRuntime>,
) -> TerrainChunkRecord {
    let generated = generate_terrain_for_chunk(spec, coord, color, heightmap);
    spawn_generated_terrain_chunk(
        world, root, mats, material, spec, surface, color, coord, generated,
    )
}

pub(super) fn enqueue_streamed_terrain_chunk(
    state: &mut GameReadyTerrainStreamingState,
    thread_pool: Option<&ThreadPoolHandle>,
    coord: TerrainChunkCoord,
) -> bool {
    if state.pending.contains_key(&coord) || state.loaded.contains_key(&coord) {
        return false;
    }
    if state.pending.len() >= state.max_pending_jobs.max(1) {
        return false;
    }

    let Some(thread_pool) = thread_pool else {
        return false;
    };

    let spec = state.spec.clone();
    let color = state.color;
    let heightmap = state.heightmap.clone();
    let result = Arc::new(Mutex::new(None));
    let result_for_job = Arc::clone(&result);
    let ticket = thread_pool.submit_request(
        TaskRequest::new("game-ready.terrain.chunk.render-packet")
            .with_source("scene.streaming.terrain")
            .with_owner("engine.render")
            .with_category("terrain.render-packet")
            .with_lane(TaskLane::RenderPrep)
            .with_priority(TaskPriority::Interactive)
            .with_dependency_group(format!("terrain.chunk.{}.{}.renderprep", coord.x, coord.z))
            .with_task_domain(task_domain::ENGINE_RENDER_PREP)
            .with_task_pass(task_pass::TERRAIN_RENDER_PACKET),
        move || {
            let generated = generate_terrain_for_chunk(&spec, coord, color, heightmap.as_deref());
            if let Ok(mut slot) = result_for_job.lock() {
                *slot = Some(generated);
            }
        },
    );
    state
        .pending
        .insert(coord, PendingTerrainChunk { result, ticket });
    true
}
