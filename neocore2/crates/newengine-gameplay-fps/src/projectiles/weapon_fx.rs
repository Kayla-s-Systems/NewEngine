#[inline]
fn weapon_fx_render_options() -> newengine_model_domain_api::MeshRenderOptions {
    let mut render = newengine_model_domain_api::MeshRenderOptions::world_opaque();
    render.shadow_policy = newengine_model_domain_api::MeshShadowPolicy::None;
    render
}

/// Spawns presentation-only firing effects from the already-resolved physical muzzle. Damage and
/// collision stay authoritative in the hitscan query path. The generic composition provides a
/// compact directional muzzle flame, hot core/light pulse and tracer; an equipped weapon may also
/// author a physical casing contract. No presentation entity participates in ballistic collision.
pub(crate) fn spawn_weapon_shot_fx(
    world: &mut World,
    owner: EntityId,
    shot_sequence: u64,
    origin: Vec3,
    direction: Vec3,
    range: f32,
) {
    let direction = direction.normalize_or_zero();
    if !origin.is_finite() || direction.length_squared() <= 1.0e-8 {
        return;
    }
    let tracer_rotation = Quat::from_rotation_arc(Vec3::Z, direction).normalize_or_identity();
    let cone_rotation = Quat::from_rotation_arc(Vec3::Y, direction).normalize_or_identity();

    // Directional flame: cone base sits close to the crown, apex extends downrange.
    let flash = world.spawn();
    let _ = world.insert(
        flash,
        Name(format!(
            "WeaponFx/MuzzleFlash/{:016x}/{shot_sequence}",
            owner.stable_u64()
        )),
    );
    let _ = world.insert(
        flash,
        Transform {
            position: origin + direction * 0.085,
            rotation: cone_rotation,
            scale: Vec3::new(0.075, 0.17, 0.075),
        },
    );
    let _ = world.insert(
        flash,
        Primitive {
            id: prim_builtins::ID_CONE,
            color: [1.0, 0.54, 0.10, 1.0],
        },
    );
    let _ = world.insert(flash, DisplayVisibility::default());
    let _ = world.insert(flash, GameplayActor);
    let _ = world.insert(flash, weapon_fx_render_options());
    let _ = world.insert(
        flash,
        WeaponShotFxRuntime {
            owner,
            shot_sequence,
            kind: WeaponShotFxKind::MuzzleFlash,
            origin,
            velocity: Vec3::ZERO,
            traveled: 0.0,
            max_distance: 0.0,
            remaining_seconds: MUZZLE_FLASH_LIFETIME_SECONDS,
        },
    );

    // Compact white-hot core also owns the light pulse. Keeping this much smaller than the old
    // sphere avoids the debug-ball silhouette while preserving a strong flash in dark scenes.
    let core = world.spawn();
    let _ = world.insert(
        core,
        Name(format!(
            "WeaponFx/MuzzleCore/{:016x}/{shot_sequence}",
            owner.stable_u64()
        )),
    );
    let _ = world.insert(
        core,
        Transform {
            position: origin + direction * 0.018,
            rotation: tracer_rotation,
            scale: Vec3::new(0.040, 0.040, 0.065),
        },
    );
    let _ = world.insert(
        core,
        Primitive {
            id: prim_builtins::ID_SPHERE_UV,
            color: [1.0, 0.86, 0.48, 1.0],
        },
    );
    let _ = world.insert(core, DisplayVisibility::default());
    let _ = world.insert(core, GameplayActor);
    let _ = world.insert(core, weapon_fx_render_options());
    let _ = world.insert(
        core,
        PointLight {
            color: [1.0, 0.58, 0.18],
            intensity: 34.0,
            range: 3.8,
        },
    );
    let _ = world.insert(
        core,
        WeaponShotFxRuntime {
            owner,
            shot_sequence,
            kind: WeaponShotFxKind::MuzzleCore,
            origin,
            velocity: Vec3::ZERO,
            traveled: 0.0,
            max_distance: 0.0,
            remaining_seconds: MUZZLE_CORE_LIFETIME_SECONDS,
        },
    );

    let max_distance = if range.is_finite() {
        range.clamp(0.1, 100_000.0)
    } else {
        120.0
    };
    let tracer = world.spawn();
    let _ = world.insert(
        tracer,
        Name(format!(
            "WeaponFx/Tracer/{:016x}/{shot_sequence}",
            owner.stable_u64()
        )),
    );
    let _ = world.insert(
        tracer,
        Transform {
            position: origin + direction * WEAPON_TRACER_HALF_LENGTH_M,
            rotation: tracer_rotation,
            scale: Vec3::new(0.004, 0.004, WEAPON_TRACER_HALF_LENGTH_M * 2.0),
        },
    );
    let _ = world.insert(
        tracer,
        Primitive {
            id: prim_builtins::ID_CUBE,
            color: [1.0, 0.72, 0.24, 1.0],
        },
    );
    let _ = world.insert(tracer, DisplayVisibility::default());
    let _ = world.insert(tracer, GameplayActor);
    let _ = world.insert(tracer, weapon_fx_render_options());
    let _ = world.insert(
        tracer,
        WeaponShotFxRuntime {
            owner,
            shot_sequence,
            kind: WeaponShotFxKind::Tracer,
            origin,
            velocity: direction * WEAPON_TRACER_SPEED_MPS,
            traveled: WEAPON_TRACER_HALF_LENGTH_M,
            max_distance,
            remaining_seconds: (max_distance / WEAPON_TRACER_SPEED_MPS + 0.06).clamp(0.06, 0.8),
        },
    );

    // Physical casing behavior belongs to the equipped weapon definition. Weapons without an
    // authored casing contract simply do not schedule a casing entity.
    let casing_contract = world
        .get::<EquippedWeaponBinding>(owner)
        .copied()
        .and_then(|binding| {
            world
                .resource::<ItemCatalog>()?
                .get(binding.item)
                .map(|definition| {
                    (
                        binding.item.raw(),
                        definition.weapon_casing.clone().sanitized(),
                    )
                })
        })
        .filter(|(_, casing)| casing.enabled());
    if let Some((weapon_item_id, casing)) = casing_contract {
        let pending = world.spawn();
        let _ = world.insert(
            pending,
            Name(format!(
                "WeaponFx/ShellEjectionPending/{:016x}/{shot_sequence}",
                owner.stable_u64()
            )),
        );
        let _ = world.insert(
            pending,
            PendingWeaponShellEjection {
                owner,
                shot_sequence,
                weapon_item_id,
                shot_origin: origin,
                shot_direction: direction,
                remaining_seconds: casing.ejection_delay_seconds,
            },
        );
    }
}

