#[inline]
fn player_capsule_ground_offset_y(world: &newengine_ecs::World, player: EntityId) -> f32 {
    if let Some(body) = world.get::<newengine_engine_runtime::gameplay::PhysicsBodyDesc>(player) {
        if let newengine_engine_runtime::gameplay::CollisionShapeDesc::Capsule {
            radius,
            half_height,
        } = body.shape.sanitized()
        {
            return -(half_height + radius);
        }
    }
    world
        .get::<newengine_engine_runtime::gameplay::CharacterBody>(player)
        .map(|body| {
            let body = body.sanitized();
            -(body.standing_half_height + body.radius)
        })
        .unwrap_or(0.0)
}

/// Keeps the authored avatar root anchored to the capsule sole while stance geometry changes.
///
/// `apply_player_stance_geometry` moves the capsule center when half-height changes so the
/// physics sole stays on the same support plane. A model root parented to that center must use
/// the *current* capsule extent as its inverse local offset; a standing-only offset makes the
/// whole avatar follow the crouched center below the floor.
pub(crate) fn tick_player_model_grounding(world: &mut newengine_ecs::World) {
    let players = world
        .query::<newengine_engine_runtime::gameplay::PlayerModelBinding>()
        .filter_map(|(player, binding)| binding.visual_root.map(|root| (player, root)))
        .collect::<Vec<_>>();

    for (player, visual_root) in players {
        if !world.exists(visual_root) {
            continue;
        }
        let local_offset = world
            .get::<newengine_engine_runtime::gameplay::PlayerModelAssignment>(player)
            .map(|assignment| assignment.local_offset)
            .unwrap_or(Vec3::ZERO);
        let grounded_local_y = local_offset.y + player_capsule_ground_offset_y(world, player);
        if let Some(transform) = world.get_mut::<Transform>(visual_root) {
            transform.position.x = local_offset.x;
            transform.position.y = grounded_local_y;
            transform.position.z = local_offset.z;
        }
    }
}

/// Applies runtime model assignment changes without replacing the PlayerActor.
/// Physics, inventory, input possession and camera targeting survive avatar swaps.
pub fn tick_player_model_assignments(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
) {
    let pending = world
        .query::<newengine_engine_runtime::gameplay::PlayerModelAssignment>()
        .filter_map(|(player, assignment)| {
            let bound_revision = world
                .get::<newengine_engine_runtime::gameplay::PlayerModelBinding>(player)
                .map(|binding| binding.assignment_revision)
                .unwrap_or(0);
            (assignment.revision != bound_revision).then_some((player, assignment.clone()))
        })
        .collect::<Vec<_>>();

    for (player, assignment) in pending {
        let ground_offset = player_capsule_ground_offset_y(world, player);
        if let Err(error) =
            bind_player_model_assignment(world, prims, mats, player, &assignment, ground_offset)
        {
            // Record the attempted revision so a bad asset does not spam every frame. Assigning
            // another model increments the revision and immediately retries with the new source.
            mark_assignment_attempted(world, player, assignment.revision);
            newengine_ulog_api::ulog::warn!(
                "fps-character: player model assignment failed player={} revision={} source='{}': {}",
                player.stable_u64(),
                assignment.revision,
                assignment.source,
                error
            );
        }
    }
}
pub fn spawn_authored_player_model(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    mats: &MaterialRegistry,
    player: EntityId,
    spec: &crate::AuthoredPlayerModelSpec,
    capsule_ground_offset_y: f32,
) -> bool {
    let requested = assignment_from_spec(spec);
    let revision = match newengine_engine_runtime::gameplay::set_player_model_assignment(
        world, player, requested,
    ) {
        Ok(revision) => revision,
        Err(error) => {
            newengine_ulog_api::ulog::warn!(
                "fps-character: player model assignment rejected player={}: {}",
                player.stable_u64(),
                error
            );
            return false;
        }
    };
    let Some(assignment) = world
        .get::<newengine_engine_runtime::gameplay::PlayerModelAssignment>(player)
        .cloned()
    else {
        return false;
    };

    match bind_player_model_assignment(
        world,
        prims,
        mats,
        player,
        &assignment,
        capsule_ground_offset_y,
    ) {
        Ok(bound) => bound,
        Err(error) => {
            mark_assignment_attempted(world, player, revision);
            newengine_ulog_api::ulog::warn!(
                "fps-character: player model binding failed revision={} source='{}': {}",
                revision,
                assignment.source,
                error
            );
            false
        }
    }
}
