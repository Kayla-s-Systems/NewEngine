use super::*;
use newengine_ai_api::AiFrameOutputV1;
use newengine_physics_api::PhysicsQueryHitDto;

fn spawn_character(world: &mut World, position: Vec3, team: u32) -> EntityId {
    let entity = world.spawn();
    let _ = world.insert(
        entity,
        Transform {
            position,
            ..Transform::default()
        },
    );
    let _ = world.insert(entity, CharacterBody::default());
    let _ = world.insert(entity, Health::new(100.0));
    let _ = world.insert(entity, CharacterLifeState::Alive);
    let _ = world.insert(entity, CharacterControlState::enabled());
    let _ = world.insert(entity, CombatTeam::new(team));
    entity
}

#[test]
fn perception_selects_nearest_hostile_in_range_and_fov() {
    let mut world = World::new();
    let observer = spawn_character(&mut world, Vec3::ZERO, 2);
    let friendly = spawn_character(&mut world, Vec3::new(0.0, 0.0, -2.0), 2);
    let hostile_near = spawn_character(&mut world, Vec3::new(0.0, 0.0, -4.0), 1);
    let _hostile_far = spawn_character(&mut world, Vec3::new(0.0, 0.0, -8.0), 1);
    let _ = world.insert(observer, AIController::default());
    let _ = world.insert(observer, PerceptionTuning::default());

    prepare_ai_perception(&mut world, 1.0 / 60.0);

    let probe = world
        .get::<AIPerceptionProbe>(observer)
        .expect("perception probe");
    assert_eq!(probe.target, hostile_near);
    assert_ne!(probe.target, friendly);
    assert_eq!(collect_ai_perception_queries(&world).len(), 1);
}

#[test]
fn physics_visibility_populates_memory_and_memory_expires() {
    let mut world = World::new();
    let observer = spawn_character(&mut world, Vec3::ZERO, 2);
    let target = spawn_character(&mut world, Vec3::new(0.0, 0.0, -5.0), 1);
    let _ = world.insert(observer, AIController::default());
    let _ = world.insert(
        observer,
        PerceptionTuning {
            memory_seconds: 0.5,
            ..PerceptionTuning::default()
        },
    );
    prepare_ai_perception(&mut world, 0.0);
    let probe = *world.get::<AIPerceptionProbe>(observer).unwrap();
    let hits = [PhysicsQueryHitDto {
        seq: probe.seq,
        entity: target.stable_u64(),
        position: [0.0, 0.0, -5.0],
        normal: [0.0, 0.0, 1.0],
        distance: 5.0,
        subshape_id: 0,
        hit_index: 0,
        back_face: false,
    }];
    let key_to_entity = [(target.stable_u64(), target)].into_iter().collect();
    let consumed = resolve_ai_perception_query_hits(&mut world, 1, &hits, &key_to_entity);
    assert!(consumed.contains(&probe.seq));
    let memory = world.get::<TargetMemory>(observer).copied().unwrap();
    assert_eq!(memory.target, Some(target));
    assert!(memory.visible);

    world.get_mut::<Transform>(target).unwrap().position = Vec3::new(0.0, 0.0, 50.0);
    prepare_ai_perception(&mut world, 0.25);
    prepare_ai_perception(&mut world, 0.25);
    assert_eq!(world.get::<TargetMemory>(observer).unwrap().target, None);
}

#[test]
fn ai_output_applies_engage_then_investigate_without_provider_world_access() {
    let mut world = World::new();
    let observer = spawn_character(&mut world, Vec3::ZERO, 2);
    let target = spawn_character(&mut world, Vec3::new(1.0, 0.0, -5.0), 1);
    let _ = world.insert(observer, AIController::default());
    let engage = AiIntentDtoV1 {
        intent_id: "test.engage".to_owned(),
        agent: EntityHandle::new(observer.stable_u64()),
        kind: AiIntentKind::Custom("combat.engage".to_owned()),
        target_position: None,
        path: None,
        task: None,
        animation: None,
        tags: Vec::new(),
        payload: serde_json::json!({
            "target": target.stable_u64(),
            "target_position": [1.0, 0.0, -5.0],
        }),
    };
    apply_ai_frame_output(
        &mut world,
        &AiFrameOutputV1 {
            accepted: true,
            fixed_tick: 1,
            intents: vec![engage],
            decision_trace: Vec::new(),
            diagnostics: Vec::new(),
        },
    );
    let intent = world.get::<CombatIntent>(observer).copied().unwrap();
    assert_eq!(intent.kind, CombatIntentKind::Engage);
    assert_eq!(intent.target, Some(target));

    let investigate = AiIntentDtoV1 {
        intent_id: "test.investigate".to_owned(),
        agent: EntityHandle::new(observer.stable_u64()),
        kind: AiIntentKind::Custom("combat.investigate".to_owned()),
        target_position: None,
        path: None,
        task: None,
        animation: None,
        tags: Vec::new(),
        payload: serde_json::json!({
            "target": target.stable_u64(),
            "target_position": [2.0, 0.0, -3.0],
        }),
    };
    apply_ai_frame_output(
        &mut world,
        &AiFrameOutputV1 {
            accepted: true,
            fixed_tick: 2,
            intents: vec![investigate],
            decision_trace: Vec::new(),
            diagnostics: Vec::new(),
        },
    );
    let intent = world.get::<CombatIntent>(observer).copied().unwrap();
    assert_eq!(intent.kind, CombatIntentKind::Investigate);
    assert_eq!(intent.target, Some(target));
}

#[test]
fn dead_or_disabled_ai_clears_memory_probe_and_combat_intent() {
    let mut world = World::new();
    let observer = spawn_character(&mut world, Vec3::ZERO, 2);
    let target = spawn_character(&mut world, Vec3::new(0.0, 0.0, -4.0), 1);
    let _ = world.insert(observer, AIController::default());
    let _ = world.insert(
        observer,
        TargetMemory {
            target: Some(target),
            visible: true,
            last_known_position: Vec3::new(0.0, 0.0, -4.0),
            seconds_since_seen: 0.0,
            revision: 1,
        },
    );
    let _ = world.insert(
        observer,
        CombatIntent {
            kind: CombatIntentKind::Engage,
            target: Some(target),
            target_position: Vec3::new(0.0, 0.0, -4.0),
            revision: 1,
        },
    );
    let _ = world.insert(observer, CharacterLifeState::Dead);

    prepare_ai_perception(&mut world, 1.0 / 60.0);

    assert!(world.get::<AIPerceptionProbe>(observer).is_none());
    assert_eq!(world.get::<TargetMemory>(observer).unwrap().target, None);
    assert_eq!(
        world.get::<CombatIntent>(observer).unwrap().kind,
        CombatIntentKind::Idle
    );
}
