/// Publishes a semantic weapon-shot effect from the already-resolved physical muzzle.
/// Damage/collision remain authoritative in the hitscan path; transient visual composition,
/// budgets and lifetime are owned by `newengine-vfx-runtime`. Physical shell casings remain here
/// because they are persistent world objects with authored rigid-body behavior.
pub fn spawn_weapon_shot_fx(
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
    let max_distance = if range.is_finite() {
        range.clamp(0.1, 100_000.0)
    } else {
        120.0
    };
    let request = newengine_vfx_api::VfxSpawnRequestV1 {
        effect: newengine_vfx_api::VfxEffectRef::new(
            newengine_vfx_runtime::VFX_WEAPON_SHOT_DEFAULT,
        ),
        owner: Some(newengine_vfx_api::EntityHandle::new(owner.stable_u64())),
        correlation_id: shot_sequence,
        position: vec3_array(origin),
        direction: vec3_array(direction),
        max_distance,
        seed: owner.stable_u64() ^ shot_sequence.rotate_left(23),
        tags: vec!["weapon".to_owned(), "shot".to_owned()],
        ..Default::default()
    };
    if let Err(error) = newengine_vfx_runtime::spawn_vfx(world, request) {
        newengine_ulog_api::ulog::warn!(
            "weapon VFX spawn rejected owner={} shot={} err='{}'",
            owner.stable_u64(),
            shot_sequence,
            error
        );
    }

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

/// Narrows the semantic tracer to the authoritative hitscan impact without creating an impact
/// composition. Kept as a compatibility facade for callers that only know the hit point.
pub fn clamp_weapon_shot_fx_to_hit(
    world: &mut World,
    owner: EntityId,
    shot_sequence: u64,
    point: Vec3,
) {
    newengine_vfx_runtime::clamp_vfx_tracers_to_hit(
        world,
        owner.stable_u64(),
        shot_sequence,
        point,
    );
}

/// Resolves the complete weapon impact presentation: tracer termination plus surface-aware
/// sparks/smoke/decal composition. The physics query remains authoritative for point/normal/entity.
pub fn resolve_weapon_shot_hit_fx(
    world: &mut World,
    owner: EntityId,
    shot_sequence: u64,
    point: Vec3,
    normal: Vec3,
    target: Option<EntityId>,
) {
    if !point.is_finite() {
        return;
    }
    clamp_weapon_shot_fx_to_hit(world, owner, shot_sequence, point);
    let normal = if normal.is_finite() && normal.length_squared() > 1.0e-8 {
        normal.normalize_or_zero()
    } else {
        Vec3::Y
    };
    let surface = target
        .and_then(|entity| world.get::<PhysicsSurface>(entity))
        .map(|surface| surface.id.clone())
        .or_else(|| Some("surface.default".to_owned()));
    let request = newengine_vfx_api::VfxSpawnRequestV1 {
        effect: newengine_vfx_api::VfxEffectRef::new(
            newengine_vfx_runtime::VFX_WEAPON_IMPACT_DEFAULT,
        ),
        owner: Some(newengine_vfx_api::EntityHandle::new(owner.stable_u64())),
        correlation_id: shot_sequence,
        position: vec3_array(point),
        direction: vec3_array(-normal),
        normal: vec3_array(normal),
        seed: owner.stable_u64()
            ^ shot_sequence.rotate_left(23)
            ^ point.x.to_bits() as u64
            ^ (point.y.to_bits() as u64).rotate_left(11)
            ^ (point.z.to_bits() as u64).rotate_left(37),
        surface,
        tags: vec!["weapon".to_owned(), "impact".to_owned()],
        ..Default::default()
    };
    if let Err(error) = newengine_vfx_runtime::spawn_vfx(world, request) {
        newengine_ulog_api::ulog::warn!(
            "weapon impact VFX rejected owner={} shot={} err='{}'",
            owner.stable_u64(),
            shot_sequence,
            error
        );
    }
}

pub fn step_weapon_shot_fx(world: &mut World, dt: f32) {
    let dt = finite_or(dt, 0.0).clamp(0.0, 0.1);
    if dt <= 0.0 {
        return;
    }

    newengine_vfx_runtime::step_vfx(world, dt);

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
}

#[inline]
fn vec3_array(value: Vec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}
