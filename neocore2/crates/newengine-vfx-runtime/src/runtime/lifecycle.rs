pub fn install_vfx_runtime(world: &mut World) {
    if world.resource::<VfxEffectLibrary>().is_none() {
        world.insert_resource(VfxEffectLibrary::default());
    }
    if world.resource::<VfxRuntimeState>().is_none() {
        world.insert_resource(VfxRuntimeState::default());
    }
    if world.resource::<VfxSurfaceResponseLibrary>().is_none() {
        world.insert_resource(VfxSurfaceResponseLibrary::default());
    }
    if world.resource::<VfxSpawnQueue>().is_none() {
        world.insert_resource(VfxSpawnQueue::default());
    }
    if world.resource::<VfxGpuParticleBridge>().is_none() {
        world.insert_resource(VfxGpuParticleBridge::default());
    }
    if world.resource::<VfxGpuParticleLedger>().is_none() {
        world.insert_resource(VfxGpuParticleLedger::default());
    }
    if world.resource::<VfxGpuTextureRegistry>().is_none() {
        world.insert_resource(VfxGpuTextureRegistry::default());
    }
}

/// Normalizes and stores semantic VFX work for deterministic processing at the VFX frame stage.
pub fn queue_vfx(world: &mut World, request: VfxSpawnRequestV1) -> Result<bool, String> {
    let request = request.normalized()?;
    install_vfx_runtime(world);
    {
        let definition = world
            .resource::<VfxEffectLibrary>()
            .and_then(|library| library.get(request.effect.as_str()))
            .ok_or_else(|| format!("unknown VFX effect '{}'", request.effect.as_str()))?;
        definition.validate()?;
    }
    Ok(world
        .resource_mut::<VfxSpawnQueue>()
        .expect("VFX spawn queue installed")
        .push_normalized(request))
}

/// Processes stored semantic requests in stable FIFO order. Malformed/unknown requests cannot
/// enter the queue; an execution failure is isolated, counted and does not block later work.
pub fn process_queued_vfx(world: &mut World) -> VfxQueueProcessReport {
    install_vfx_runtime(world);
    let mut report = VfxQueueProcessReport::default();
    let mut live = None;
    loop {
        let request = world
            .resource_mut::<VfxSpawnQueue>()
            .and_then(VfxSpawnQueue::pop_front);
        let Some(request) = request else {
            break;
        };
        report.processed = report.processed.saturating_add(1);
        if live.is_none() {
            live = Some(live_counts(world));
        }
        match spawn_vfx_normalized(
            world,
            request,
            live.as_mut().expect("live counts initialized"),
        ) {
            Ok(Some(_)) => report.spawned = report.spawned.saturating_add(1),
            Ok(None) => report.budget_rejected = report.budget_rejected.saturating_add(1),
            Err(_) => {
                report.failed = report.failed.saturating_add(1);
                if let Some(queue) = world.resource_mut::<VfxSpawnQueue>() {
                    queue.note_execution_drop();
                }
            }
        }
    }
    report
}

pub fn spawn_vfx(
    world: &mut World,
    request: VfxSpawnRequestV1,
) -> Result<Option<VfxInstanceId>, String> {
    let request = request.normalized()?;
    install_vfx_runtime(world);
    let mut live = live_counts(world);
    spawn_vfx_normalized(world, request, &mut live)
}

