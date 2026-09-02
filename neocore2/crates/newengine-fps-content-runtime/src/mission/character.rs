fn normalize_character_actor_presentation_basis(
    world: &mut newengine_ecs::World,
    entity: EntityId,
) {
    if let Some(transform) = world.get_mut_tracked::<Transform>(entity) {
        transform.scale = Vec3::ONE;
    }
}

fn attach_enemy_character_foundation(
    world: &mut newengine_ecs::World,
    entity: EntityId,
    target: &AuthoredMissionTargetSpec,
) {
    let radius = target.scale.x.abs().max(target.scale.z.abs()).max(0.1);
    let half_height = (target.scale.y.abs() - radius).max(0.1);
    let shape = newengine_engine_runtime::gameplay::CollisionShapeDesc::Capsule {
        radius,
        half_height,
    };
    newengine_engine_runtime::gameplay::ensure_physics_body(
        world,
        entity,
        newengine_engine_runtime::gameplay::PhysicsBodyDesc::dynamic_solid(shape),
    );
    let _ = world.insert(entity, newengine_engine_runtime::gameplay::GameplayActor);
    let _ = world.insert(
        entity,
        newengine_engine_runtime::gameplay::CharacterBody {
            radius,
            standing_half_height: half_height,
            crouched_half_height: half_height,
            standing_eye_height: half_height,
            crouched_eye_height: half_height,
            visual_radius: radius,
            visual_half_height: target.scale.y.abs().max(radius),
        }
        .sanitized(),
    );
    let mut motor = newengine_sim::CharacterMotor::default();
    if let Some(ai) = target.ai.as_ref() {
        motor.move_speed = ai.navigation.move_speed;
    }
    let _ = world.insert(entity, motor);
    let _ = world.insert(entity, newengine_sim::MotorInput::default());
    let _ = world.insert(entity, newengine_sim::Velocity(Vec3::ZERO));
    let _ = world.insert(
        entity,
        newengine_engine_runtime::gameplay::Health::new(target.health),
    );
    let _ = world.insert(
        entity,
        newengine_engine_runtime::gameplay::CharacterLifeState::Alive,
    );
    let _ = world.insert(
        entity,
        newengine_engine_runtime::gameplay::CharacterControlState::enabled(),
    );
    let _ = world.insert(
        entity,
        newengine_engine_runtime::gameplay::DamageReceiver::character(),
    );
    let _ = world.insert(
        entity,
        newengine_engine_runtime::gameplay::DamageHitZoneMap::default(),
    );
    let _ = world.insert(
        entity,
        newengine_engine_runtime::gameplay::CharacterDamageResponseTuning::default(),
    );
    let _ = world.insert(
        entity,
        newengine_engine_runtime::gameplay::CharacterInjuryState::default(),
    );
    let _ = world.insert(
        entity,
        newengine_engine_runtime::gameplay::CharacterDeathPolicy::default(),
    );
    if let Some(character_ref) = target.character_ref.as_deref() {
        match crate::authored_world_profile::ytyp_metadata::load_character_model_assignment(
            character_ref,
        ) {
            Ok(assignment) => {
                let source = assignment.source.clone();
                // Mission target scale describes the diagnostic/physics capsule dimensions, not
                // character presentation scale. A skeletal visual is parented to this actor, so
                // leaving the authored capsule scale on the actor would non-uniformly squash the
                // complete character hierarchy (for example 0.55x/1.05y/0.55z). Once an authored
                // character assignment is admitted, keep the actor basis rigid/unit-scale and let
                // CollisionShapeDesc + CharacterBody remain authoritative for body dimensions.
                normalize_character_actor_presentation_basis(world, entity);
                let _ = world.insert(entity, assignment);
                let _ = world.insert(
                    entity,
                    newengine_engine_runtime::gameplay::PlayerModelBinding::default(),
                );
                let _ = world.insert(
                    entity,
                    newengine_engine_runtime::gameplay::PlayerAnimationState::default(),
                );
                newengine_ulog_api::ulog::info!(
                    "fps content mission character presentation requested target='{}' definition_ref='{}' model='{}'",
                    target.id,
                    character_ref,
                    source,
                );
            }
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "fps content mission character presentation unavailable target='{}' definition_ref='{}' err='{}' action='keep mission capsule fallback'",
                    target.id,
                    character_ref,
                    error,
                );
            }
        }
    }
    if let Some(ai) = target.ai.as_ref() {
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::CombatTeam::new(ai.combat_team),
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::AIController {
                enabled: true,
                decision_interval_seconds: ai.decision_interval_seconds,
                decision_cooldown_remaining: 0.0,
            }
            .sanitized(),
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::PerceptionTuning {
                sight_range: ai.sight_range,
                field_of_view_degrees: ai.field_of_view_degrees,
                memory_seconds: ai.memory_seconds,
            }
            .sanitized(),
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::PerceptionState::default(),
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::TargetMemory::default(),
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::CombatIntent::default(),
        );
        let _ = world.insert(entity, ai.navigation);
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::AINavigationState::default(),
        );
        if !ai.patrol_route.is_empty() {
            let _ = world.insert(
                entity,
                newengine_engine_runtime::gameplay::AIPatrolRoute {
                    waypoints: ai.patrol_route.clone(),
                    looping: ai.patrol_looping,
                },
            );
            let _ = world.insert(
                entity,
                newengine_engine_runtime::gameplay::AIPatrolState::default(),
            );
        }
        let _ = world.insert(entity, ai.combat);
        let _ = world.insert(entity, ai.weapon_mount);
        let _ = world.insert(
            entity,
            newengine_gameplay_fps_api::FpsActorLoadoutRequest::new(ai.loadout.clone()),
        );
    }
}
