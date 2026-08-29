use super::*;

pub fn ensure_physics_body(world: &mut World, entity: EntityId, body: PhysicsBodyDesc) {
    let _ = world.insert(entity, body);
    let _ = world.insert(entity, body.to_bounds());
    if world.get::<PhysicsSurface>(entity).is_none() {
        let _ = world.insert(entity, PhysicsSurface::default());
    }
}

#[inline]
pub fn remove_physics_body(world: &mut World, entity: EntityId) {
    let _ = world.remove::<PhysicsBodyDesc>(entity);
}

#[inline]
pub fn spawn_default_player(
    world: &mut World,
    root: Option<EntityId>,
    name: impl Into<String>,
    position: Vec3,
) -> EntityId {
    spawn_player_controller(
        world,
        root,
        name,
        position,
        CharacterBody::default(),
        CharacterMotionTuning::default(),
        true,
    )
}

/// Spawns a controller-ready character as an ordinary ECS composition.
///
/// The engine owns only generic identity, body, motion, camera/stance state and low-level
/// movement components. Product gameplay packages attach weapons, abilities, inventories,
/// mission state and other game-specific components separately.
pub fn spawn_player_controller(
    world: &mut World,
    root: Option<EntityId>,
    name: impl Into<String>,
    position: Vec3,
    body: CharacterBody,
    motion: CharacterMotionTuning,
    spawn_fallback_visual: bool,
) -> EntityId {
    let body = body.sanitized();
    let motion = motion.sanitized();
    let name = name.into();
    let e = world.spawn();

    let _ = world.insert(e, Name(name.clone()));
    let _ = world.insert(
        e,
        Transform {
            position,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
    );
    let _ = world.insert(e, GameplayActor);
    let _ = world.insert(e, PlayerActor);
    let _ = world.insert(e, PlayerController::local_input());
    let _ = world.insert(e, PlayerCommandFrame::default());
    let _ = world.insert(e, body);
    let _ = world.insert(e, motion);
    let _ = world.insert(e, PlayerMovementSpeeds::default());
    let _ = world.insert(e, Health::default());
    // Inventory storage is generic. Concrete catalogs/loadouts are installed by gameplay providers.
    ensure_player_inventory(world, e);
    let _ = world.insert(e, PlayerGroundState::default());
    let _ = world.insert(e, PlayerLocomotionState::default());
    let _ = world.insert(e, PlayerAnimationState::default());
    let _ = world.insert(e, PlayerStanceState::standing(body.standing_eye_height));
    let _ = world.insert(e, PlayerModelAssignment::default());
    let _ = world.insert(e, PlayerModelBinding::default());
    let _ = world.insert(e, CharacterMotor::default());
    let _ = world.insert(e, MotorInput::default());
    let _ = world.insert(e, Velocity(Vec3::ZERO));

    ensure_physics_body(
        world,
        e,
        PhysicsBodyDesc::dynamic_solid(CollisionShapeDesc::Capsule {
            radius: body.radius,
            half_height: body.standing_half_height,
        }),
    );

    if let Some(root) = root.filter(|id| world.exists(*id)) {
        let _ = set_parent(world, e, Some(root));
    }

    if spawn_fallback_visual {
        spawn_fallback_player_visual(world, e, &name, body);
    }

    emit_player_event(
        world,
        e,
        PlayerEventKind::Spawned,
        format!("character controller entity spawned name='{name}'"),
    );

    e
}

fn spawn_fallback_player_visual(
    world: &mut World,
    owner: EntityId,
    owner_name: &str,
    body: CharacterBody,
) -> EntityId {
    let visual = world.spawn();
    let _ = world.insert(visual, Name(format!("{owner_name}/Visual/FallbackCapsule")));
    let _ = world.insert(
        visual,
        Transform {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::new(
                body.visual_radius,
                body.visual_half_height,
                body.visual_radius,
            ),
        },
    );
    let _ = world.insert(
        visual,
        Primitive {
            id: prim_builtins::ID_CAPSULE,
            color: [0.30, 0.72, 0.98, 1.0],
        },
    );
    let _ = world.insert(visual, GameplayActor);
    let _ = world.insert(
        visual,
        PlayerVisualPart {
            owner,
            part_index: 0,
            kind: PlayerVisualKind::FallbackCapsule,
            material_slot: "fallback_capsule".to_owned(),
        },
    );
    let _ = world.insert(visual, PlayerViewVisibility::fallback_capsule_default());
    let _ = set_parent(world, visual, Some(owner));
    visual
}