fn spawn_vfx_normalized(
    world: &mut World,
    request: VfxSpawnRequestV1,
    live: &mut LiveCounts,
) -> Result<Option<VfxInstanceId>, String> {
    let definition = world
        .resource::<VfxEffectLibrary>()
        .and_then(|library| library.get(request.effect.as_str()))
        .cloned()
        .ok_or_else(|| format!("unknown VFX effect '{}'", request.effect.as_str()))?;
    definition.validate()?;
    let surface_response = world
        .resource::<VfxSurfaceResponseLibrary>()
        .map(|library| library.resolve(request.surface.as_deref()))
        .unwrap_or_default();

    let budget = world
        .resource::<VfxRuntimeState>()
        .map(|state| state.budget.sanitized())
        .unwrap_or_default();
    if live.instances >= budget.max_active_instances {
        note_dropped_instance(world);
        return Ok(None);
    }

    let instance_id = world
        .resource_mut::<VfxRuntimeState>()
        .expect("VFX runtime state installed")
        .allocate_instance_id();
    let owner_stable_id = request.owner.map(|owner| owner.stable_id);
    let requested_lifetime = request.lifetime_seconds;
    let root_lifetime = requested_lifetime
        .unwrap_or_else(|| definition.max_lifetime_seconds())
        .max(0.001);
    let root = world.spawn();
    let _ = world.insert(
        root,
        Name(format!(
            "Vfx/{}/{:016x}/{}",
            request.effect.as_str(),
            owner_stable_id.unwrap_or(0),
            request.correlation_id
        )),
    );
    let _ = world.insert(
        root,
        VfxInstanceRoot {
            id: instance_id,
            owner_stable_id,
            correlation_id: request.correlation_id,
            remaining_seconds: root_lifetime,
        },
    );

    let spawn_context = VfxSpawnContext {
        instance_id,
        owner_stable_id,
        request: &request,
        requested_lifetime,
        surface_response,
    };
    let mut spawned_layers = 0u32;
    for (layer_index, layer) in definition.layers.iter().enumerate() {
        let wanted_layers = layer.estimated_layers();
        let wanted_particles = layer.estimated_particles();
        let wanted_lights = layer.estimated_lights();
        let wanted_decals = layer.estimated_decals();
        let wanted_trails = u32::from(matches!(
            layer,
            VfxLayerDefinition::Pulse {
                kind: VfxLayerKind::Trail,
                ..
            }
        ));
        let allowed = live.layers.saturating_add(wanted_layers) <= budget.max_active_layers
            && live.particles.saturating_add(wanted_particles) <= budget.max_particle_estimate
            && live.lights.saturating_add(wanted_lights) <= budget.max_transient_lights
            && live.decals.saturating_add(wanted_decals) <= budget.max_decals
            && live.trails.saturating_add(wanted_trails) <= budget.max_trails;
        if !allowed {
            note_dropped_layers(world, u64::from(wanted_layers));
            continue;
        }
        let count = spawn_layer(world, spawn_context, layer_index as u64, layer);
        if count < wanted_layers {
            note_dropped_layers(world, u64::from(wanted_layers - count));
        }
        spawned_layers = spawned_layers.saturating_add(count);
        live.layers = live.layers.saturating_add(count);
        live.particles = live.particles.saturating_add(wanted_particles.min(count));
        live.lights = live.lights.saturating_add(wanted_lights.min(count));
        live.decals = live.decals.saturating_add(wanted_decals.min(count));
        live.trails = live.trails.saturating_add(wanted_trails.min(count));
    }

    if spawned_layers == 0 {
        let _ = world.despawn(root);
        note_dropped_instance(world);
        return Ok(None);
    }
    live.instances = live.instances.saturating_add(1);
    if let Some(state) = world.resource_mut::<VfxRuntimeState>() {
        state.spawned_instances = state.spawned_instances.saturating_add(1);
    }
    Ok(Some(instance_id))
}

