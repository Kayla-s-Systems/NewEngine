fn live_counts(world: &World) -> LiveCounts {
    let instances = world
        .query::<VfxInstanceRoot>()
        .count()
        .min(u32::MAX as usize) as u32;
    let mut counts = LiveCounts {
        instances,
        ..LiveCounts::default()
    };
    for (entity, layer) in world.query::<VfxLayerRuntime>() {
        counts.layers = counts.layers.saturating_add(1);
        if world.get::<PointLight>(entity).is_some() {
            counts.lights = counts.lights.saturating_add(1);
        }
        match layer.kind {
            VfxLayerKind::ImpactDecal => counts.decals = counts.decals.saturating_add(1),
            VfxLayerKind::Trail => counts.trails = counts.trails.saturating_add(1),
            _ => counts.particles = counts.particles.saturating_add(1),
        }
    }
    let persistent_decals = world
        .query::<VfxPersistentDecal>()
        .count()
        .min(u32::MAX as usize) as u32;
    counts.layers = counts.layers.saturating_add(persistent_decals);
    counts.decals = counts.decals.saturating_add(persistent_decals);
    counts.lights = counts.lights.saturating_add(
        world
            .query::<VfxTransientLightRuntime>()
            .count()
            .min(u32::MAX as usize) as u32,
    );
    if let Some(ledger) = world.resource::<VfxGpuParticleLedger>() {
        for layer in ledger.layers() {
            counts.layers = counts.layers.saturating_add(layer.particle_count);
            counts.particles = counts.particles.saturating_add(layer.particle_count);
        }
    }
    counts
}

fn note_dropped_instance(world: &mut World) {
    if let Some(state) = world.resource_mut::<VfxRuntimeState>() {
        state.dropped_instances = state.dropped_instances.saturating_add(1);
    }
}

fn note_dropped_layers(world: &mut World, count: u64) {
    if let Some(state) = world.resource_mut::<VfxRuntimeState>() {
        state.dropped_layers = state.dropped_layers.saturating_add(count);
    }
}

fn despawn_layers_for_instance(world: &mut World, id: VfxInstanceId) {
    let layers = world
        .query::<VfxLayerRuntime>()
        .filter_map(|(entity, layer)| (layer.instance_id == id).then_some(entity))
        .collect::<Vec<_>>();
    for entity in layers {
        let _ = world.despawn(entity);
    }
    let transient_lights = world
        .query::<VfxTransientLightRuntime>()
        .filter_map(|(entity, runtime)| (runtime.instance_id == id).then_some(entity))
        .collect::<Vec<_>>();
    for entity in transient_lights {
        let _ = world.despawn(entity);
    }
    let removed_gpu_particles = world
        .resource_mut::<VfxGpuParticleLedger>()
        .map(|ledger| ledger.remove_instance(id))
        .unwrap_or(0);
    if removed_gpu_particles > 0 {
        if let Some(bridge) = world.resource::<VfxGpuParticleBridge>() {
            let _ = bridge.enqueue_kill_instance(id.0);
        }
    }
}
