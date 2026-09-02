#[derive(Clone, Copy, Debug)]
struct AuthoredCellRoots {
    cell: EntityId,
    render: Option<EntityId>,
    simulation: Option<EntityId>,
}

#[derive(Default)]
pub(super) struct GameReadyAuthoredMapCellRoots {
    roots: BTreeMap<CellCoord, AuthoredCellRoots>,
}

/// Reference ownership for runtime meshes contributed by streamed authored cells.
/// Domain is part of the key because render and simulation have independent residency.
#[derive(Default)]
pub(super) struct GameReadyAuthoredMapPrimitiveResidency {
    cell_primitives: BTreeMap<(CellCoord, AuthoredCellDomain), BTreeSet<PrimitiveId>>,
    ref_counts: BTreeMap<PrimitiveId, u32>,
}

pub(super) fn record_static_world_primitive_residency(
    world: &mut newengine_ecs::World,
    prefab: &AuthoredWorldPlacementSpec,
    decoded: &[super::super::foliage::DecodedPrefabMeshPart],
) {
    let Some(coord) = prefab.authored_cell else {
        return;
    };
    if world
        .resource::<GameReadyAuthoredMapPrimitiveResidency>()
        .is_none()
    {
        world.insert_resource(GameReadyAuthoredMapPrimitiveResidency::default());
    }
    let domain = static_world_prefab_domain(prefab);
    let Some(residency) = world.resource_mut::<GameReadyAuthoredMapPrimitiveResidency>() else {
        return;
    };
    let cell = residency
        .cell_primitives
        .entry((coord, domain))
        .or_default();
    for part in decoded {
        if cell.insert(part.primitive_id) {
            *residency.ref_counts.entry(part.primitive_id).or_default() += 1;
        }
    }
}

fn take_primitive_release_candidates(
    world: &mut newengine_ecs::World,
    coord: CellCoord,
    domain: AuthoredCellDomain,
    output: &mut BTreeSet<PrimitiveId>,
) {
    let Some(residency) = world.resource_mut::<GameReadyAuthoredMapPrimitiveResidency>() else {
        return;
    };
    let Some(ids) = residency.cell_primitives.remove(&(coord, domain)) else {
        return;
    };
    for id in ids {
        match residency.ref_counts.get_mut(&id) {
            Some(count) if *count > 1 => *count -= 1,
            Some(_) => {
                residency.ref_counts.remove(&id);
                output.insert(id);
            }
            None => {
                output.insert(id);
            }
        }
    }
}

fn finalize_primitive_releases(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    candidates: BTreeSet<PrimitiveId>,
) -> usize {
    if candidates.is_empty() {
        return 0;
    }

    // One liveness scan for the entire replan batch. The old implementation did
    // one full Primitive query per unloaded cell/domain.
    let active = world
        .query::<Primitive>()
        .map(|(_, primitive)| primitive.id)
        .collect::<BTreeSet<_>>();

    if world
        .resource::<newengine_engine_runtime::gameplay::PrimitiveGpuEvictionQueue>()
        .is_none()
    {
        world.insert_resource(
            newengine_engine_runtime::gameplay::PrimitiveGpuEvictionQueue::default(),
        );
    }

    let mut released = 0usize;
    for id in candidates {
        if active.contains(&id) || !prims.unregister_runtime_mesh(id) {
            continue;
        }
        if let Some(queue) =
            world.resource::<newengine_engine_runtime::gameplay::PrimitiveGpuEvictionQueue>()
        {
            queue.enqueue(id);
        }
        released = released.saturating_add(1);
    }
    released
}

#[cfg(test)]
fn release_cell_primitive_residency(
    world: &mut newengine_ecs::World,
    prims: &mut PrimitiveRegistry,
    coord: CellCoord,
    domain: AuthoredCellDomain,
) -> usize {
    let mut candidates = BTreeSet::new();
    take_primitive_release_candidates(world, coord, domain, &mut candidates);
    finalize_primitive_releases(world, prims, candidates)
}

