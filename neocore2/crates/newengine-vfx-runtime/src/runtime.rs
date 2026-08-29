use newengine_ecs::{EntityId, World};
use newengine_lighting::PointLight;
use newengine_math::{Quat, Vec3};
use newengine_model_domain_api::{
    MeshCullPolicy, MeshDepthPolicy, MeshRenderOptions, MeshRenderRole, MeshShadowPolicy,
    MeshSortPolicy,
};
use newengine_primitives::Primitive;
use newengine_scene::components::Name;
use newengine_transform::Transform;
use newengine_vfx_api::{
    VfxGpuParticleBridge, VfxGpuParticleKind, VfxGpuParticleSpawnV1, VfxRuntimeStatsV1,
    VfxSpawnRequestV1,
};

use crate::{
    VfxAlignment, VfxEffectLibrary, VfxGpuLayerRuntime, VfxGpuParticleLedger, VfxInstanceId,
    VfxInstanceRoot, VfxLayerDefinition, VfxLayerKind, VfxLayerRuntime, VfxLightDefinition,
    VfxQueueProcessReport, VfxRenderRole, VfxRuntimeStage, VfxRuntimeState, VfxSpawnQueue,
    VfxSurfaceResponse, VfxSurfaceResponseLibrary,
};

#[derive(Clone, Copy)]
struct VfxSpawnContext<'a> {
    instance_id: VfxInstanceId,
    owner_stable_id: Option<u64>,
    request: &'a VfxSpawnRequestV1,
    requested_lifetime: Option<f32>,
    surface_response: VfxSurfaceResponse,
}

