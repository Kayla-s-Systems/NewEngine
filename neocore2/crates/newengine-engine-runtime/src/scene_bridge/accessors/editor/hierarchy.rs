fn canonical_editor_actor_roots(
    world: &newengine_ecs::World,
    selected: &[EntityId],
    protected_root: Option<EntityId>,
) -> Vec<EntityId> {
    let mut roots = selected
        .iter()
        .copied()
        .filter(|entity| world.exists(*entity))
        .map(|entity| authored_editor_actor_root(world, entity).unwrap_or(entity))
        .filter(|entity| {
            protected_root != Some(*entity)
                && world
                    .get::<newengine_editor_viewport_runtime::EditorGizmoAxisComponent>(*entity)
                    .is_none()
                && world.get::<crate::gameplay::PlayerActor>(*entity).is_none()
        })
        .collect::<Vec<_>>();
    roots.sort_by_key(|entity| entity.stable_u64());
    roots.dedup();

    let root_keys = roots
        .iter()
        .map(|entity| entity.stable_u64())
        .collect::<std::collections::BTreeSet<_>>();
    roots.retain(|entity| {
        let mut cursor = world
            .get::<newengine_transform_api::Parent>(*entity)
            .map(|parent| parent.0.stable_id);
        let mut depth = 0usize;
        while let Some(parent_key) = cursor {
            if root_keys.contains(&parent_key) {
                return false;
            }
            let Some(parent) = world
                .iter_entities()
                .find(|candidate| candidate.stable_u64() == parent_key)
            else {
                break;
            };
            cursor = world
                .get::<newengine_transform_api::Parent>(parent)
                .map(|next| next.0.stable_id);
            depth += 1;
            if depth >= 128 {
                break;
            }
        }
        true
    });
    roots
}

fn authored_editor_actor_root(world: &newengine_ecs::World, entity: EntityId) -> Option<EntityId> {
    let mut cursor = Some(entity);
    let mut depth = 0usize;
    while let Some(current) = cursor {
        if world.get::<crate::gameplay::PlayerActor>(current).is_some() {
            return Some(current);
        }
        if let Some(authored) = world.get::<newengine_world_authoring_api::AuthoredMapPlacement>(current) {
            if authored.primary {
                return Some(current);
            }
            if let Some(primary) = world
                .query::<newengine_world_authoring_api::AuthoredMapPlacement>()
                .find_map(|(candidate, identity)| {
                    (identity.primary
                        && identity.map_ref == authored.map_ref
                        && identity.placement_id == authored.placement_id
                        && identity.source == authored.source)
                        .then_some(candidate)
                })
            {
                return Some(primary);
            }
        }
        let parent_key = world
            .get::<newengine_transform_api::Parent>(current)
            .map(|parent| parent.0.stable_id);
        cursor = parent_key.and_then(|key| {
            world
                .iter_entities()
                .find(|candidate| candidate.stable_u64() == key)
        });
        depth += 1;
        if depth >= 128 {
            break;
        }
    }
    None
}

fn collect_editor_actor_subtree(world: &newengine_ecs::World, roots: &[EntityId]) -> Vec<EntityId> {
    let mut keys = roots
        .iter()
        .filter(|entity| world.exists(**entity))
        .map(|entity| entity.stable_u64())
        .collect::<std::collections::BTreeSet<_>>();
    loop {
        let mut changed = false;
        for entity in world.iter_entities() {
            if keys.contains(&entity.stable_u64()) {
                continue;
            }
            if world
                .get::<newengine_transform_api::Parent>(entity)
                .is_some_and(|parent| keys.contains(&parent.0.stable_id))
            {
                changed |= keys.insert(entity.stable_u64());
            }
        }
        if !changed {
            break;
        }
    }

    let mut entities = world
        .iter_entities()
        .filter(|entity| keys.contains(&entity.stable_u64()))
        .collect::<Vec<_>>();
    entities.sort_by_key(|entity| entity.stable_u64());
    entities
}

fn editor_entity_depth_in_set(
    world: &newengine_ecs::World,
    entity: EntityId,
    roots: &[EntityId],
) -> usize {
    let root_keys = roots
        .iter()
        .map(|root| root.stable_u64())
        .collect::<std::collections::BTreeSet<_>>();
    let mut depth = 0usize;
    let mut cursor = Some(entity);
    while let Some(current) = cursor {
        if root_keys.contains(&current.stable_u64()) {
            return depth;
        }
        let parent_key = world
            .get::<newengine_transform_api::Parent>(current)
            .map(|parent| parent.0.stable_id);
        cursor = parent_key.and_then(|key| {
            world
                .iter_entities()
                .find(|candidate| candidate.stable_u64() == key)
        });
        depth += 1;
        if depth >= 128 {
            break;
        }
    }
    depth
}
