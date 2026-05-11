#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_bounds::Bounds;
use newengine_ecs::EntityId;
use newengine_materials::{MaterialId, MaterialRegistry};
use newengine_math::Vec3;
use newengine_primitives::{Primitive, PrimitiveId, PrimitiveRegistry};
use newengine_scene::components::SceneRoot;
use newengine_scene::{spawn_named, Scene, SceneState};

use crate::scene_bootstrap::bootstrap_runtime_scene;

use super::material_application::{apply_material_to_entity, MaterialApplySpec};

#[inline]
pub(super) fn place_spawn_position(base: Vec3, index: usize) -> Vec3 {
    let spacing = 1.75_f32;
    let cols = 6_usize;
    let x = (index % cols) as f32;
    let z = (index / cols) as f32;
    let cx = (cols as f32 - 1.0) * 0.5;
    base + Vec3::new((x - cx) * spacing, 0.0, z * spacing)
}

#[inline]
pub(super) fn ensure_primitive_base(world: &mut newengine_ecs::World, entity: EntityId, base: MaterialId) {
    let _ = world.insert(entity, super::imported_assets::PrimitiveMaterialBase { id: base });
}

#[inline]
pub(super) fn apply_primitive_instance(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    entity: EntityId,
    base: MaterialId,
    color: [f32; 4],
) {
    let _ = apply_material_to_entity(
        world,
        mats,
        entity,
        MaterialApplySpec::primitive_tint(base, base, color),
    );
}

#[inline]
pub(super) fn apply_exact_material(
    world: &mut newengine_ecs::World,
    mats: &MaterialRegistry,
    entity: EntityId,
    material: MaterialId,
    fallback: MaterialId,
    color: [f32; 4],
) -> MaterialId {
    apply_material_to_entity(
        world,
        mats,
        entity,
        MaterialApplySpec::exact(material, fallback, color),
    )
}

#[inline]
pub(super) fn ensure_root(scene: &mut Scene) -> EntityId {
    if let Some(root) = scene.root() {
        return root;
    }

    let cam_opt = scene.active_camera();
    let world = scene.world_mut();
    let root = spawn_named(world, "Root");
    let _ = world.insert(root, SceneRoot);

    match world.resource_mut::<SceneState>() {
        Some(st) => {
            st.root = Some(root);
            if st.active_camera.is_none() {
                st.active_camera = cam_opt;
            }
        }
        None => world.insert_resource(SceneState::new(Some(root), cam_opt)),
    }

    root
}

#[inline]
pub(super) fn primitive_bounds(reg: &PrimitiveRegistry, id: PrimitiveId) -> Option<Bounds> {
    let mesh = reg.build_mesh(id).ok()?;
    Some(Bounds::from_local_sphere(newengine_bounds::Sphere::new(
        mesh.bounds_center,
        mesh.bounds_radius.max(0.001),
    )))
}
#[inline]
pub(super) fn restore_non_collision_bounds(
    world: &mut newengine_ecs::World,
    reg: &PrimitiveRegistry,
    entity: EntityId,
) {
    if let Some(prim) = world.get::<Primitive>(entity).copied() {
        if let Some(bounds) = primitive_bounds(reg, prim.id) {
            let _ = world.insert(entity, bounds);
        }
    } else {
        let _ = world.remove::<Bounds>(entity);
    }
}


#[inline]
pub(super) fn effective_material_base(material: MaterialId, fallback: MaterialId) -> MaterialId {
    if material.is_valid() {
        material
    } else {
        fallback
    }
}


#[inline]
pub(super) fn reset_editor_runtime_state(scene: &mut Scene) -> Option<EntityId> {
    bootstrap_runtime_scene(scene);
    scene.active_camera()
}

