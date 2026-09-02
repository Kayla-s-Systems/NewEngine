fn fallback_weapon_socket(position: Vec3, forward: Vec3) -> Option<WeaponSocketPose> {
    let forward = forward.normalize_or_zero();
    if !position.is_finite() || forward.length_squared() <= 1.0e-8 {
        return None;
    }
    let rotation = Quat::from_rotation_arc(Vec3::Z, forward).normalize_or_identity();
    WeaponSocketPose::stationary(position, rotation)
}
fn spawn_weapon_segment_effect(
    world: &mut World,
    owner: EntityId,
    effect: String,
    shot_sequence: u64,
    bounce_count: u8,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
    tag: &str,
) {
    let direction = direction.normalize_or_zero();
    if !origin.is_finite() || direction.length_squared() <= 1.0e-8 || max_distance <= 0.0 {
        return;
    }
    let effect_owner = equipped_weapon_entity(world, owner).unwrap_or(owner);
    let correlation_id = weapon_segment_correlation_id(shot_sequence, bounce_count);
    let request = newengine_vfx_api::VfxSpawnRequestV1 {
        effect: newengine_vfx_api::VfxEffectRef::new(effect),
        owner: Some(newengine_vfx_api::EntityHandle::new(
            effect_owner.stable_u64(),
        )),
        correlation_id,
        position: vec3_array(origin),
        direction: vec3_array(direction),
        max_distance: max_distance.clamp(0.05, 1_000.0),
        seed: effect_owner.stable_u64()
            ^ correlation_id.rotate_left(23)
            ^ u64::from(bounce_count).rotate_left(41),
        tags: vec!["weapon".to_owned(), tag.to_owned()],
        ..Default::default()
    };
    if let Err(error) = newengine_vfx_runtime::spawn_vfx(world, request) {
        newengine_ulog_api::ulog::warn!(
            "project weapon segment VFX rejected owner={} shot={} bounce={} tag='{}' err='{}'",
            owner.stable_u64(),
            shot_sequence,
            bounce_count,
            tag,
            error,
        );
    }
}

fn spawn_weapon_ricochet_fx(
    world: &mut World,
    owner: EntityId,
    shot_sequence: u64,
    bounce_count: u8,
    point: Vec3,
    normal: Vec3,
    reflected: Vec3,
    remaining: f32,
) {
    let Some(vfx) = equipped_weapon_vfx_definition(world, owner) else {
        return;
    };
    let effect_owner = equipped_weapon_entity(world, owner).unwrap_or(owner);
    let correlation_id = weapon_segment_correlation_id(shot_sequence, bounce_count);
    if let Some(effect) = vfx.ricochet {
        let request = newengine_vfx_api::VfxSpawnRequestV1 {
            effect: newengine_vfx_api::VfxEffectRef::new(effect),
            owner: Some(newengine_vfx_api::EntityHandle::new(
                effect_owner.stable_u64(),
            )),
            correlation_id,
            position: vec3_array(point + normal.normalize_or_zero() * 0.004),
            direction: vec3_array(reflected),
            normal: vec3_array(normal),
            seed: effect_owner.stable_u64() ^ correlation_id.rotate_left(29),
            tags: vec!["weapon".to_owned(), "ricochet".to_owned()],
            ..Default::default()
        };
        let _ = newengine_vfx_runtime::spawn_vfx(world, request);
    }
    if let Some(tracer) = vfx.tracer {
        spawn_weapon_segment_effect(
            world,
            owner,
            tracer,
            shot_sequence,
            bounce_count,
            point + reflected.normalize_or_zero() * 0.012,
            reflected,
            remaining,
            "ricochet_tracer",
        );
    }
}

