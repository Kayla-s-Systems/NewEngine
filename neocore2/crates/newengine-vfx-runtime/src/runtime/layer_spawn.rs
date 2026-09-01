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
            texture_slot,
            billboard,
            offset_along_direction,
            offset_along_normal,
            scale,
            growth_per_second,
            color,
            lifetime_seconds,
            fade_start_fraction,
            fade_in_fraction,
            drag_per_second,
            depth_softness_m,
            rotation_radians,
            rotation_random_radians,
            spin_radians_per_second,
            light,
        } => {
            if *role == VfxRenderRole::Transparent
                && (gpu_particle_kind(*kind).is_some() || *texture_slot > 0)
            {
                let lifetime = requested_lifetime.unwrap_or(*lifetime_seconds).max(0.001);
                let color = surface_color(*kind, *color, surface_response);
                let layer_position =
                    position + direction * *offset_along_direction + normal * *offset_along_normal;
                let base_scale = *scale * request.scale;
                let growth = *growth_per_second * request.scale;
                let layer_seed = mix64(
                    request.seed
                        ^ request.correlation_id.rotate_left(17)
                        ^ layer_index.rotate_left(31),
                );
                let rotation = *rotation_radians
                    + signed_unit_float(mix64(layer_seed ^ 0x6a09_e667_f3bc_c909))
                        * *rotation_random_radians;
                let spawn = VfxGpuParticleSpawnV1 {
                    instance_id: instance_id.0,
                    kind: gpu_particle_kind(*kind).unwrap_or(VfxGpuParticleKind::Debris),
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
                    fade_in_fraction: fade_in_fraction.clamp(0.0, 0.999),
                    drag_per_second: (*drag_per_second).max(0.0),
                    depth_softness_m: (*depth_softness_m).max(0.0),
                    rotation_radians: rotation,
                    angular_velocity_radians_per_second: *spin_radians_per_second,
                    texture_slot: *texture_slot,
                    billboard: *billboard,
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
                    // GPU billboards replace only visible pulse geometry. Preserve authored
                    // illumination through a light-only transient entity so first-person muzzle
                    // containment does not regress scene lighting.
                    if let Some(definition) = *light {
                        let light_entity = world.spawn();
                        let _ = world.insert(
                            light_entity,
                            Name(format!(
                                "VfxLight/{:?}/{}/{}",
                                kind, request.correlation_id, layer_index
                            )),
                        );
                        let _ = world.insert(
                            light_entity,
                            Transform {
                                position: layer_position,
                                rotation: Quat::IDENTITY,
                                scale: Vec3::ONE,
                            },
                        );
                        let initial_intensity =
                            install_light(world, light_entity, definition, request.intensity);
                        let _ = world.insert(
                            light_entity,
                            VfxTransientLightRuntime {
                                instance_id,
                                age_seconds: 0.0,
                                lifetime_seconds: lifetime,
                                fade_start_fraction: fade_start_fraction.clamp(0.0, 0.999),
                                initial_intensity,
                            },
                        );
                    }
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
                    tracer_mode: VfxTracerMode::Swept,
                    tracer_updates_remaining: 0,
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
            mode,
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
            let authored_length = (half_length * 2.0).max(0.0002);
            let visible_length = if *mode == VfxTracerMode::SingleFrame {
                authored_length.min(request.max_distance.max(0.0002))
            } else {
                authored_length
            };
            let lifetime = if *mode == VfxTracerMode::SingleFrame {
                // Lifetime is not used to decide the single-frame retirement. Keep it valid for
                // instance budgeting/root lifetime while `tracer_updates_remaining` owns visibility.
                requested_lifetime
                    .unwrap_or(*max_lifetime_seconds)
                    .max(0.001)
            } else {
                requested_lifetime
                    .unwrap_or(
                        (request.max_distance / speed.max(0.001) + 0.06).min(*max_lifetime_seconds),
                    )
                    .max(0.001)
            };
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
                    position: position + direction * (visible_length * 0.5),
                    rotation,
                    scale: Vec3::new(radius, radius, visible_length),
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
                    tracer_mode: *mode,
                    tracer_updates_remaining: u8::from(*mode == VfxTracerMode::SingleFrame),
                    origin: position,
                    velocity: direction * *speed,
                    acceleration: Vec3::ZERO,
                    age_seconds: 0.0,
                    lifetime_seconds: lifetime,
                    base_scale: Vec3::new(radius, radius, visible_length),
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
            texture_slot,
            billboard,
            emission_axis,
            count,
            scale,
            color,
            speed_min,
            speed_max,
            cone_angle_degrees,
            size_variance,
            lifetime_variance,
            acceleration,
            drag_per_second,
            depth_softness_m,
            rotation_random_radians,
            spin_radians_per_second,
            spin_variance,
            lifetime_seconds,
            fade_start_fraction,
            fade_in_fraction,
        } => {
            if *role == VfxRenderRole::Transparent
                && (gpu_particle_kind(*kind).is_some() || *texture_slot > 0)
            {
                let base_scale = *scale * request.scale;
                let base_lifetime = requested_lifetime.unwrap_or(*lifetime_seconds).max(0.001);
                let emission_axis = resolve_emission_axis(*emission_axis, direction, normal);
                let resolved_color = surface_color(*kind, *color, surface_response);
                let mut admitted = 0u32;
                let mut max_admitted_lifetime = 0.0_f32;
                for particle_index in 0..u64::from(*count) {
                    let seed = mix64(
                        request.seed
                            ^ request.correlation_id.rotate_left(17)
                            ^ layer_index.rotate_left(31)
                            ^ particle_index,
                    );
                    let travel_direction = random_direction_in_cone(
                        emission_axis,
                        cone_angle_degrees.to_radians(),
                        seed,
                    );
                    let speed_t = unit_float(mix64(seed ^ 0x9e37_79b9_7f4a_7c15));
                    let speed = speed_min + (speed_max - speed_min).max(0.0) * speed_t;
                    let velocity = travel_direction * speed + vec3_from_array(request.velocity);
                    let size_factor = (1.0
                        + signed_unit_float(mix64(seed ^ 0xbb67_ae85_84ca_a73b)) * *size_variance)
                        .max(0.05);
                    let lifetime_factor = (1.0
                        + signed_unit_float(mix64(seed ^ 0x3c6e_f372_fe94_f82b))
                            * *lifetime_variance)
                        .max(0.05);
                    let lifetime = (base_lifetime * lifetime_factor).max(0.001);
                    let rotation = signed_unit_float(mix64(seed ^ 0xa54f_f53a_5f1d_36f1))
                        * *rotation_random_radians;
                    let spin = *spin_radians_per_second
                        + signed_unit_float(mix64(seed ^ 0x510e_527f_ade6_82d1)) * *spin_variance;
                    let spawn = VfxGpuParticleSpawnV1 {
                        instance_id: instance_id.0,
                        kind: gpu_particle_kind(*kind).unwrap_or(VfxGpuParticleKind::Debris),
                        position: [
                            position.x + emission_axis.x * 0.012,
                            position.y + emission_axis.y * 0.012,
                            position.z + emission_axis.z * 0.012,
                        ],
                        velocity: [velocity.x, velocity.y, velocity.z],
                        acceleration: [acceleration.x, acceleration.y, acceleration.z],
                        size: [
                            (base_scale.x * 2.0 * size_factor).max(0.0001),
                            (base_scale.z * 2.0 * size_factor).max(0.0001),
                        ],
                        growth_per_second: [0.0; 2],
                        color: resolved_color,
                        lifetime_seconds: lifetime,
                        fade_start_fraction: fade_start_fraction.clamp(0.0, 0.999),
                        fade_in_fraction: fade_in_fraction.clamp(0.0, 0.999),
                        drag_per_second: (*drag_per_second).max(0.0),
                        depth_softness_m: (*depth_softness_m).max(0.0),
                        rotation_radians: rotation,
                        angular_velocity_radians_per_second: spin,
                        texture_slot: *texture_slot,
                        billboard: *billboard,
                    };
                    if world
                        .resource::<VfxGpuParticleBridge>()
                        .is_some_and(|bridge| bridge.enqueue_spawn(spawn))
                    {
                        admitted = admitted.saturating_add(1);
                        max_admitted_lifetime = max_admitted_lifetime.max(lifetime);
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
                            remaining_seconds: max_admitted_lifetime.max(base_lifetime),
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
                let emission_axis = resolve_emission_axis(*emission_axis, direction, normal);
                let travel_direction =
                    random_direction_in_cone(emission_axis, cone_angle_degrees.to_radians(), seed);
                let speed_t = unit_float(mix64(seed ^ 0x9e37_79b9_7f4a_7c15));
                let speed = speed_min + (speed_max - speed_min).max(0.0) * speed_t;
                let velocity = travel_direction * speed + vec3_from_array(request.velocity);
                let entity = world.spawn();
                let rotation =
                    Quat::from_rotation_arc(Vec3::Z, travel_direction).normalize_or_identity();
                let color = surface_color(*kind, *color, surface_response);
                let size_factor = (1.0
                    + signed_unit_float(mix64(seed ^ 0xbb67_ae85_84ca_a73b)) * *size_variance)
                    .max(0.05);
                let base_scale = *scale * request.scale * size_factor;
                let lifetime_factor = (1.0
                    + signed_unit_float(mix64(seed ^ 0x3c6e_f372_fe94_f82b)) * *lifetime_variance)
                    .max(0.05);
                let lifetime =
                    (requested_lifetime.unwrap_or(*lifetime_seconds) * lifetime_factor).max(0.001);
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
                        tracer_mode: VfxTracerMode::Swept,
                        tracer_updates_remaining: 0,
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
            material_ref,
            scale,
            color,
            normal_offset,
            persistent,
            lifetime_seconds,
            fade_start_fraction,
        } => {
            let entity = world.spawn();
            let color = surface_color(VfxLayerKind::ImpactDecal, *color, surface_response);
            let base_scale = *scale * request.scale;
            let lifetime =
                (!*persistent).then(|| requested_lifetime.unwrap_or(*lifetime_seconds).max(0.001));
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
            if let Some(material_ref) = material_ref.as_deref() {
                let _ = world.insert(
                    entity,
                    VfxDecalMaterialAssetRef {
                        logical_ref: material_ref.to_owned(),
                    },
                );
            }
            let _ = world.insert(entity, render_options(VfxRenderRole::Decal));
            if *persistent {
                let _ = world.insert(
                    entity,
                    VfxPersistentDecal {
                        source_instance_id: instance_id,
                        owner_stable_id,
                        correlation_id: request.correlation_id,
                    },
                );
            } else if let Some(lifetime) = lifetime {
                let _ = world.insert(
                    entity,
                    VfxLayerRuntime {
                        instance_id,
                        owner_stable_id,
                        correlation_id: request.correlation_id,
                        kind: VfxLayerKind::ImpactDecal,
                        tracer_mode: VfxTracerMode::Swept,
                        tracer_updates_remaining: 0,
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
            }
            1
        }
    }
}
