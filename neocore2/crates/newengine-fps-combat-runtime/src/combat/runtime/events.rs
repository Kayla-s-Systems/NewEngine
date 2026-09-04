fn weapon_event(
    kind: WeaponEventKind,
    shooter: EntityId,
    weapon_instance_id: ItemInstanceId,
    shot_sequence: u64,
) -> WeaponEvent {
    WeaponEvent {
        kind,
        shooter,
        weapon_instance_id,
        target: None,
        shot_sequence,
        damage: 0.0,
        point: Vec3::ZERO,
        normal: Vec3::ZERO,
    }
}

fn semantic_weapon_event_id(kind: WeaponEventKind) -> &'static str {
    match kind {
        WeaponEventKind::Fired => GAMEPLAY_EVENT_WEAPON_FIRED,
        WeaponEventKind::MeleeAttacked => GAMEPLAY_EVENT_WEAPON_MELEE_ATTACKED,
        WeaponEventKind::Empty => GAMEPLAY_EVENT_WEAPON_EMPTY,
        WeaponEventKind::ReloadStarted => GAMEPLAY_EVENT_WEAPON_RELOAD_STARTED,
        WeaponEventKind::ReloadMagazineDetached
        | WeaponEventKind::ReloadAmmoCommitted
        | WeaponEventKind::ReloadMagazineInserted
        | WeaponEventKind::ReloadChambered => GAMEPLAY_EVENT_WEAPON_RELOAD_PHASE,
        WeaponEventKind::ReloadCompleted => GAMEPLAY_EVENT_WEAPON_RELOAD_COMPLETED,
        WeaponEventKind::Hit => GAMEPLAY_EVENT_WEAPON_HIT,
    }
}

#[inline]
fn reload_phase_label(kind: WeaponEventKind) -> Option<&'static str> {
    match kind {
        WeaponEventKind::ReloadStarted => Some("started"),
        WeaponEventKind::ReloadMagazineDetached => Some("magazine_detached"),
        WeaponEventKind::ReloadAmmoCommitted => Some("ammo_committed"),
        WeaponEventKind::ReloadMagazineInserted => Some("magazine_inserted"),
        WeaponEventKind::ReloadChambered => Some("chambered"),
        WeaponEventKind::ReloadCompleted => Some("completed"),
        _ => None,
    }
}

#[inline]
fn vec3_payload(value: Vec3) -> [f32; 3] {
    [value.x, value.y, value.z]
}

