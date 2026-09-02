use super::foliage::{decode_runtime_ydd_prefab, terrain_height, DecodedPrefabMeshPart};
use super::*;
use crate::content::AuthoredMissionPickupSpec;

const MISSION_STREAMING_PIN_OWNER: &str = "game-ready.mission";

#[derive(Debug, Default)]
struct MissionAssetPinSet {
    leases: std::collections::BTreeMap<String, newengine_assets::AssetStreamingPinLease>,
}

fn pin_mission_asset(world: &mut newengine_ecs::World, logical_path: &str) -> Result<(), String> {
    let logical_path = logical_path.trim().replace('\\', "/");
    if logical_path.is_empty() {
        return Ok(());
    }

    let mut pins = world
        .remove_resource::<MissionAssetPinSet>()
        .unwrap_or_default();
    if pins.leases.contains_key(&logical_path) {
        world.insert_resource(pins);
        return Ok(());
    }

    let client =
        newengine_assets::AssetServiceClient::new(newengine_plugin_host::default_host_api());
    match newengine_assets::AssetStreamingPinLease::acquire(
        client,
        logical_path.clone(),
        MISSION_STREAMING_PIN_OWNER,
        newengine_assets::AssetStreamingPinClassV1::Mission,
    ) {
        Ok(lease) => {
            pins.leases.insert(logical_path, lease);
            world.insert_resource(pins);
            Ok(())
        }
        Err(error) => {
            world.insert_resource(pins);
            Err(error)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct AuthoredMissionSpawnSummary {
    /// Mission-objective pickups only (relay cores, etc.).
    pub pickups: u32,
    /// Inventory-backed authored world pickups. These never affect mission-core totals.
    pub item_pickups: u32,
    pub targets: u32,
    pub hazards: u32,
    pub goals: u32,
}

#[derive(Clone, Debug)]
struct DeferredWorldItemPickup {
    parent: EntityId,
    terrain: EntityId,
    spec: AuthoredMissionPickupSpec,
    attempts: u32,
}

#[derive(Clone, Debug, Default)]
struct DeferredWorldItemPickups {
    pending: Vec<DeferredWorldItemPickup>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RuntimeWorldItemAdmissionState {
    attempts: u32,
}

#[derive(Clone, Copy, Debug, Default)]
struct MissionMaterials {
    core: Option<MaterialId>,
    target: Option<MaterialId>,
    hazard: Option<MaterialId>,
    goal: Option<MaterialId>,
}

fn mission_material(
    mats: &MaterialRegistry,
    role: &str,
    authored_ref: Option<&str>,
    required: bool,
) -> Result<Option<MaterialId>, String> {
    if !required {
        return Ok(None);
    }
    let reference = authored_ref
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "mission role '{}' requires a project-authored material reference",
                role
            )
        })?;
    let id = register_required_material_ref(
        mats,
        &format!("Mission/{role}"),
        MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
        reference,
    )?;
    Ok(Some(id))
}

fn register_mission_materials(
    mats: &MaterialRegistry,
    mission: &AuthoredMissionSpec,
) -> Result<MissionMaterials, String> {
    Ok(MissionMaterials {
        core: mission_material(
            mats,
            "Core",
            mission.core_material.as_deref(),
            !mission.pickups.is_empty(),
        )?,
        target: mission_material(
            mats,
            "Target",
            mission.target_material.as_deref(),
            !mission.targets.is_empty(),
        )?,
        hazard: mission_material(
            mats,
            "Hazard",
            mission.hazard_material.as_deref(),
            !mission.hazards.is_empty(),
        )?,
        goal: mission_material(
            mats,
            "Goal",
            mission.goal_material.as_deref(),
            !mission.goals.is_empty(),
        )?,
    })
}

#[inline]
fn mission_position(
    world: &newengine_ecs::World,
    terrain: EntityId,
    authored: Vec3,
    center_offset: f32,
) -> Vec3 {
    Vec3::new(
        authored.x,
        terrain_height(world, terrain, authored.x, authored.z) + authored.y + center_offset,
        authored.z,
    )
}

fn spawn_mission_primitive(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    parent: EntityId,
    material_id: MaterialId,
    primitive_id: PrimitiveId,
    name: &str,
    position: Vec3,
    scale: Vec3,
) -> EntityId {
    spawn_game_primitive(
        world,
        prims,
        mats,
        PrimitiveSpawnSpec {
            parent,
            primitive_id,
            material_id,
            name,
            position,
            scale,
            color: [1.0, 1.0, 1.0, 1.0],
            render_options: newengine_model_domain_api::MeshRenderOptions::world_opaque(),
        },
    )
}

#[path = "mission/world_items.rs"]
mod world_items;

#[cfg(test)]
use world_items::{
    scaled_world_item_half_extents, world_item_material_asset, world_item_render_options,
};
pub(super) use world_items::{tick_deferred_item_pickups, tick_runtime_world_item_visuals};

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
    target: &crate::content::AuthoredMissionTargetSpec,
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
        match super::ytyp_metadata::load_character_model_assignment(character_ref) {
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
                    "game-ready mission character presentation requested target='{}' definition_ref='{}' model='{}'",
                    target.id,
                    character_ref,
                    source,
                );
            }
            Err(error) => {
                newengine_ulog_api::ulog::warn!(
                    "game-ready mission character presentation unavailable target='{}' definition_ref='{}' err='{}' action='keep mission capsule fallback'",
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

pub(super) fn instantiate_authored_mission(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    parent: EntityId,
    terrain: EntityId,
    mission: &AuthoredMissionSpec,
) -> Result<AuthoredMissionSpawnSummary, String> {
    let mut summary = AuthoredMissionSpawnSummary::default();
    for material_ref in [
        mission.core_material.as_deref(),
        mission.target_material.as_deref(),
        mission.hazard_material.as_deref(),
        mission.goal_material.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Err(error) = pin_mission_asset(world, material_ref) {
            newengine_ulog_api::ulog::warn!(
                "authored mission asset pin failed asset='{}' class='mission' owner='{}' err='{}'",
                material_ref,
                MISSION_STREAMING_PIN_OWNER,
                error,
            );
        }
    }
    let materials = register_mission_materials(mats, mission)?;

    let mut deferred_items = Vec::new();
    for pickup in &mission.pickups {
        if pickup.item.is_some() {
            deferred_items.push(DeferredWorldItemPickup {
                parent,
                terrain,
                spec: pickup.clone(),
                attempts: 0,
            });
            summary.item_pickups = summary.item_pickups.saturating_add(1);
            continue;
        }

        let position = mission_position(world, terrain, pickup.position, pickup.scale.y.abs());
        let entity = spawn_mission_primitive(
            world,
            &*prims,
            mats,
            parent,
            materials.core.expect("mission material validated"),
            builtins::ID_SPHERE_UV,
            &format!("Mission/Pickup/{}", pickup.id),
            position,
            pickup.scale,
        );
        let _ = world.insert(
            entity,
            FpsObjectivePickup {
                radius: pickup.radius,
            },
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::Interactable::new(format!("Collect {}", pickup.id)),
        );
        summary.pickups = summary.pickups.saturating_add(1);
    }
    if !deferred_items.is_empty() {
        let mut queue = world
            .remove_resource::<DeferredWorldItemPickups>()
            .unwrap_or_default();
        queue.pending.extend(deferred_items);
        world.insert_resource(queue);
    }

    for target in &mission.targets {
        let position = mission_position(world, terrain, target.position, target.scale.y.abs());
        let entity = spawn_mission_primitive(
            world,
            &*prims,
            mats,
            parent,
            materials.target.expect("mission material validated"),
            builtins::ID_CAPSULE,
            &format!("Mission/Target/{}", target.id),
            position,
            target.scale,
        );
        attach_enemy_character_foundation(world, entity, target);
        let _ = world.insert(entity, FpsObjectiveTarget);
        summary.targets = summary.targets.saturating_add(1);
    }

    for hazard in &mission.hazards {
        let position = mission_position(world, terrain, hazard.position, hazard.scale.y.abs());
        let entity = spawn_mission_primitive(
            world,
            &*prims,
            mats,
            parent,
            materials.hazard.expect("mission material validated"),
            builtins::ID_CYLINDER,
            &format!("Mission/Hazard/{}", hazard.id),
            position,
            hazard.scale,
        );
        let _ = world.insert(
            entity,
            FpsObjectiveHazard {
                radius: hazard.radius,
            },
        );
        summary.hazards = summary.hazards.saturating_add(1);
    }

    for goal in &mission.goals {
        let position = mission_position(world, terrain, goal.position, goal.scale.y.abs() * 0.15);
        let entity = spawn_mission_primitive(
            world,
            &*prims,
            mats,
            parent,
            materials.goal.expect("mission material validated"),
            builtins::ID_TORUS,
            &format!("Mission/Goal/{}", goal.id),
            position,
            goal.scale,
        );
        let _ = world.insert(
            entity,
            FpsObjectiveGoal {
                radius: goal.radius,
            },
        );
        let _ = world.insert(
            entity,
            newengine_engine_runtime::gameplay::Interactable::new(format!(
                "Extract at {}",
                goal.id
            )),
        );
        summary.goals = summary.goals.saturating_add(1);
    }

    newengine_ulog_api::ulog::info!(
        "authored mission instantiated: pickups={} item_pickups={} targets={} hazards={} goals={} policy='all generic mission presentation materials are project-authored'",
        summary.pickups,
        summary.item_pickups,
        summary.targets,
        summary.hazards,
        summary.goals,
    );
    Ok(summary)
}

#[cfg(test)]
mod world_item_runtime_tests {
    use super::*;

    #[test]
    fn skeletal_character_presentation_does_not_inherit_capsule_scale() {
        let mut world = newengine_ecs::World::new();
        let entity = world.spawn();
        let capsule_scale = Vec3::new(0.55, 1.05, 0.55);
        let _ = world.insert(
            entity,
            Transform {
                position: Vec3::new(1.0, 2.0, 3.0),
                rotation: Quat::IDENTITY,
                scale: capsule_scale,
            },
        );
        let body = newengine_engine_runtime::gameplay::CharacterBody {
            radius: 0.55,
            standing_half_height: 0.5,
            crouched_half_height: 0.5,
            standing_eye_height: 0.5,
            crouched_eye_height: 0.5,
            visual_radius: 0.55,
            visual_half_height: 1.05,
        }
        .sanitized();
        let _ = world.insert(entity, body);

        normalize_character_actor_presentation_basis(&mut world, entity);

        assert_eq!(world.get::<Transform>(entity).unwrap().scale, Vec3::ONE);
        let preserved = *world
            .get::<newengine_engine_runtime::gameplay::CharacterBody>(entity)
            .expect("character body");
        assert!((preserved.radius - 0.55).abs() <= f32::EPSILON);
        assert!((preserved.visual_half_height - 1.05).abs() <= f32::EPSILON);
    }

    #[test]
    fn authored_ai_target_composes_shared_character_damage_and_ai_foundation() {
        let mut world = newengine_ecs::World::new();
        let entity = world.spawn();
        let target = crate::content::AuthoredMissionTargetSpec {
            id: "dummy.enemy.test".to_owned(),
            character_ref: None,
            position: Vec3::ZERO,
            health: 125.0,
            scale: Vec3::new(0.55, 1.05, 0.55),
            ai: Some(crate::content::GameReadyEnemyAiSpec {
                combat_team: 2,
                sight_range: 24.0,
                field_of_view_degrees: 110.0,
                memory_seconds: 3.0,
                decision_interval_seconds: 0.1,
                navigation: newengine_engine_runtime::gameplay::AINavigationTuning {
                    move_speed: 2.4,
                    investigate_arrival_distance: 0.8,
                    engage_standoff_distance: 8.0,
                    waypoint_arrival_distance: 0.35,
                    repath_interval_seconds: 0.35,
                    view_turn_speed_radians_per_second: 240.0_f32.to_radians(),
                },
                patrol_route: vec![Vec3::new(-2.0, 0.0, -4.0), Vec3::new(2.0, 0.0, -4.0)],
                patrol_looping: true,
                combat: newengine_gameplay_fps_api::FpsAiCombatTuning {
                    fire_distance: 22.0,
                    aim_tolerance_radians: 3.0_f32.to_radians(),
                },
                weapon_mount: newengine_gameplay_fps_api::FpsActorWeaponMountTuning {
                    local_offset: [0.20, 1.20, -0.45],
                    local_forward: [0.0, 0.0, -1.0],
                },
                loadout: "loadout.fps.default".to_owned(),
            }),
        };

        attach_enemy_character_foundation(&mut world, entity, &target);

        assert!(world
            .get::<newengine_engine_runtime::gameplay::GameplayActor>(entity)
            .is_some());
        assert!(world
            .get::<newengine_engine_runtime::gameplay::CharacterBody>(entity)
            .is_some());
        assert!(world.get::<newengine_sim::CharacterMotor>(entity).is_some());
        assert!(world
            .get::<newengine_engine_runtime::gameplay::CharacterControlState>(entity)
            .is_some_and(|state| state.enabled));
        assert_eq!(
            world
                .get::<newengine_engine_runtime::gameplay::Health>(entity)
                .expect("health")
                .current,
            125.0
        );
        assert_eq!(
            world
                .get::<newengine_engine_runtime::gameplay::CharacterLifeState>(entity)
                .copied(),
            Some(newengine_engine_runtime::gameplay::CharacterLifeState::Alive)
        );
        assert_eq!(
            world
                .get::<newengine_engine_runtime::gameplay::DamageReceiver>(entity)
                .expect("damage receiver")
                .kind,
            newengine_engine_runtime::gameplay::DamageReceiverKind::Character
        );
        assert!(world
            .get::<newengine_engine_runtime::gameplay::DamageHitZoneMap>(entity)
            .is_some());
        assert_eq!(
            world
                .get::<newengine_engine_runtime::gameplay::CombatTeam>(entity)
                .copied(),
            Some(newengine_engine_runtime::gameplay::CombatTeam::new(2))
        );
        assert!(world
            .get::<newengine_engine_runtime::gameplay::AIController>(entity)
            .is_some_and(|controller| controller.enabled));
        let perception = world
            .get::<newengine_engine_runtime::gameplay::PerceptionTuning>(entity)
            .expect("perception tuning");
        assert_eq!(perception.sight_range, 24.0);
        assert_eq!(perception.field_of_view_degrees, 110.0);
        assert_eq!(perception.memory_seconds, 3.0);
        assert!(world
            .get::<newengine_engine_runtime::gameplay::TargetMemory>(entity)
            .is_some());
        assert!(world
            .get::<newengine_engine_runtime::gameplay::CombatIntent>(entity)
            .is_some());
        assert_eq!(
            world
                .get::<newengine_engine_runtime::gameplay::PhysicsBodyDesc>(entity)
                .expect("dynamic enemy physics")
                .kind,
            newengine_physics_contracts::PhysicsBodyKind::Dynamic
        );
        assert!(world
            .get::<newengine_engine_runtime::gameplay::AINavigationState>(entity)
            .is_some());
        assert_eq!(
            world
                .get::<newengine_engine_runtime::gameplay::AINavigationTuning>(entity)
                .expect("navigation tuning")
                .move_speed,
            2.4
        );
        let patrol = world
            .get::<newengine_engine_runtime::gameplay::AIPatrolRoute>(entity)
            .expect("patrol route");
        assert_eq!(patrol.waypoints.len(), 2);
        assert!(patrol.looping);
        assert!(world
            .get::<newengine_engine_runtime::gameplay::AIPatrolState>(entity)
            .is_some());
        assert_eq!(
            world
                .get::<newengine_gameplay_fps_api::FpsAiCombatTuning>(entity)
                .expect("AI combat tuning")
                .fire_distance,
            22.0
        );
        assert_eq!(
            world
                .get::<newengine_gameplay_fps_api::FpsActorWeaponMountTuning>(entity)
                .expect("AI weapon mount")
                .local_offset,
            [0.20, 1.20, -0.45]
        );
        assert_eq!(
            world
                .get::<newengine_gameplay_fps_api::FpsActorLoadoutRequest>(entity)
                .expect("authored enemy loadout request")
                .loadout,
            "loadout.fps.default"
        );
        assert!(world
            .get::<newengine_engine_runtime::gameplay::PlayerController>(entity)
            .is_none());
    }

    #[test]
    fn world_item_material_library_is_scoped_by_mesh_slot() {
        assert_eq!(
            world_item_material_asset(Some("shared/materials/weapon_rifle.nemat"), "m00", None,)
                .as_deref(),
            Some("shared/materials/weapon_rifle.nemat@m00")
        );
        assert_eq!(
            world_item_material_asset(
                Some("shared/materials/weapon_rifle.nemat@m01"),
                "m00",
                None,
            )
            .as_deref(),
            Some("shared/materials/weapon_rifle.nemat@m01")
        );
        assert_eq!(
            world_item_material_asset(None, "m01", Some("shared/materials/weapon_rifle.nemat"))
                .as_deref(),
            Some("shared/materials/weapon_rifle.nemat@m01")
        );
    }

    #[test]
    fn dropped_rifle_physics_uses_canonical_ydd_bounds_not_pickup_box() {
        let min = Vec3::new(-0.069_917_45, -0.065_805_55, -0.372_692_38);
        let max = Vec3::new(0.120_714_34, 0.127_575_71, 0.633_752_35);
        let half = scaled_world_item_half_extents(min, max, Vec3::ONE).expect("rifle bounds");
        assert!((half.x - 0.095_315_9).abs() < 1.0e-5, "half={half:?}");
        assert!((half.y - 0.096_690_63).abs() < 1.0e-5, "half={half:?}");
        assert!((half.z - 0.503_222_35).abs() < 1.0e-5, "half={half:?}");
        assert!(
            half.z > half.x * 5.0,
            "rifle collider must stay elongated: {half:?}"
        );
    }

    #[test]
    fn authored_world_item_render_path_casts_and_receives_shadows() {
        let options = world_item_render_options();
        assert_eq!(
            options.shadow_policy,
            newengine_model_domain_api::MeshShadowPolicy::CastAndReceive
        );
    }
}
