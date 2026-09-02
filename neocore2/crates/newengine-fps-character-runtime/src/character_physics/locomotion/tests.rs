use super::*;
use newengine_engine_runtime::gameplay::{
    spawn_default_player, PlayerEventBus, PlayerFallState, PlayerLandingState,
    PlayerMovementSpeeds, PlayerStanceKind, PlayerStanceState,
};
use newengine_math::Vec3;

#[test]
fn authored_surface_signal_publishes_exact_project_event_id() {
    let mut world = World::new();
    let player = world.spawn();
    let surface = PhysicsSurface {
        id: "project.surface.custom".to_owned(),
        ..PhysicsSurface::default()
    }
    .with_event("contact", "project.events.boot_on_deck");
    publish_authored_surface_event(
        &mut world,
        player,
        &surface,
        "contact",
        serde_json::json!({"energy": 0.75}),
    );
    let events = newengine_engine_runtime::gameplay::drain_gameplay_events(&mut world);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].id, "project.events.boot_on_deck");
    assert_eq!(events[0].source, Some(player.stable_u64()));
    assert_eq!(events[0].payload["energy"], 0.75);
}

#[test]
fn unbound_surface_signal_is_silent_instead_of_inventing_event_id() {
    let mut world = World::new();
    let player = world.spawn();
    publish_authored_surface_event(
        &mut world,
        player,
        &PhysicsSurface::default(),
        "contact",
        serde_json::json!({"ignored": true}),
    );
    assert!(newengine_engine_runtime::gameplay::drain_gameplay_events(&mut world).is_empty());
}

fn grounded_player(world: &mut World, velocity: Vec3) -> EntityId {
    let player = spawn_default_player(world, None, "footstep-test", Vec3::new(0.0, 1.0, 0.0));
    let _ = world.insert(player, Velocity(velocity));
    if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
        ground.grounded = true;
        ground.walkable = true;
        ground.last_fixed_tick = 1;
    }
    let _ = world.insert(
        player,
        PlayerMovementSpeeds {
            walk: 2.0,
            run: 5.0,
            sprint: 9.0,
            crouch: 1.6,
        },
    );
    player
}

#[test]
fn walking_probe_glitches_never_manufacture_fall_or_landing_over_1000_frames() {
    use newengine_engine_runtime::gameplay::{
        drain_player_events, update_player_animation_states, PlayerAnimationState,
        PlayerLocomotionAnimation,
    };

    let mut world = World::new();
    let player = grounded_player(&mut world, Vec3::new(0.0, 0.0, -2.0));
    update_player_locomotion(&mut world, &BTreeMap::new(), 1.0 / 60.0);
    update_player_animation_states(&mut world, 1.0 / 60.0);
    let _ = drain_player_events(&mut world);

    for frame in 0..1000 {
        let glitch = frame % 17 == 5 || frame % 17 == 6 || frame % 17 == 7;
        if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
            ground.grounded = !glitch;
            ground.walkable = !glitch;
            if !glitch {
                ground.last_fixed_tick = ground.last_fixed_tick.saturating_add(1).max(1);
            }
        }
        if let Some(velocity) = world.get_mut::<Velocity>(player) {
            velocity.0 = Vec3::new(0.0, if glitch { -0.18 } else { 0.0 }, -2.0);
        }
        if glitch {
            if let Some(transform) = world.get_mut::<Transform>(player) {
                transform.position.y -= 0.0005;
            }
        }

        update_player_locomotion(&mut world, &BTreeMap::new(), 1.0 / 60.0);
        update_player_animation_states(&mut world, 1.0 / 60.0);

        let fall = world
            .get::<PlayerFallState>(player)
            .copied()
            .unwrap_or_default();
        assert!(
            !fall.falling,
            "probe glitch manufactured Fall at frame {frame}: {fall:?}"
        );
        let animation = world
            .get::<PlayerAnimationState>(player)
            .copied()
            .expect("player animation state");
        assert_ne!(
            animation.locomotion,
            PlayerLocomotionAnimation::Fall,
            "probe glitch selected Fall animation at frame {frame}"
        );
    }

    let events = drain_player_events(&mut world);
    assert!(!events.iter().any(|event| matches!(
        event.kind,
        PlayerEventKind::FallStarted | PlayerEventKind::FallEnded
    )));
    assert_eq!(
        world
            .get::<PlayerLandingState>(player)
            .copied()
            .unwrap_or_default()
            .revision,
        0,
        "probe glitches must not synthesize landing revisions"
    );
}