/// Publishes a semantic weapon-shot effect from the already-resolved physical muzzle.
/// Damage/collision remain authoritative in the hitscan path; transient visual composition,
/// budgets and lifetime are owned by `newengine-vfx-runtime`. Physical shell casings remain here
/// because they are persistent world objects with authored rigid-body behavior.
pub fn spawn_weapon_shot_fx(
    world: &mut World,
    owner: EntityId,
    shot_sequence: u64,
    muzzle_position: Vec3,
    muzzle_forward: Vec3,
    shot_direction: Vec3,
    range: f32,
) {
    let muzzle_forward = muzzle_forward.normalize_or_zero();
    let shot_direction = shot_direction.normalize_or_zero();
    if !muzzle_position.is_finite()
        || muzzle_forward.length_squared() <= 1.0e-8
        || shot_direction.length_squared() <= 1.0e-8
    {
        return;
    }
    let max_distance = if range.is_finite() && range > 0.0 {
        range.clamp(0.1, 1_000.0)
    } else {
        0.0
    };
    if let Some(effect) = equipped_weapon_vfx_definition(world, owner).and_then(|vfx| vfx.shot) {
        let effect_owner = equipped_weapon_entity(world, owner).unwrap_or(owner);
        let request = newengine_vfx_api::VfxSpawnRequestV1 {
            effect: newengine_vfx_api::VfxEffectRef::new(effect),
            owner: Some(newengine_vfx_api::EntityHandle::new(
                effect_owner.stable_u64(),
            )),
            correlation_id: shot_sequence,
            // Muzzle-local layers (flash/core/smoke/light) belong to the rendered barrel pose.
            // The camera-derived convergence direction must never rotate or relocate them.
            position: vec3_array(muzzle_position),
            direction: vec3_array(muzzle_forward),
            max_distance,
            seed: effect_owner.stable_u64() ^ shot_sequence.rotate_left(23),
            tags: vec!["weapon".to_owned(), "shot".to_owned()],
            ..Default::default()
        };
        if let Err(error) = newengine_vfx_runtime::spawn_vfx(world, request) {
            newengine_ulog_api::ulog::warn!(
                "project weapon shot VFX rejected owner={} shot={} err='{}'",
                owner.stable_u64(),
                shot_sequence,
                error
            );
        }
    }
    if let Some(tracer) = equipped_weapon_vfx_definition(world, owner).and_then(|vfx| vfx.tracer) {
        spawn_weapon_segment_effect(
            world,
            owner,
            tracer,
            shot_sequence,
            0,
            // Tracers follow the actual ballistic trajectory, but their visible segment starts at
            // the physical barrel instead of a camera/reticle origin.
            muzzle_position,
            shot_direction,
            max_distance,
            "tracer",
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
                weapon_entity: equipped_weapon_entity(world, owner),
                shot_sequence,
                weapon_item_id,
                // Casing fallback axes are weapon-local as well. A reticle convergence vector is not
                // a physical ejection frame.
                shot_origin: muzzle_position,
                shot_direction: muzzle_forward,
                remaining_seconds: casing.ejection_delay_seconds,
            },
        );
    }
}
fn spawn_persistent_shell_casing(
    world: &mut World,
    owner: EntityId,
    weapon_entity: Option<EntityId>,
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
    let authored_socket = weapon_entity
        .filter(|entity| world.exists(*entity))
        .and_then(|entity| world.get::<WeaponEntitySockets>(entity))
        .and_then(|sockets| sockets.casing_ejection);
    let muzzle_socket = weapon_entity
        .filter(|entity| world.exists(*entity))
        .and_then(|entity| world.get::<WeaponEntitySockets>(entity))
        .and_then(|sockets| sockets.muzzle)
        .or_else(|| {
            world
                .get::<EquippedWeaponMuzzle>(owner)
                .copied()
                .and_then(|muzzle| fallback_weapon_socket(muzzle.position, muzzle.forward))
        });
    let socket = if casing_definition.ejection_joint.is_some() {
        authored_socket.or(muzzle_socket)
    } else {
        muzzle_socket
    }
    .or_else(|| fallback_weapon_socket(fallback_origin, fallback_direction))?;

    let right = (socket.rotation * Vec3::X).normalize_or_zero();
    let up = (socket.rotation * Vec3::Y).normalize_or_zero();
    let forward = (socket.rotation * Vec3::Z).normalize_or_zero();
    if right.length_squared() <= 1.0e-8
        || up.length_squared() <= 1.0e-8
        || forward.length_squared() <= 1.0e-8
    {
        return None;
    }
    let local_vector = |value: [f32; 3]| right * value[0] + up * value[1] + forward * value[2];
    let casing_origin = socket.position + local_vector(casing_definition.origin_local);
    let velocity_local = [
        casing_definition.velocity_local[0]
            + signed_casing_noise(owner, weapon_item_id, shot_sequence, 0)
                * casing_definition.velocity_jitter[0],
        casing_definition.velocity_local[1]
            + signed_casing_noise(owner, weapon_item_id, shot_sequence, 1)
                * casing_definition.velocity_jitter[1],
        casing_definition.velocity_local[2]
            + signed_casing_noise(owner, weapon_item_id, shot_sequence, 2)
                * casing_definition.velocity_jitter[2],
    ];
    let casing_velocity = socket.linear_velocity * casing_definition.inherit_socket_linear_velocity
        + local_vector(velocity_local);
    let casing_axis = local_vector(casing_definition.axis_local).normalize_or_zero();
    let casing_axis = if casing_axis.length_squared() > 1.0e-8 {
        casing_axis
    } else {
        right
    };
    // Physics cylinders are Y-up in the provider-neutral contract. The authored shell mesh is
    // Z-long, so presentation applies a local Z->Y correction while the rigid body itself maps
    // +Y onto the ejection axis.
    let casing_rotation = Quat::from_rotation_arc(Vec3::Y, casing_axis).normalize_or_identity();
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

    let casing_radius = casing_definition.half_extents[0]
        .abs()
        .max(casing_definition.half_extents[1].abs())
        .max(0.001);
    let casing_half_height = casing_definition.half_extents[2].abs().max(0.001);
    let mut body = PhysicsBodyDesc::dynamic_solid(CollisionShapeDesc::Cylinder {
        radius: casing_radius,
        half_height: casing_half_height,
    });
    body.material.friction = casing_definition.friction;
    body.material.restitution = casing_definition.restitution;
    body.material.density = casing_definition.density;
    body.linear_damping = Some(casing_definition.linear_damping);
    body.angular_damping = Some(casing_definition.angular_damping);
    body.flags.continuous_collision = true;
    let _ = world.insert(casing, body);
    let _ = world.insert(casing, body.to_bounds());
    let _ = world.insert(casing, Velocity(casing_velocity));
    let angular_local = [
        casing_definition.angular_velocity[0]
            + signed_casing_noise(owner, weapon_item_id, shot_sequence, 3)
                * casing_definition.angular_velocity_jitter[0],
        casing_definition.angular_velocity[1]
            + signed_casing_noise(owner, weapon_item_id, shot_sequence, 4)
                * casing_definition.angular_velocity_jitter[1],
        casing_definition.angular_velocity[2]
            + signed_casing_noise(owner, weapon_item_id, shot_sequence, 5)
                * casing_definition.angular_velocity_jitter[2],
    ];
    let casing_angular_velocity = socket.angular_velocity
        * casing_definition.inherit_socket_angular_velocity
        + local_vector(angular_local);
    let _ = world.insert(casing, AngularVelocity(casing_angular_velocity));
    let _ = world.insert(
        casing,
        WeaponShellCasing::new(owner.stable_u64(), shot_sequence, weapon_item_id, variant),
    );
    let _ = world.insert(casing, WeaponShellContactRuntime::default());
    let weapon_name = world
        .resource::<ItemCatalog>()
        .and_then(|catalog| catalog.get(ItemId(weapon_item_id)))
        .map(|definition| definition.name.clone());
    if let Err(error) = emit_gameplay_event(
        world,
        GAMEPLAY_EVENT_WEAPON_SHELL_EJECTED,
        Some(owner),
        serde_json::json!({
            "schema": "newengine.gameplay.weapon_shell_event.v1",
            "version": 1,
            "weapon_item_id": weapon_item_id,
            "weapon": weapon_name,
            "shot_sequence": shot_sequence,
            "casing": casing.stable_u64(),
            "position": vec3_array(casing_origin),
            "velocity": vec3_array(casing_velocity),
            "variant": variant,
        }),
    ) {
        newengine_ulog_api::ulog::warn!(
            "weapon shell semantic event rejected owner={} shot={} err='{}'",
            owner.stable_u64(),
            shot_sequence,
            error,
        );
    }
    newengine_ulog_api::ulog::info!(
        "weapon casing ejected entity={} owner={} weapon_entity={:?} shot={} weapon_item={:016x} variant={} delay_ms={:.3} collider_cylinder=[radius={:.5},half_height={:.5}] inherited_linear={:.3} inherited_angular={:.3} physics='dynamic+damped' persistence='world' visual='authored-definition'",
        casing.stable_u64(),
        owner.stable_u64(),
        weapon_entity.map(EntityId::stable_u64),
        shot_sequence,
        weapon_item_id,
        variant,
        casing_definition.ejection_delay_seconds * 1000.0,
        casing_radius,
        casing_half_height,
        casing_definition.inherit_socket_linear_velocity,
        casing_definition.inherit_socket_angular_velocity,
    );
    Some(casing)
}