fn spawn_persistent_shell_casing(
    world: &mut World,
    owner: EntityId,
    shot_sequence: u64,
    weapon_item_id: u64,
    fallback_origin: Vec3,
    fallback_direction: Vec3,
) -> Option<EntityId> {
    let casing_definition = world
        .resource::<ItemCatalog>()?
        .get(ItemId(weapon_item_id))?
        .weapon_casing
        .clone()
        .sanitized();
    if !casing_definition.enabled() {
        return None;
    }
    let (origin, direction) = world
        .get::<EquippedWeaponMuzzle>(owner)
        .copied()
        .map(|muzzle| (muzzle.position, muzzle.forward.normalize_or_zero()))
        .filter(|(position, forward)| position.is_finite() && forward.length_squared() > 1.0e-8)
        .unwrap_or((fallback_origin, fallback_direction.normalize_or_zero()));
    if !origin.is_finite() || direction.length_squared() <= 1.0e-8 {
        return None;
    }

    let mut right = direction.cross(Vec3::Y).normalize_or_zero();
    if right.length_squared() <= 1.0e-8 {
        right = Vec3::X;
    }
    let up = right.cross(direction).normalize_or_zero();
    let jitter = (((shot_sequence
        .wrapping_mul(1_103_515_245)
        .wrapping_add(12_345)
        >> 8)
        & 0xffff) as f32
        / 65_535.0)
        - 0.5;
    let local_vector = |value: [f32; 3]| right * value[0] + up * value[1] + direction * value[2];
    let casing_origin = origin + local_vector(casing_definition.origin_local);
    let velocity_local = [
        casing_definition.velocity_local[0] + jitter * casing_definition.velocity_jitter[0],
        casing_definition.velocity_local[1] + jitter * casing_definition.velocity_jitter[1],
        casing_definition.velocity_local[2] + jitter * casing_definition.velocity_jitter[2],
    ];
    let casing_velocity = local_vector(velocity_local);
    let casing_axis = local_vector(casing_definition.axis_local).normalize_or_zero();
    let casing_axis = if casing_axis.length_squared() > 1.0e-8 {
        casing_axis
    } else {
        right
    };
    let casing_rotation = Quat::from_rotation_arc(Vec3::Z, casing_axis).normalize_or_identity();
    let variant_count = casing_definition
        .variants
        .len()
        .min(u16::MAX as usize)
        .max(1);
    let variant = (shot_sequence % variant_count as u64) as u16;
    let casing = world.spawn();
    let _ = world.insert(
        casing,
        Name(format!(
            "WeaponFx/ShellCasing/{:016x}/{shot_sequence}",
            owner.stable_u64()
        )),
    );
    let _ = world.insert(
        casing,
        Transform {
            position: casing_origin,
            rotation: casing_rotation,
            scale: Vec3::ONE,
        },
    );
    // Model-backed casings use an invisible staging root. The world presentation provider admits
    // the authored model/material hierarchy atomically, so no generic brass-colored cube leaks in.
    let _ = world.insert(casing, DisplayVisibility::default());
    let _ = world.insert(casing, GameplayActor);
    let _ = world.insert(casing, PhysicsSurface::default());

    let mut body = PhysicsBodyDesc::dynamic_solid(CollisionShapeDesc::Box {
        half_extents: casing_definition.half_extents,
    });
    body.material.friction = casing_definition.friction;
    body.material.restitution = casing_definition.restitution;
    body.material.density = casing_definition.density;
    let _ = world.insert(casing, body);
    let _ = world.insert(casing, body.to_bounds());
    let _ = world.insert(casing, Velocity(casing_velocity));
    let angular_local = [
        casing_definition.angular_velocity[0]
            + jitter * casing_definition.angular_velocity_jitter[0],
        casing_definition.angular_velocity[1]
            + jitter * casing_definition.angular_velocity_jitter[1],
        casing_definition.angular_velocity[2]
            + jitter * casing_definition.angular_velocity_jitter[2],
    ];
    let _ = world.insert(casing, AngularVelocity(local_vector(angular_local)));
    let _ = world.insert(
        casing,
        WeaponShellCasing::new(owner.stable_u64(), shot_sequence, weapon_item_id, variant),
    );
    play_equipped_weapon_audio(world, owner, WeaponAudioAction::ShellEject);
    newengine_ulog_api::ulog::info!(
        "weapon casing ejected entity={} owner={} shot={} weapon_item={:016x} variant={} delay_ms={:.3} collider_half_extents={:?} physics='dynamic' persistence='world' visual='authored-definition'",
        casing.stable_u64(),
        owner.stable_u64(),
        shot_sequence,
        weapon_item_id,
        variant,
        casing_definition.ejection_delay_seconds * 1000.0,
        casing_definition.half_extents,
    );
    Some(casing)
}