#[test]
fn falling_publishes_height_aware_lifecycle_for_animation_subscribers() {
    let mut world = World::new();
    let player = grounded_player(&mut world, Vec3::ZERO);
    if let Some(transform) = world.get_mut::<Transform>(player) {
        transform.position.y = 10.0;
    }
    update_player_locomotion(&mut world, &BTreeMap::new(), 1.0 / 60.0);

    if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
        ground.grounded = false;
        ground.walkable = false;
    }
    if let Some(velocity) = world.get_mut::<Velocity>(player) {
        velocity.0.y = 3.0;
    }
    update_player_locomotion(&mut world, &BTreeMap::new(), 1.0 / 60.0);
    if let Some(transform) = world.get_mut::<Transform>(player) {
        transform.position.y = 12.0;
    }
    update_player_locomotion(&mut world, &BTreeMap::new(), 1.0 / 60.0);

    if let Some(velocity) = world.get_mut::<Velocity>(player) {
        velocity.0.y = -6.0;
    }
    // A walk-off/physics fall needs sustained airborne evidence; a single downward tick is
    // deliberately insufficient because ground-probe chatter can produce the same signal.
    for step in 0..4 {
        if let Some(transform) = world.get_mut::<Transform>(player) {
            transform.position.y = 12.0 - (step as f32 + 1.0) * 0.875;
        }
        update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);
    }
    // The confirmation predicate observes airborne time accumulated by prior fixed steps.
    // Hold the same measured height for one more tick so this test crosses 0.35 s without
    // manufacturing extra fall distance.
    update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);

    let fall = world
        .get::<PlayerFallState>(player)
        .copied()
        .expect("fall state");
    assert!(fall.airborne && fall.falling);
    assert!((fall.peak_height - 12.0).abs() < 1.0e-4);
    assert!((fall.distance - 3.5).abs() < 1.0e-4);
    assert!(fall.downward_speed >= 6.0);
    let bus = world
        .resource::<PlayerEventBus>()
        .expect("player event bus");
    assert!(bus.events.iter().any(|event| {
        event.entity == player
            && event.kind == PlayerEventKind::FallStarted
            && event.message.contains("distance_m=3.500")
            && event.message.contains("state_component='PlayerFallState'")
    }));

    if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
        ground.grounded = true;
        ground.walkable = true;
    }
    if let Some(transform) = world.get_mut::<Transform>(player) {
        transform.position.y = 8.0;
    }
    update_player_locomotion(&mut world, &BTreeMap::new(), 1.0 / 60.0);
    let bus = world
        .resource::<PlayerEventBus>()
        .expect("player event bus");
    assert!(bus.events.iter().any(|event| {
        event.entity == player
            && event.kind == PlayerEventKind::FallEnded
            && event.message.contains("distance_m=3.500")
    }));
    let landing = world
        .get::<PlayerLandingState>(player)
        .copied()
        .expect("landing state");
    assert!((landing.distance - 3.5).abs() < 1.0e-4);
    assert!(landing.downward_speed >= 6.0);
    assert!(landing.revision > 0);
}

#[test]
fn fixed_step_contacts_alternate_left_and_right() {
    let mut world = World::new();
    let player = grounded_player(&mut world, Vec3::new(0.0, 0.0, -6.0));
    update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);
    update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);
    update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);
    update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);
    let bus = world
        .resource::<PlayerEventBus>()
        .expect("player event bus");
    let contacts = bus
        .events
        .iter()
        .filter(|event| event.entity == player && event.kind == PlayerEventKind::Footstep)
        .collect::<Vec<_>>();
    assert!(
        contacts.len() >= 2,
        "expected alternating foot contacts: {contacts:?}"
    );
    assert!(contacts[0].message.contains("foot='left'"));
    assert!(contacts[1].message.contains("foot='right'"));
}

