#![forbid(unsafe_op_in_unsafe_fn)]

//! Narrow public authoring façade for application-owned world/profile packages.
//!
//! Product world crates may assemble scenes through these stable operations without importing
//! private `scene_bridge` implementation modules. The engine remains owner of generic material,
//! definition and scene-object invariant mechanics.

use newengine_bounds::Bounds;
use newengine_ecs::{EntityId, World};
use newengine_materials::{MaterialId, MaterialRegistry};
use newengine_math::Vec3;
use newengine_model_domain_api::ResolvedAssetGraphV1;
use newengine_primitives::{PrimitiveId, PrimitiveRegistry};
use newengine_scene::Scene;

pub use crate::scene_bridge::{DefinitionInstantiateTransform, DefinitionRuntimeTrace};

#[inline]
pub fn bootstrap_runtime_scene(scene: &mut Scene) {
    crate::scene_bootstrap::bootstrap_runtime_scene(scene);
}

#[inline]
pub fn bootstrap_runtime_scene_foundation(scene: &mut Scene) {
    crate::scene_bootstrap::bootstrap_runtime_scene_foundation(scene);
}

#[inline]
pub fn ensure_scene_root(scene: &mut Scene) -> EntityId {
    crate::scene_bridge::ensure_root(scene)
}

#[inline]
pub fn primitive_bounds(registry: &PrimitiveRegistry, id: PrimitiveId) -> Option<Bounds> {
    crate::scene_bridge::primitive_bounds(registry, id)
}

#[inline]
pub fn ensure_primitive_material_base(world: &mut World, entity: EntityId, base: MaterialId) {
    crate::scene_bridge::ensure_primitive_base(world, entity, base);
}

#[inline]
pub fn apply_primitive_material_instance(
    world: &mut World,
    materials: &MaterialRegistry,
    entity: EntityId,
    base: MaterialId,
    color: [f32; 4],
) {
    crate::scene_bridge::apply_primitive_instance(world, materials, entity, base, color);
}

#[inline]
pub fn apply_exact_material(
    world: &mut World,
    materials: &MaterialRegistry,
    entity: EntityId,
    material: MaterialId,
    fallback: MaterialId,
    color: [f32; 4],
) -> MaterialId {
    crate::scene_bridge::apply_exact_material(world, materials, entity, material, fallback, color)
}

#[inline]
pub fn instantiate_definition(
    world: &mut World,
    parent: Option<EntityId>,
    definition_ref: String,
    transform: DefinitionInstantiateTransform,
    graph: ResolvedAssetGraphV1,
) -> (EntityId, DefinitionRuntimeTrace) {
    crate::scene_bridge::apply_definition_instantiation(
        world,
        parent,
        definition_ref,
        transform,
        graph,
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SceneObjectInvariantSummary {
    pub checked: usize,
    pub repaired: usize,
    pub missing_transform: usize,
    pub missing_bounds: usize,
    pub missing_physics: usize,
}

pub fn validate_scene_objects(
    world: &mut World,
    phase: &'static str,
) -> SceneObjectInvariantSummary {
    let report = crate::scene_bridge::validate_scene_object_invariants(world, phase);
    SceneObjectInvariantSummary {
        checked: report.checked,
        repaired: report.repaired,
        missing_transform: report.missing_transform,
        missing_bounds: report.missing_bounds,
        missing_physics: report.missing_physics,
    }
}

#[inline]
pub fn attach_scene_object(
    world: &mut World,
    entity: EntityId,
    position: Vec3,
    half_extents: Vec3,
) {
    crate::gameplay::attach_scene_object_core(world, entity, position, half_extents);
}