#[derive(Clone, Copy, Debug, Default)]
struct LiveCounts {
    instances: u32,
    layers: u32,
    lights: u32,
    decals: u32,
    trails: u32,
    particles: u32,
}

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
    loop {
        let request = world
            .resource_mut::<VfxSpawnQueue>()
            .and_then(VfxSpawnQueue::pop_front);
        let Some(request) = request else {
            break;
        };
        report.processed = report.processed.saturating_add(1);
        match spawn_vfx(world, request) {
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

    let mut live = live_counts(world);
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

fn spawn_layer(
    world: &mut World,
    context: VfxSpawnContext<'_>,
    layer_index: u64,
    layer: &VfxLayerDefinition,
) -> u32 {
    let VfxSpawnContext {
        instance_id,
        owner_stable_id,
        request,
        requested_lifetime,
        surface_response,
    } = context;
    let position = vec3_from_array(request.position);
    let direction = vec3_from_array(request.direction).normalize_or_zero();
    let normal = vec3_from_array(request.normal).normalize_or_zero();
    match layer {
        VfxLayerDefinition::Pulse {
            kind,
            primitive,
            role,
            alignment,
            offset_along_direction,
            offset_along_normal,
            scale,
            growth_per_second,
            color,
            lifetime_seconds,
            fade_start_fraction,
            light,
        } => {
            if *kind == VfxLayerKind::Smoke && *role == VfxRenderRole::Transparent {
                let lifetime = requested_lifetime.unwrap_or(*lifetime_seconds).max(0.001);
                let color = surface_color(*kind, *color, surface_response);
                let layer_position =
                    position + direction * *offset_along_direction + normal * *offset_along_normal;
                let base_scale = *scale * request.scale;
                let growth = *growth_per_second * request.scale;
                let spawn = VfxGpuParticleSpawnV1 {
                    instance_id: instance_id.0,
                    kind: VfxGpuParticleKind::Smoke,
                    position: [layer_position.x, layer_position.y, layer_position.z],
                    velocity: request.velocity,
                    acceleration: [0.0; 3],
                    size: [
                        (base_scale.x * 2.0).max(0.0001),
                        (base_scale.z * 2.0).max(0.0001),
                    ],
                    growth_per_second: [growth.x * 2.0, growth.z * 2.0],
                    color,
                    lifetime_seconds: lifetime,
                    fade_start_fraction: fade_start_fraction.clamp(0.0, 0.999),
                };
                let admitted = world
                    .resource::<VfxGpuParticleBridge>()
                    .is_some_and(|bridge| bridge.enqueue_spawn(spawn));
                if admitted {
                    world
                        .resource_mut::<VfxGpuParticleLedger>()
                        .expect("VFX GPU particle ledger installed")
                        .push(VfxGpuLayerRuntime {
                            instance_id,
                            kind: *kind,
                            particle_count: 1,
                            remaining_seconds: lifetime,
                        });
                    return 1;
                }
                return 0;
            }

            let lifetime = requested_lifetime.unwrap_or(*lifetime_seconds).max(0.001);
            let color = surface_color(*kind, *color, surface_response);
            let entity = world.spawn();
            let layer_position =
                position + direction * *offset_along_direction + normal * *offset_along_normal;
            let rotation = alignment_rotation(*alignment, direction, normal);
            let base_scale = *scale * request.scale;
            let _ = world.insert(
                entity,
                Name(format!(
                    "VfxLayer/{:?}/{}/{}",
                    kind, request.correlation_id, layer_index
                )),
            );
            let _ = world.insert(
                entity,
                Transform {
                    position: layer_position,
                    rotation,
                    scale: base_scale,
                },
            );
            let _ = world.insert(
                entity,
                Primitive {
                    id: *primitive,
                    color,
                },
            );
            let _ = world.insert(entity, render_options(*role));
            let initial_light_intensity = light
                .map(|definition| install_light(world, entity, definition, request.intensity))
                .unwrap_or(0.0);
            let _ = world.insert(
                entity,
                VfxLayerRuntime {
                    instance_id,
                    owner_stable_id,
                    correlation_id: request.correlation_id,
                    kind: *kind,
                    origin: position,
                    velocity: vec3_from_array(request.velocity),
                    acceleration: Vec3::ZERO,
                    age_seconds: 0.0,
                    lifetime_seconds: lifetime,
                    base_scale,
                    growth_per_second: *growth_per_second * request.scale,
                    start_color: color,
                    fade_start_fraction: fade_start_fraction.clamp(0.0, 0.999),
                    traveled: 0.0,
                    max_distance: 0.0,
                    initial_light_intensity,
                },
            );
            1
        }
        VfxLayerDefinition::Tracer {
            primitive,
            color,
            half_length,
            radius,
            speed,
            max_lifetime_seconds,
        } => {
            if request.max_distance <= 0.0 {
                return 0;
            }
            let entity = world.spawn();
            let half_length = *half_length * request.scale;
            let radius = *radius * request.scale;
            let lifetime = requested_lifetime
                .unwrap_or(
                    (request.max_distance / speed.max(0.001) + 0.06).min(*max_lifetime_seconds),
                )
                .max(0.001);
            let rotation = Quat::from_rotation_arc(Vec3::Z, direction).normalize_or_identity();
            let color = surface_color(VfxLayerKind::Tracer, *color, surface_response);
            let _ = world.insert(
                entity,
                Name(format!(
                    "VfxLayer/Tracer/{}/{}",
                    request.correlation_id, layer_index
                )),
            );
            let _ = world.insert(
                entity,
                Transform {
                    position: position + direction * half_length,
                    rotation,
                    scale: Vec3::new(radius, radius, half_length * 2.0),
                },
            );
            let _ = world.insert(
                entity,
                Primitive {
                    id: *primitive,
                    color,
                },
            );
            let _ = world.insert(entity, render_options(VfxRenderRole::Transparent));
            let _ = world.insert(
                entity,
                VfxLayerRuntime {
                    instance_id,
                    owner_stable_id,
                    correlation_id: request.correlation_id,
                    kind: VfxLayerKind::Tracer,
                    origin: position,
                    velocity: direction * *speed,
                    acceleration: Vec3::ZERO,
                    age_seconds: 0.0,
                    lifetime_seconds: lifetime,
                    base_scale: Vec3::new(radius, radius, half_length * 2.0),
                    growth_per_second: Vec3::ZERO,
                    start_color: color,
                    fade_start_fraction: 0.55,
                    traveled: half_length,
                    max_distance: request.max_distance,
                    initial_light_intensity: 0.0,
                },
            );
            1
        }
        VfxLayerDefinition::Burst {
            kind,
            primitive,
            role,
            count,
            scale,
            color,
            speed_min,
            speed_max,
            acceleration,
            lifetime_seconds,
            fade_start_fraction,
        } => {
            if *kind == VfxLayerKind::Spark && *role == VfxRenderRole::Transparent {
                let base_scale = *scale * request.scale;
                let lifetime = requested_lifetime.unwrap_or(*lifetime_seconds).max(0.001);
                let resolved_color = surface_color(*kind, *color, surface_response);
                let mut admitted = 0u32;
                for particle_index in 0..u64::from(*count) {
                    let seed = mix64(
                        request.seed
                            ^ request.correlation_id.rotate_left(17)
                            ^ layer_index.rotate_left(31)
                            ^ particle_index,
                    );
                    let random = random_unit_vector(seed);
                    let hemisphere = if random.dot(normal) < 0.0 {
                        -random
                    } else {
                        random
                    };
                    let travel_direction = (hemisphere + normal * 0.40).normalize_or_zero();
                    let speed_t = unit_float(mix64(seed ^ 0x9e37_79b9_7f4a_7c15));
                    let speed = speed_min + (speed_max - speed_min).max(0.0) * speed_t;
                    let velocity = travel_direction * speed + vec3_from_array(request.velocity);
                    let spawn = VfxGpuParticleSpawnV1 {
                        instance_id: instance_id.0,
                        kind: VfxGpuParticleKind::Spark,
                        position: [
                            position.x + normal.x * 0.012,
                            position.y + normal.y * 0.012,
                            position.z + normal.z * 0.012,
                        ],
                        velocity: [velocity.x, velocity.y, velocity.z],
                        acceleration: [acceleration.x, acceleration.y, acceleration.z],
                        size: [
                            (base_scale.x * 2.0).max(0.0001),
                            (base_scale.z * 2.0).max(0.0001),
                        ],
                        growth_per_second: [0.0; 2],
                        color: resolved_color,
                        lifetime_seconds: lifetime,
                        fade_start_fraction: fade_start_fraction.clamp(0.0, 0.999),
                    };
                    if world
                        .resource::<VfxGpuParticleBridge>()
                        .is_some_and(|bridge| bridge.enqueue_spawn(spawn))
                    {
                        admitted = admitted.saturating_add(1);
                    }
                }
                if admitted > 0 {
                    world
                        .resource_mut::<VfxGpuParticleLedger>()
                        .expect("VFX GPU particle ledger installed")
                        .push(VfxGpuLayerRuntime {
                            instance_id,
                            kind: *kind,
                            particle_count: admitted,
                            remaining_seconds: lifetime,
                        });
                }
                return admitted;
            }

            let mut spawned = 0u32;
            for particle_index in 0..u64::from(*count) {
                let seed = mix64(
                    request.seed
                        ^ request.correlation_id.rotate_left(17)
                        ^ layer_index.rotate_left(31)
                        ^ particle_index,
                );
                let random = random_unit_vector(seed);
                let hemisphere = if random.dot(normal) < 0.0 {
                    -random
                } else {
                    random
                };
                let travel_direction = (hemisphere + normal * 0.40).normalize_or_zero();
                let speed_t = unit_float(mix64(seed ^ 0x9e37_79b9_7f4a_7c15));
                let speed = speed_min + (speed_max - speed_min).max(0.0) * speed_t;
                let velocity = travel_direction * speed + vec3_from_array(request.velocity);
                let entity = world.spawn();
                let rotation =
                    Quat::from_rotation_arc(Vec3::Z, travel_direction).normalize_or_identity();
                let color = surface_color(*kind, *color, surface_response);
                let base_scale = *scale * request.scale;
                let lifetime = requested_lifetime.unwrap_or(*lifetime_seconds).max(0.001);
                let _ = world.insert(
                    entity,
                    Name(format!(
                        "VfxLayer/{:?}/{}/{}-{}",
                        kind, request.correlation_id, layer_index, particle_index
                    )),
                );
                let _ = world.insert(
                    entity,
                    Transform {
                        position: position + normal * 0.012,
                        rotation,
                        scale: base_scale,
                    },
                );
                let _ = world.insert(
                    entity,
                    Primitive {
                        id: *primitive,
                        color,
                    },
                );
                let _ = world.insert(entity, render_options(*role));
                let _ = world.insert(
                    entity,
                    VfxLayerRuntime {
                        instance_id,
                        owner_stable_id,
                        correlation_id: request.correlation_id,
                        kind: *kind,
                        origin: position,
                        velocity,
                        acceleration: *acceleration,
                        age_seconds: 0.0,
                        lifetime_seconds: lifetime,
                        base_scale,
                        growth_per_second: Vec3::ZERO,
                        start_color: color,
                        fade_start_fraction: fade_start_fraction.clamp(0.0, 0.999),
                        traveled: 0.0,
                        max_distance: 0.0,
                        initial_light_intensity: 0.0,
                    },
                );
                spawned += 1;
            }
            spawned
        }
        VfxLayerDefinition::Decal {
            primitive,
            scale,
            color,
            normal_offset,
            lifetime_seconds,
            fade_start_fraction,
        } => {
            let entity = world.spawn();
            let color = surface_color(VfxLayerKind::ImpactDecal, *color, surface_response);
            let base_scale = *scale * request.scale;
            let lifetime = requested_lifetime.unwrap_or(*lifetime_seconds).max(0.001);
            let _ = world.insert(
                entity,
                Name(format!(
                    "VfxLayer/ImpactDecal/{}/{}",
                    request.correlation_id, layer_index
                )),
            );
            let _ = world.insert(
                entity,
                Transform {
                    position: position + normal * *normal_offset,
                    rotation: Quat::from_rotation_arc(Vec3::Y, normal).normalize_or_identity(),
                    scale: base_scale,
                },
            );
            let _ = world.insert(
                entity,
                Primitive {
                    id: *primitive,
                    color,
                },
            );
            let _ = world.insert(entity, render_options(VfxRenderRole::Decal));
            let _ = world.insert(
                entity,
                VfxLayerRuntime {
                    instance_id,
                    owner_stable_id,
                    correlation_id: request.correlation_id,
                    kind: VfxLayerKind::ImpactDecal,
                    origin: position,
                    velocity: Vec3::ZERO,
                    acceleration: Vec3::ZERO,
                    age_seconds: 0.0,
                    lifetime_seconds: lifetime,
                    base_scale,
                    growth_per_second: Vec3::ZERO,
                    start_color: color,
                    fade_start_fraction: fade_start_fraction.clamp(0.0, 0.999),
                    traveled: 0.0,
                    max_distance: 0.0,
                    initial_light_intensity: 0.0,
                },
            );
            1
        }
    }
}

fn render_options(role: VfxRenderRole) -> MeshRenderOptions {
    let mut options = MeshRenderOptions::world_opaque();
    options.role = match role {
        VfxRenderRole::Transparent => MeshRenderRole::WorldTransparent,
        VfxRenderRole::Decal => MeshRenderRole::Decal,
    };
    options.depth_policy = MeshDepthPolicy::ReadOnly;
    options.shadow_policy = MeshShadowPolicy::None;
    options.cull_policy = MeshCullPolicy::None;
    options.sort_policy = MeshSortPolicy::Transparent;
    options
}

fn alignment_rotation(alignment: VfxAlignment, direction: Vec3, normal: Vec3) -> Quat {
    match alignment {
        VfxAlignment::None => Quat::IDENTITY,
        VfxAlignment::DirectionY => {
            Quat::from_rotation_arc(Vec3::Y, direction).normalize_or_identity()
        }
        VfxAlignment::DirectionZ => {
            Quat::from_rotation_arc(Vec3::Z, direction).normalize_or_identity()
        }
        VfxAlignment::NormalY => Quat::from_rotation_arc(Vec3::Y, normal).normalize_or_identity(),
    }
}

fn install_light(
    world: &mut World,
    entity: EntityId,
    definition: VfxLightDefinition,
    request_intensity: f32,
) -> f32 {
    let intensity = definition.intensity * request_intensity;
    let _ = world.insert(
        entity,
        PointLight {
            color: definition.color,
            intensity,
            range: definition.range,
        },
    );
    intensity
}

fn surface_color(kind: VfxLayerKind, base: [f32; 4], response: VfxSurfaceResponse) -> [f32; 4] {
    let mut color = base;
    match kind {
        VfxLayerKind::Spark => {
            if let Some(rgb) = response.spark_color {
                color[..3].copy_from_slice(&rgb);
            }
            color[3] *= if response.spark_alpha_scale.is_finite() {
                response.spark_alpha_scale.max(0.0)
            } else {
                1.0
            };
        }
        VfxLayerKind::Smoke => {
            if let Some(rgb) = response.smoke_color {
                color[..3].copy_from_slice(&rgb);
            }
        }
        VfxLayerKind::ImpactDecal => {
            if let Some(rgb) = response.decal_color {
                color[..3].copy_from_slice(&rgb);
            }
        }
        _ => {}
    }
    color
}

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

#[inline]
fn fade_multiplier(life_fraction: f32, fade_start_fraction: f32) -> f32 {
    if life_fraction <= fade_start_fraction {
        1.0
    } else {
        let span = (1.0 - fade_start_fraction).max(1.0e-5);
        (1.0 - (life_fraction - fade_start_fraction) / span).clamp(0.0, 1.0)
    }
}

#[inline]
fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[inline]
fn unit_float(value: u64) -> f32 {
    ((value >> 40) as u32 as f32) / ((1u32 << 24) as f32)
}

fn random_unit_vector(seed: u64) -> Vec3 {
    let x = unit_float(mix64(seed ^ 0x243f_6a88_85a3_08d3)) * 2.0 - 1.0;
    let y = unit_float(mix64(seed ^ 0x1319_8a2e_0370_7344)) * 2.0 - 1.0;
    let z = unit_float(mix64(seed ^ 0xa409_3822_299f_31d0)) * 2.0 - 1.0;
    Vec3::new(x, y, z).normalize_or_zero()
}

#[inline]
fn vec3_from_array(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}