#[test]
fn crouched_contact_reports_stealth_gait() {
    let mut world = World::new();
    let player = grounded_player(&mut world, Vec3::new(0.0, 0.0, -2.0));
    if let Some(stance) = world.get_mut::<PlayerStanceState>(player) {
        stance.current = PlayerStanceKind::Crouched;
    }
    for _ in 0..4 {
        update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);
    }
    let bus = world
        .resource::<PlayerEventBus>()
        .expect("player event bus");
    assert!(bus.events.iter().any(|event| {
        event.entity == player
            && event.kind == PlayerEventKind::Footstep
            && event.message.contains("mode='stealth'")
    }));
}
#[test]
fn rigged_model_cadence_is_driven_by_foot_contact_not_distance() {
    use newengine_engine_runtime::gameplay::{CollisionShapeDesc, PhysicsBodyDesc};
    use newengine_model_contact_api::ModelFootPoseState;
    use newengine_transform::Transform;

    let mut world = World::new();
    let player = grounded_player(&mut world, Vec3::new(0.0, 0.0, -6.0));
    if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
        ground.distance = 0.0;
        ground.normal = Vec3::Y;
    }

    let transform = world
        .get::<Transform>(player)
        .copied()
        .expect("player transform");
    let body = world
        .get::<PhysicsBodyDesc>(player)
        .copied()
        .expect("player physics body");
    let extent = match body.shape.sanitized() {
        CollisionShapeDesc::Box { half_extents } => half_extents[1],
        CollisionShapeDesc::Sphere { radius } => radius,
        CollisionShapeDesc::Capsule {
            radius,
            half_height,
        } => radius + half_height,
        CollisionShapeDesc::Cylinder { half_height, .. } => half_height,
    };
    let contact_skin = tuning(&world).contact_skin;
    let epsilon = (contact_skin.abs() * 0.25).clamp(0.001, 0.01);
    let support_y = transform.position.y - extent + epsilon;

    let high = ModelFootPoseState::from_world_positions(
        1,
        Vec3::new(-0.15, support_y + 0.14, 0.0),
        Vec3::new(0.15, support_y + 0.14, 0.0),
        None,
        0.1,
    );
    let _ = world.insert(player, high);

    // At 6 m/s the old stride accumulator would have emitted several contacts by now.
    for _ in 0..4 {
        update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);
    }
    assert!(
        world
            .resource::<PlayerEventBus>()
            .expect("player event bus")
            .events
            .iter()
            .all(|event| event.kind != PlayerEventKind::Footstep),
        "distance must not manufacture footsteps while animated feet remain airborne"
    );

    let planted = ModelFootPoseState::from_world_positions(
        2,
        Vec3::new(-0.15, support_y + 0.03, 0.0),
        Vec3::new(0.15, support_y + 0.14, 0.0),
        Some(high),
        0.1,
    );
    let _ = world.insert(player, planted);
    update_player_locomotion(&mut world, &BTreeMap::new(), 0.1);

    let bus = world
        .resource::<PlayerEventBus>()
        .expect("player event bus");
    let contacts = bus
        .events
        .iter()
        .filter(|event| event.entity == player && event.kind == PlayerEventKind::Footstep)
        .collect::<Vec<_>>();
    assert_eq!(
        contacts.len(),
        1,
        "one animated foot plant must emit one contact"
    );
    assert!(contacts[0].message.contains("source='model-contact'"));
    assert!(contacts[0].message.contains("foot='left'"));
}
#[test]
fn rigged_feet_select_independent_surface_profiles() {
    use newengine_engine_runtime::gameplay::{CollisionShapeDesc, PhysicsBodyDesc};
    use newengine_model_contact_api::{
        ModelFootGroundSample, ModelFootGroundState, ModelFootPoseState, ModelGroundPlane,
    };
    use newengine_transform::Transform;

    let mut world = World::new();
    let player = grounded_player(&mut world, Vec3::new(0.0, 0.0, -4.0));
    let wood = world.spawn();
    let stone = world.spawn();
    let _ = world.insert(
        wood,
        PhysicsSurface {
            id: "surface.wood".to_owned(),
            ..PhysicsSurface::default()
        }
        .with_event("contact", "project.contact.wood"),
    );
    let _ = world.insert(
        stone,
        PhysicsSurface {
            id: "surface.stone".to_owned(),
            ..PhysicsSurface::default()
        }
        .with_event("contact", "project.contact.stone"),
    );

    let transform = world
        .get::<Transform>(player)
        .copied()
        .expect("player transform");
    let body = world
        .get::<PhysicsBodyDesc>(player)
        .copied()
        .expect("player physics body");
    let extent = match body.shape.sanitized() {
        CollisionShapeDesc::Box { half_extents } => half_extents[1],
        CollisionShapeDesc::Sphere { radius } => radius,
        CollisionShapeDesc::Capsule {
            radius,
            half_height,
        } => radius + half_height,
        CollisionShapeDesc::Cylinder { half_height, .. } => half_height,
    };
    let contact_skin = tuning(&world).contact_skin;
    let epsilon = (contact_skin.abs() * 0.25).clamp(0.001, 0.01);
    let support_y = transform.position.y - extent + epsilon;
    if let Some(ground) = world.get_mut::<PlayerGroundState>(player) {
        ground.distance = 0.0;
        ground.normal = Vec3::Y;
        ground.ground_entity = Some(wood.stable_u64());
    }
    let _ = world.insert(
        player,
        ModelFootGroundState {
            revision: 1,
            left: ModelFootGroundSample {
                plane: ModelGroundPlane::new(Vec3::new(-0.15, support_y, 0.0), Vec3::Y),
                surface_key: Some(wood.stable_u64()),
            },
            right: ModelFootGroundSample {
                plane: ModelGroundPlane::new(Vec3::new(0.15, support_y, 0.0), Vec3::Y),
                surface_key: Some(stone.stable_u64()),
            },
        },
    );
    let keys = BTreeMap::from([(wood.stable_u64(), wood), (stone.stable_u64(), stone)]);

    let high = ModelFootPoseState::from_world_positions(
        1,
        Vec3::new(-0.15, support_y + 0.14, 0.0),
        Vec3::new(0.15, support_y + 0.14, 0.0),
        None,
        0.1,
    );
    let _ = world.insert(player, high);
    update_player_locomotion(&mut world, &keys, 0.1);

    let left_plant = ModelFootPoseState::from_world_positions(
        2,
        Vec3::new(-0.15, support_y + 0.03, 0.0),
        Vec3::new(0.15, support_y + 0.14, 0.0),
        Some(high),
        0.1,
    );
    let _ = world.insert(player, left_plant);
    update_player_locomotion(&mut world, &keys, 0.1);

    let lifted = ModelFootPoseState::from_world_positions(
        3,
        Vec3::new(-0.15, support_y + 0.14, 0.0),
        Vec3::new(0.15, support_y + 0.14, 0.0),
        Some(left_plant),
        0.1,
    );
    let _ = world.insert(player, lifted);
    update_player_locomotion(&mut world, &keys, 0.1);

    let right_plant = ModelFootPoseState::from_world_positions(
        4,
        Vec3::new(-0.15, support_y + 0.14, 0.0),
        Vec3::new(0.15, support_y + 0.03, 0.0),
        Some(lifted),
        0.1,
    );
    let _ = world.insert(player, right_plant);
    update_player_locomotion(&mut world, &keys, 0.1);

    let bus = world
        .resource::<PlayerEventBus>()
        .expect("player event bus");
    let contacts = bus
        .events
        .iter()
        .filter(|event| event.entity == player && event.kind == PlayerEventKind::Footstep)
        .collect::<Vec<_>>();
    assert_eq!(
        contacts.len(),
        2,
        "expected one contact per planted foot: {contacts:?}"
    );
    assert!(contacts[0].message.contains("foot='left'"));
    assert!(contacts[0].message.contains("surface='surface.wood'"));
    assert!(contacts[1].message.contains("foot='right'"));
    assert!(contacts[1].message.contains("surface='surface.stone'"));
}