fn step_vfx_internal(world: &mut World, dt: f32) {
    let dt = if dt.is_finite() {
        dt.clamp(0.0, 0.1)
    } else {
        0.0
    };
    if dt <= 0.0 {
        return;
    }

    if let Some(ledger) = world.resource_mut::<VfxGpuParticleLedger>() {
        ledger.step(dt);
    }

    let layers = world
        .query::<VfxLayerRuntime>()
        .map(|(entity, runtime)| (entity, *runtime))
        .collect::<Vec<_>>();
    for (entity, mut runtime) in layers {
        if runtime.kind == VfxLayerKind::Tracer && runtime.tracer_mode == VfxTracerMode::SingleFrame
        {
            if runtime.tracer_updates_remaining == 0 {
                let _ = world.despawn(entity);
                continue;
            }
            runtime.tracer_updates_remaining = runtime.tracer_updates_remaining.saturating_sub(1);
            let _ = world.insert(entity, runtime);
            continue;
        }

        runtime.age_seconds += dt;
        if runtime.age_seconds + 1.0e-6 >= runtime.lifetime_seconds {
            let _ = world.despawn(entity);
            continue;
        }

        if runtime.kind == VfxLayerKind::Tracer {
            let speed = runtime.velocity.length();
            let direction = runtime.velocity.normalize_or_zero();
            if speed <= 1.0e-6 || direction.length_squared() <= 1.0e-8 {
                let _ = world.despawn(entity);
                continue;
            }
            let remaining_distance = (runtime.max_distance - runtime.traveled).max(0.0);
            let advance = (speed * dt).min(remaining_distance);
            if let Some(transform) = world.get_mut::<Transform>(entity) {
                transform.position += direction * advance;
            }
            runtime.traveled += advance;
            if runtime.traveled + 1.0e-4 >= runtime.max_distance {
                let _ = world.despawn(entity);
                continue;
            }
        } else {
            runtime.velocity += runtime.acceleration * dt;
            if let Some(transform) = world.get_mut::<Transform>(entity) {
                transform.position += runtime.velocity * dt;
                transform.scale = (runtime.base_scale
                    + runtime.growth_per_second * runtime.age_seconds)
                    .max(Vec3::splat(0.0001));
            }
        }

        let life_fraction = (runtime.age_seconds / runtime.lifetime_seconds).clamp(0.0, 1.0);
        let fade = fade_multiplier(life_fraction, runtime.fade_start_fraction);
        if let Some(primitive) = world.get_mut::<Primitive>(entity) {
            primitive.color = runtime.start_color;
            primitive.color[3] *= fade;
        }
        if let Some(light) = world.get_mut::<PointLight>(entity) {
            light.intensity = runtime.initial_light_intensity * fade;
        }
        let _ = world.insert(entity, runtime);
    }

    let transient_lights = world
        .query::<VfxTransientLightRuntime>()
        .map(|(entity, runtime)| (entity, *runtime))
        .collect::<Vec<_>>();
    for (entity, mut runtime) in transient_lights {
        runtime.age_seconds += dt;
        if runtime.age_seconds + 1.0e-6 >= runtime.lifetime_seconds {
            let _ = world.despawn(entity);
            continue;
        }
        let life_fraction =
            (runtime.age_seconds / runtime.lifetime_seconds.max(0.001)).clamp(0.0, 1.0);
        let fade = fade_multiplier(life_fraction, runtime.fade_start_fraction);
        if let Some(light) = world.get_mut::<PointLight>(entity) {
            light.intensity = runtime.initial_intensity * fade;
        }
        let _ = world.insert(entity, runtime);
    }

    let roots = world
        .query::<VfxInstanceRoot>()
        .map(|(entity, root)| (entity, *root))
        .collect::<Vec<_>>();
    for (entity, mut root) in roots {
        root.remaining_seconds -= dt;
        if root.remaining_seconds <= 0.0 {
            let id = root.id;
            let _ = world.despawn(entity);
            despawn_layers_for_instance(world, id);
        } else {
            let _ = world.insert(entity, root);
        }
    }
}

pub fn pre_update_vfx(world: &mut World) {
    install_vfx_runtime(world);
    if let Some(state) = world.resource_mut::<VfxRuntimeState>() {
        state.frame_index = state.frame_index.wrapping_add(1);
        state.stage = VfxRuntimeStage::PreUpdate;
    }
    let _ = process_queued_vfx(world);
}