fn publish_weapon_project_event(world: &mut World, event: &WeaponEvent) {
    let binding = world
        .get::<EquippedWeaponBinding>(event.shooter)
        .copied()
        .filter(|binding| binding.instance_id == event.weapon_instance_id);
    let item = binding.map(|binding| binding.item);
    let item_name = item.and_then(|item| {
        world
            .resource::<ItemCatalog>()
            .and_then(|catalog| catalog.get(item))
            .map(|definition| definition.name.clone())
    });
    let state = world.get::<PlayerWeaponState>(event.shooter).copied();
    let muzzle = active_equipped_weapon_muzzle(world, event.shooter);
    let pending = matches!(
        event.kind,
        WeaponEventKind::Fired | WeaponEventKind::MeleeAttacked | WeaponEventKind::Hit
    )
    .then(|| world.get::<PendingHitscan>(event.shooter).copied())
    .flatten()
    .filter(|pending| {
        pending.weapon_instance_id == event.weapon_instance_id
            && pending.shot_sequence == event.shot_sequence
    });
    let attack_kind = pending.map(|pending| match pending.attack_kind {
        WeaponAttackKind::Firearm => "firearm",
        WeaponAttackKind::Melee => "melee",
    });
    let hit_surface = event.target.and_then(|target| {
        world
            .get::<PhysicsSurface>(target)
            .map(|surface| surface.id.clone())
    });
    let incidence_dot = pending
        .filter(|pending| {
            event.kind == WeaponEventKind::Hit && pending.attack_kind == WeaponAttackKind::Firearm
        })
        .map(|pending| ricochet_incidence_dot(pending.direction, event.normal));
    let ricochet_material = event.target.and_then(|target| {
        world
            .get::<BallisticMaterialResponse>(target)
            .copied()
            .map(BallisticMaterialResponse::sanitized)
    });
    let ricochet = pending
        .filter(|pending| {
            event.kind == WeaponEventKind::Hit && pending.attack_kind == WeaponAttackKind::Firearm
        })
        .zip(ricochet_material)
        .is_some_and(|(pending, material)| {
            ballistic_material_allows_ricochet(
                material,
                pending.direction,
                event.normal,
                pending.bounce_count,
                pending.max_bounces,
            )
        });
    let ricochet_direction = pending
        .filter(|_| ricochet)
        .map(|pending| {
            let incoming = pending.direction.normalize_or_zero();
            let normal = event.normal.normalize_or_zero();
            (incoming - normal * (2.0 * incoming.dot(normal))).normalize_or_zero()
        })
        .filter(|direction| direction.length_squared() > 1.0e-8);
    let ricochet_range = pending.filter(|_| ricochet).map(|pending| {
        let hit_distance = (event.point - pending.origin)
            .length()
            .clamp(0.0, pending.range);
        let retention = ricochet_material
            .map(|material| material.ricochet_energy_retention)
            .unwrap_or(0.0);
        (pending.range - hit_distance).max(0.0) * retention
    });

    let payload = serde_json::json!({
        "schema": "newengine.gameplay.weapon_event.v1",
        "version": 1,
        "weapon_instance_id": event.weapon_instance_id.0,
        "weapon_item_id": item.map(|item| item.raw()),
        "weapon": item_name,
        "shot_sequence": event.shot_sequence,
        "reload_phase": reload_phase_label(event.kind),
        "attack_kind": attack_kind,
        "target": event.target.map(EntityId::stable_u64),
        "damage": if event.damage > 0.0 {
            event.damage
        } else {
            pending.map(|pending| pending.damage).unwrap_or(0.0)
        },
        "point": vec3_payload(event.point),
        "normal": vec3_payload(event.normal),
        // For firearm fire/hit events the pending ray captured the exact rendered muzzle at trigger
        // time. Reuse that snapshot for VFX so flash/tracer/ballistics cannot diverge if animation
        // advances before consumers process the event.
        "muzzle_position": pending
            .filter(|pending| pending.attack_kind == WeaponAttackKind::Firearm)
            .map(|pending| vec3_payload(pending.origin))
            .or_else(|| muzzle.map(|muzzle| vec3_payload(muzzle.position))),
        "muzzle_forward": muzzle.map(|muzzle| vec3_payload(muzzle.forward)),
        "shot_origin": pending.map(|pending| vec3_payload(pending.origin)),
        "shot_direction": pending.map(|pending| vec3_payload(pending.direction)),
        "range": pending.map(|pending| pending.range),
        "bounce_count": pending.map(|pending| pending.bounce_count),
        "surface": hit_surface,
        "incidence_dot": incidence_dot,
        "ricochet": ricochet,
        "ricochet_direction": ricochet_direction.map(vec3_payload),
        "ricochet_range": ricochet_range,
        "aiming": state.map(|state| state.aiming),
        "ammo_in_magazine": state.map(|state| state.ammo_in_magazine),
        "reserve_ammo": state.map(|state| state.reserve_ammo),
    });

    let animation_event =
        binding.and_then(|binding| match (event.kind, binding.weapon.weapon_type) {
            (WeaponEventKind::Fired, WeaponType::Firearm) => Some("character.weapon.firearm.fire"),
            (WeaponEventKind::MeleeAttacked, WeaponType::Unarmed) => {
                Some("character.weapon.unarmed.attack")
            }
            (WeaponEventKind::MeleeAttacked, WeaponType::Melee) => {
                Some("character.weapon.melee.attack")
            }
            (WeaponEventKind::ReloadStarted, WeaponType::Firearm) => {
                Some("character.weapon.firearm.reload_started")
            }
            (WeaponEventKind::ReloadMagazineDetached, WeaponType::Firearm) => {
                Some("character.weapon.firearm.magazine_detached")
            }
            (WeaponEventKind::ReloadAmmoCommitted, WeaponType::Firearm) => {
                Some("character.weapon.firearm.ammo_committed")
            }
            (WeaponEventKind::ReloadMagazineInserted, WeaponType::Firearm) => {
                Some("character.weapon.firearm.magazine_inserted")
            }
            (WeaponEventKind::ReloadChambered, WeaponType::Firearm) => {
                Some("character.weapon.firearm.chambered")
            }
            (WeaponEventKind::ReloadCompleted, WeaponType::Firearm) => {
                Some("character.weapon.firearm.reload_completed")
            }
            _ => None,
        });
    if let Some(animation_event) = animation_event {
        if let Err(error) = emit_animation_pulse(
            world,
            event.shooter,
            "character.weapon.action",
            animation_event,
            payload.clone(),
        ) {
            newengine_ulog_api::ulog::warn!(
                "weapon animation semantic pulse rejected event='{}' shooter={} err='{}'",
                animation_event,
                event.shooter.stable_u64(),
                error,
            );
        }
    }

    if let Err(error) = emit_gameplay_event(
        world,
        semantic_weapon_event_id(event.kind),
        Some(event.shooter),
        payload,
    ) {
        newengine_ulog_api::ulog::warn!(
            "weapon semantic event rejected: event='{}' shooter={} err='{}'",
            semantic_weapon_event_id(event.kind),
            event.shooter.stable_u64(),
            error,
        );
    }
}

pub(super) fn emit_weapon_event(world: &mut World, event: WeaponEvent) {
    publish_weapon_project_event(world, &event);
    if world.resource::<WeaponEventBus>().is_none() {
        world.insert_resource(WeaponEventBus::default());
    }
    if let Some(bus) = world.resource_mut::<WeaponEventBus>() {
        bus.emit(event);
    }
}
pub(super) fn emit_interaction_event(world: &mut World, event: InteractionEvent) {
    if world.resource::<InteractionEventBus>().is_none() {
        world.insert_resource(InteractionEventBus::default());
    }
    if let Some(bus) = world.resource_mut::<InteractionEventBus>() {
        bus.emit(event);
    }
}