fn ensure_cell_root(
    world: &mut newengine_ecs::World,
    parent: EntityId,
    map_ref: &str,
    coord: CellCoord,
) -> EntityId {
    if let Some(existing) = world
        .resource::<GameReadyAuthoredMapCellRoots>()
        .and_then(|registry| registry.roots.get(&coord).map(|roots| roots.cell))
        .filter(|entity| world.exists(*entity))
    {
        return existing;
    }

    let entity = spawn_named(world, format!("World/Cells/{}/{}", coord.x, coord.z));
    let _ = world.insert(entity, Transform::default());
    let _ = set_parent(world, entity, Some(parent));

    if world.resource::<GameReadyAuthoredMapCellRoots>().is_none() {
        world.insert_resource(GameReadyAuthoredMapCellRoots::default());
    }
    if let Some(registry) = world.resource_mut::<GameReadyAuthoredMapCellRoots>() {
        registry.roots.insert(
            coord,
            AuthoredCellRoots {
                cell: entity,
                render: None,
                simulation: None,
            },
        );
    }

    newengine_ulog_api::ulog::debug!(
        "authored map cell root created map='{}' cell={},{} entity={:?}",
        map_ref,
        coord.x,
        coord.z,
        entity,
    );
    entity
}

fn ensure_domain_root(
    world: &mut newengine_ecs::World,
    parent: EntityId,
    map_ref: &str,
    coord: CellCoord,
    domain: AuthoredCellDomain,
) -> EntityId {
    let cell = ensure_cell_root(world, parent, map_ref, coord);

    if let Some(existing) = world
        .resource::<GameReadyAuthoredMapCellRoots>()
        .and_then(|registry| registry.roots.get(&coord).copied())
        .and_then(|roots| match domain {
            AuthoredCellDomain::Render => roots.render,
            AuthoredCellDomain::Simulation => roots.simulation,
        })
        .filter(|entity| world.exists(*entity))
    {
        return existing;
    }

    let label = match domain {
        AuthoredCellDomain::Render => "Render",
        AuthoredCellDomain::Simulation => "Simulation",
    };
    let root = spawn_named(
        world,
        format!("World/Cells/{}/{}/{}", coord.x, coord.z, label),
    );
    let _ = world.insert(root, Transform::default());
    let _ = set_parent(world, root, Some(cell));

    if let Some(registry) = world.resource_mut::<GameReadyAuthoredMapCellRoots>() {
        if let Some(roots) = registry.roots.get_mut(&coord) {
            match domain {
                AuthoredCellDomain::Render => roots.render = Some(root),
                AuthoredCellDomain::Simulation => roots.simulation = Some(root),
            }
        }
    }
    root
}

#[inline]
pub(super) fn static_world_parent_for_prefab(
    world: &newengine_ecs::World,
    default_parent: EntityId,
    prefab: &AuthoredWorldPlacementSpec,
) -> EntityId {
    let domain = static_world_prefab_domain(prefab);
    prefab
        .authored_cell
        .and_then(|coord| {
            world
                .resource::<GameReadyAuthoredMapCellRoots>()
                .and_then(|registry| registry.roots.get(&coord).copied())
        })
        .and_then(|roots| match domain {
            AuthoredCellDomain::Render => roots.render,
            AuthoredCellDomain::Simulation => roots.simulation,
        })
        .filter(|entity| world.exists(*entity))
        .unwrap_or(default_parent)
}

fn take_domain_root(
    world: &mut newengine_ecs::World,
    coord: CellCoord,
    domain: AuthoredCellDomain,
) -> Option<EntityId> {
    let registry = world.resource_mut::<GameReadyAuthoredMapCellRoots>()?;
    let roots = registry.roots.get_mut(&coord)?;
    match domain {
        AuthoredCellDomain::Render => roots.render.take(),
        AuthoredCellDomain::Simulation => roots.simulation.take(),
    }
}

fn remove_empty_cell_root(world: &mut newengine_ecs::World, coord: CellCoord) -> Option<EntityId> {
    let removable = world
        .resource::<GameReadyAuthoredMapCellRoots>()
        .and_then(|registry| registry.roots.get(&coord))
        .is_some_and(|roots| roots.render.is_none() && roots.simulation.is_none());
    if !removable {
        return None;
    }
    world
        .resource_mut::<GameReadyAuthoredMapCellRoots>()
        .and_then(|registry| registry.roots.remove(&coord))
        .map(|roots| roots.cell)
}
