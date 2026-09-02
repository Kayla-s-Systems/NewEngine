pub fn instantiate_authored_mission(
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
