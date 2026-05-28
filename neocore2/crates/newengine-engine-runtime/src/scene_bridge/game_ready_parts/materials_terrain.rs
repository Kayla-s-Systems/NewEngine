

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DemoMaterialRole {
    Terrain,
    Sky,
    TreeBark,
    TreeLeaf,
    TreeBranch,
}

#[derive(Clone, Copy)]
struct DemoMaterialDefinition<'a> {
    role: DemoMaterialRole,
    name: &'static str,
    base_color: [f32; 4],
    emissive: [f32; 3],
    emissive_strength: f32,
    flags: MaterialFlags,
    spec: &'a GameReadyMaterialSpec,
}

#[derive(Clone, Copy)]
struct DemoMaterials {
    terrain: MaterialId,
    sky: MaterialId,
    tree_bark: MaterialId,
    tree_leaf: MaterialId,
    tree_branch: MaterialId,
}

impl DemoMaterials {
    fn from_registered(ids: &[(DemoMaterialRole, MaterialId)]) -> Self {
        #[inline]
        fn find(ids: &[(DemoMaterialRole, MaterialId)], role: DemoMaterialRole) -> MaterialId {
            ids.iter()
                .find_map(|(candidate, id)| (*candidate == role).then_some(*id))
                .expect("all demo material roles are registered from the canonical definition table")
        }

        Self {
            terrain: find(ids, DemoMaterialRole::Terrain),
            sky: find(ids, DemoMaterialRole::Sky),
            tree_bark: find(ids, DemoMaterialRole::TreeBark),
            tree_leaf: find(ids, DemoMaterialRole::TreeLeaf),
            tree_branch: find(ids, DemoMaterialRole::TreeBranch),
        }
    }

    #[inline]
    fn sky_visual_material(self, kind: SkyVisualKind) -> MaterialId {
        match kind {
            SkyVisualKind::Dome => self.sky,
        }
    }
}

#[derive(Clone, Copy)]
struct PrimitiveSpawnSpec<'a> {
    parent: EntityId,
    primitive_id: PrimitiveId,
    material_id: MaterialId,
    name: &'a str,
    position: Vec3,
    scale: Vec3,
    color: [f32; 4],
}

#[inline]
fn spawn_game_primitive(
    world: &mut newengine_ecs::World,
    prims: &PrimitiveRegistry,
    mats: &MaterialRegistry,
    spec: PrimitiveSpawnSpec<'_>,
) -> EntityId {
    let entity = spawn_named(world, spec.name);
    let _ = newengine_transform::set_parent(world, entity, Some(spec.parent));
    let _ = world.insert(entity, Primitive { id: spec.primitive_id, color: spec.color });

    if let Some(bounds) = primitive_bounds(prims, spec.primitive_id) {
        let _ = world.insert(entity, bounds);
    }

    ensure_primitive_base(world, entity, spec.material_id);
    apply_primitive_instance(world, mats, entity, spec.material_id, spec.color);

    if let Some(t) = world.get_mut_tracked::<Transform>(entity) {
        t.position = spec.position;
        t.scale = spec.scale;
    }

    entity
}


#[inline]
fn register_demo_material_definition(
    mats: &MaterialRegistry,
    definition: DemoMaterialDefinition<'_>,
) -> (DemoMaterialRole, MaterialId) {
    let id = register_material(
        mats,
        definition.name,
        definition.base_color,
        definition.emissive,
        definition.emissive_strength,
        definition.flags,
        definition.spec,
    );
    (definition.role, id)
}

#[inline]
fn register_demo_materials(
    mats: &MaterialRegistry,
    palette: &GameReadyPaletteSpec,
    materials: &GameReadyMaterialSetSpec,
) -> DemoMaterials {
    let definitions = [
        DemoMaterialDefinition {
            role: DemoMaterialRole::Terrain,
            name: "FPS/ProceduralTerrain",
            base_color: palette.terrain,
            emissive: [0.0, 0.0, 0.0],
            emissive_strength: 1.0,
            flags: MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
            spec: &materials.terrain,
        },
        DemoMaterialDefinition {
            role: DemoMaterialRole::Sky,
            name: "FPS/SkyDome",
            base_color: palette.sky,
            emissive: palette.sky_emissive,
            emissive_strength: 2.6,
            flags: MaterialFlags::DOUBLE_SIDED,
            spec: &materials.sky,
        },
        DemoMaterialDefinition {
            role: DemoMaterialRole::TreeBark,
            name: "FPS/Tree/Bark",
            base_color: palette.tree_bark,
            emissive: [0.0, 0.0, 0.0],
            emissive_strength: 1.0,
            flags: MaterialFlags::CAST_SHADOWS.union(MaterialFlags::RECEIVE_SHADOWS),
            spec: &materials.tree_bark,
        },
        DemoMaterialDefinition {
            role: DemoMaterialRole::TreeLeaf,
            name: "FPS/Tree/Leaf",
            base_color: palette.tree_leaf,
            emissive: [0.0, 0.0, 0.0],
            emissive_strength: 1.0,
            flags: MaterialFlags::DOUBLE_SIDED
                .union(MaterialFlags::ALPHA_TEST)
                .union(MaterialFlags::CAST_SHADOWS)
                .union(MaterialFlags::RECEIVE_SHADOWS),
            spec: &materials.tree_leaf,
        },
        DemoMaterialDefinition {
            role: DemoMaterialRole::TreeBranch,
            name: "FPS/Tree/Branch",
            base_color: palette.tree_branch,
            emissive: [0.0, 0.0, 0.0],
            emissive_strength: 1.0,
            flags: MaterialFlags::DOUBLE_SIDED
                .union(MaterialFlags::CAST_SHADOWS)
                .union(MaterialFlags::RECEIVE_SHADOWS),
            spec: &materials.tree_branch,
        },
    ];

    let registered = definitions
        .into_iter()
        .map(|definition| register_demo_material_definition(mats, definition))
        .collect::<Vec<_>>();

    DemoMaterials::from_registered(&registered)
}