/// Narrows an already spawned tracer to the authoritative hitscan impact. The tracer continues
/// travelling visually, but it can no longer pass through the wall/target that the shot hit.
pub(crate) fn clamp_weapon_shot_fx_to_hit(
    world: &mut World,
    owner: EntityId,
    shot_sequence: u64,
    point: Vec3,
) {
    if !point.is_finite() {
        return;
    }
    let effects = world
        .query::<WeaponShotFxRuntime>()
        .filter_map(|(entity, runtime)| {
            (runtime.owner == owner
                && runtime.shot_sequence == shot_sequence
                && runtime.kind == WeaponShotFxKind::Tracer)
                .then_some((entity, *runtime))
        })
        .collect::<Vec<_>>();
    for (entity, mut runtime) in effects {
        let hit_distance = (point - runtime.origin).length();
        if hit_distance.is_finite() {
            runtime.max_distance = runtime.max_distance.min(hit_distance.max(0.0));
            let _ = world.insert(entity, runtime);
        }
    }
}

pub(crate) fn step_weapon_shot_fx(world: &mut World, dt: f32) {
    let dt = finite_or(dt, 0.0).clamp(0.0, 0.1);
    if dt <= 0.0 {
        return;
    }

    let pending_ejections = world
        .query::<PendingWeaponShellEjection>()
        .map(|(entity, pending)| (entity, *pending))
        .collect::<Vec<_>>();
    for (entity, mut pending) in pending_ejections {
        pending.remaining_seconds -= dt;
        if pending.remaining_seconds <= 0.0 {
            let _ = spawn_persistent_shell_casing(
                world,
                pending.owner,
                pending.shot_sequence,
                pending.weapon_item_id,
                pending.shot_origin,
                pending.shot_direction,
            );
            let _ = world.despawn(entity);
        } else {
            let _ = world.insert(entity, pending);
        }
    }

    let effects = world
        .query::<WeaponShotFxRuntime>()
        .map(|(entity, runtime)| (entity, *runtime))
        .collect::<Vec<_>>();
    for (entity, mut runtime) in effects {
        runtime.remaining_seconds = (runtime.remaining_seconds - dt).max(0.0);
        let mut expire = runtime.remaining_seconds <= 0.0;
        match runtime.kind {
            WeaponShotFxKind::Tracer if !expire => {
                let speed = runtime.velocity.length();
                let direction = runtime.velocity.normalize_or_zero();
                if speed <= 1.0e-6 || direction.length_squared() <= 1.0e-8 {
                    expire = true;
                } else {
                    let remaining_distance = (runtime.max_distance - runtime.traveled).max(0.0);
                    let advance = (speed * dt).min(remaining_distance);
                    if let Some(transform) = world.get_mut::<Transform>(entity) {
                        transform.position += direction * advance;
                    }
                    runtime.traveled += advance;
                    if runtime.traveled + 1.0e-4 >= runtime.max_distance {
                        expire = true;
                    }
                }
            }
            _ => {}
        }
        if expire {
            let _ = world.despawn(entity);
        } else {
            let _ = world.insert(entity, runtime);
        }
    }
}
