use super::*;

fn index_with_cells(coords: &[(i32, i32)]) -> newengine_assets_api::MapIndexV1 {
    let mut index = newengine_assets_api::MapIndexV1 {
        map_id: "test".to_owned(),
        cell_size: 64.0,
        cells: coords
            .iter()
            .map(|(x, z)| {
                newengine_assets_api::MapCellRefV1::canonical(
                    newengine_assets_api::MapCellCoordV1::new(*x, *z),
                )
            })
            .collect(),
        ..Default::default()
    };
    index.normalize();
    index
}

#[test]
fn desired_cell_generation_scales_with_radius_not_world_cell_count() {
    let index = index_with_cells(&[(-100, -100), (-1, -1), (0, 0), (1, 0), (1, 1), (100, 100)]);
    let mut desired = BTreeSet::new();
    append_existing_cells(
        &index,
        newengine_assets_api::MapCellCoordV1::new(0, 0),
        1,
        &mut desired,
    );
    assert!(desired.contains(&newengine_assets_api::MapCellCoordV1::new(0, 0)));
    assert!(desired.contains(&newengine_assets_api::MapCellCoordV1::new(1, 0)));
    assert!(desired.contains(&newengine_assets_api::MapCellCoordV1::new(1, 1)));
    assert!(!desired.contains(&newengine_assets_api::MapCellCoordV1::new(100, 100)));
    assert_eq!(desired.len(), 4);
}

#[test]
fn domain_root_owns_prefab_and_cell_root_despawns_as_one_subtree() {
    let mut world = newengine_ecs::World::new();
    let terrain = world.spawn();
    let _ = world.insert(terrain, Transform::default());
    world.insert_resource(GameReadyAuthoredMapCellRoots::default());
    let coord = newengine_assets_api::MapCellCoordV1::new(2, -3);
    let render_root = ensure_domain_root(
        &mut world,
        terrain,
        "maps/test.ymap@map",
        coord,
        AuthoredCellDomain::Render,
    );
    let cell_root = world
        .resource::<GameReadyAuthoredMapCellRoots>()
        .and_then(|registry| registry.roots.get(&coord).map(|roots| roots.cell))
        .expect("cell root");
    let child = world.spawn();
    let _ = world.insert(child, Transform::default());
    let _ = set_parent(&mut world, child, Some(render_root));
    let prefab = GameReadyPrefabSpec {
        id: "p".to_owned(),
        authored_map_ref: "maps/test.ymap".to_owned(),
        authored_placement_id: "p".to_owned(),
        authored_cell: Some(coord),
        authored_discrete_placement: true,
        authored_primary: true,
        source: "models/test.ydd".to_owned(),
        proxy: STATIC_WORLD_PROXY.to_owned(),
        material: String::new(),
        enabled: true,
        position: Vec3::ZERO,
        rotation_ypr: Vec3::ZERO,
        scale: Vec3::ONE,
    };
    assert_eq!(
        static_world_parent_for_prefab(&world, terrain, &prefab),
        render_root
    );
    assert_eq!(
        newengine_transform::despawn_hierarchy(&mut world, cell_root),
        3
    );
    assert!(!world.exists(cell_root));
    assert!(!world.exists(render_root));
    assert!(!world.exists(child));
    assert!(world.exists(terrain));
}

#[test]
fn render_and_simulation_domains_get_distinct_roots() {
    let mut world = newengine_ecs::World::new();
    let terrain = world.spawn();
    let _ = world.insert(terrain, Transform::default());
    world.insert_resource(GameReadyAuthoredMapCellRoots::default());
    let coord = newengine_assets_api::MapCellCoordV1::new(0, 0);
    let render = ensure_domain_root(
        &mut world,
        terrain,
        "maps/test.ymap@map",
        coord,
        AuthoredCellDomain::Render,
    );
    let simulation = ensure_domain_root(
        &mut world,
        terrain,
        "maps/test.ymap@map",
        coord,
        AuthoredCellDomain::Simulation,
    );
    assert_ne!(render, simulation);
    let roots = world
        .resource::<GameReadyAuthoredMapCellRoots>()
        .and_then(|registry| registry.roots.get(&coord).copied())
        .expect("roots");
    assert_eq!(roots.render, Some(render));
    assert_eq!(roots.simulation, Some(simulation));
}

#[test]
fn shared_cell_mesh_is_evicted_only_after_last_domain_reference() {
    let mut world = newengine_ecs::World::new();
    let mut prims = PrimitiveRegistry::new();
    let id = PrimitiveId::new(0xabc0_1234);
    prims.register_mesh(
        id,
        "shared-cell-mesh",
        PrimitiveMesh {
            vertices: vec![
                PrimitiveVertex {
                    pos: [0.0, 0.0, 0.0],
                    nrm: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                },
                PrimitiveVertex {
                    pos: [1.0, 0.0, 0.0],
                    nrm: [0.0, 1.0, 0.0],
                    uv: [1.0, 0.0],
                },
                PrimitiveVertex {
                    pos: [0.0, 0.0, 1.0],
                    nrm: [0.0, 1.0, 0.0],
                    uv: [0.0, 1.0],
                },
            ],
            indices: vec![0, 1, 2],
            bounds_center: Vec3::new(0.5, 0.0, 0.5),
            bounds_radius: 1.0,
        },
    );
    let coord = newengine_assets_api::MapCellCoordV1::new(0, 0);
    world.insert_resource(GameReadyAuthoredMapPrimitiveResidency {
        cell_primitives: BTreeMap::from([
            ((coord, AuthoredCellDomain::Render), BTreeSet::from([id])),
            (
                (coord, AuthoredCellDomain::Simulation),
                BTreeSet::from([id]),
            ),
        ]),
        ref_counts: BTreeMap::from([(id, 2)]),
    });

    assert_eq!(
        release_cell_primitive_residency(&mut world, &mut prims, coord, AuthoredCellDomain::Render,),
        0
    );
    assert!(prims.is_registered(id));

    assert_eq!(
        release_cell_primitive_residency(
            &mut world,
            &mut prims,
            coord,
            AuthoredCellDomain::Simulation,
        ),
        1
    );
    assert!(!prims.is_registered(id));
    let queue = world
        .resource::<newengine_engine_runtime::gameplay::PrimitiveGpuEvictionQueue>()
        .expect("GPU eviction queue");
    assert_eq!(queue.len(), 1);
}