pub fn update_vfx(world: &mut World, dt: f32) {
    install_vfx_runtime(world);
    if let Some(state) = world.resource_mut::<VfxRuntimeState>() {
        state.stage = VfxRuntimeStage::Update;
    }
    step_vfx_internal(world, dt);
}

pub fn update_after_pre_render_vfx(world: &mut World) {
    install_vfx_runtime(world);
    if let Some(state) = world.resource_mut::<VfxRuntimeState>() {
        state.stage = VfxRuntimeStage::AfterPreRender;
    }
}

pub fn finish_vfx_frame(world: &mut World) {
    if let Some(state) = world.resource_mut::<VfxRuntimeState>() {
        state.stage = VfxRuntimeStage::Idle;
    }
}

/// Compatibility single-call frame driver. Hosts with explicit render staging should call
/// `pre_update_vfx`, `update_vfx`, `update_after_pre_render_vfx`, then `finish_vfx_frame`.
pub fn step_vfx(world: &mut World, dt: f32) {
    pre_update_vfx(world);
    update_vfx(world, dt);
    update_after_pre_render_vfx(world);
    finish_vfx_frame(world);
}

pub fn clamp_vfx_tracers_to_hit(
    world: &mut World,
    owner_stable_id: u64,
    correlation_id: u64,
    point: Vec3,
) {
    if !point.is_finite() {
        return;
    }
    if let Some(queue) = world.resource_mut::<VfxSpawnQueue>() {
        let _ = queue.clamp_pending_to_point(owner_stable_id, correlation_id, point);
    }
    let tracers = world
        .query::<VfxLayerRuntime>()
        .filter_map(|(entity, runtime)| {
            (runtime.kind == VfxLayerKind::Tracer
                && runtime.owner_stable_id == Some(owner_stable_id)
                && runtime.correlation_id == correlation_id)
                .then_some((entity, *runtime))
        })
        .collect::<Vec<_>>();
    for (entity, mut runtime) in tracers {
        let hit_distance = (point - runtime.origin).length();
        if hit_distance.is_finite() {
            runtime.max_distance = runtime.max_distance.min(hit_distance.max(0.0));
            if runtime.tracer_mode == VfxTracerMode::SingleFrame {
                let direction = runtime.velocity.normalize_or_zero();
                let visible_length = runtime.base_scale.z.min(runtime.max_distance.max(0.0002));
                runtime.base_scale.z = visible_length;
                if let Some(transform) = world.get_mut::<Transform>(entity) {
                    transform.position = runtime.origin + direction * (visible_length * 0.5);
                    transform.scale.z = visible_length;
                }
            }
            let _ = world.insert(entity, runtime);
        }
    }
}

pub fn stop_vfx_instance(world: &mut World, id: VfxInstanceId) {
    let roots = world
        .query::<VfxInstanceRoot>()
        .filter_map(|(entity, root)| (root.id == id).then_some(entity))
        .collect::<Vec<_>>();
    for entity in roots {
        let _ = world.despawn(entity);
    }
    despawn_layers_for_instance(world, id);
}

pub fn vfx_runtime_stats(world: &World) -> VfxRuntimeStatsV1 {
    let live = live_counts(world);
    let state = world
        .resource::<VfxRuntimeState>()
        .copied()
        .unwrap_or_default();
    VfxRuntimeStatsV1 {
        active_instances: live.instances,
        active_layers: live.layers,
        transient_lights: live.lights,
        decals: live.decals,
        trails: live.trails,
        particle_estimate: live.particles,
        spawned_instances: state.spawned_instances,
        pending_requests: world
            .resource::<VfxSpawnQueue>()
            .map(|queue| queue.len().min(u32::MAX as usize) as u32)
            .unwrap_or(0),
        dropped_requests: world
            .resource::<VfxSpawnQueue>()
            .map(VfxSpawnQueue::dropped_requests)
            .unwrap_or(0),
        dropped_instances: state.dropped_instances,
        dropped_layers: state.dropped_layers,
    }
}
