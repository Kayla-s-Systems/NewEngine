#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_math::Vec3;

#[derive(Clone, Copy, Debug)]
pub(super) struct BoundsSnap {
    pub center: Vec3,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SelectionBoundsSnap {
    pub center: Vec3,
    pub radius: f32,
}

#[inline]
pub(super) fn default_bounds() -> BoundsSnap {
    BoundsSnap {
        center: Vec3::ZERO,
        radius: 5.0,
    }
}

#[inline]
pub(super) fn scene_bounds(scene: &newengine_scene::Scene) -> Option<BoundsSnap> {
    scene_bounds_world(scene.world())
}

#[inline]
pub(super) fn scene_bounds_world(world: &newengine_ecs::World) -> Option<BoundsSnap> {
    let b = newengine_scene::scene_bounds_cached(world);
    b.sphere.map(|s| BoundsSnap {
        center: s.center,
        radius: s.radius.max(0.001),
    })
}

#[inline]
pub(super) fn selection_bounds(
    scene: &newengine_scene::Scene,
    sel: Option<newengine_ecs::EntityId>,
) -> Option<SelectionBoundsSnap> {
    selection_bounds_world(scene.world(), sel)
}

#[inline]
pub(super) fn selection_bounds_world(
    world: &newengine_ecs::World,
    sel: Option<newengine_ecs::EntityId>,
) -> Option<SelectionBoundsSnap> {
    let e = sel?;
    let b = newengine_scene::selection_world_bounds(world, [e].into_iter())?;
    let c = b.center();
    let r = b.half_extents().length().max(0.001);
    Some(SelectionBoundsSnap {
        center: c,
        radius: r,
    })
}
