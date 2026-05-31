#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_lighting::{AmbientLight, DirectionalLight, PointLight};
use newengine_math::Vec3;
use newengine_render_feature_api::{LightSceneSnapshot, PackedLights, PointLightSnapshot};
use newengine_transform::GlobalTransform;

#[inline]
pub(super) fn collect_lights(world: &newengine_ecs::World) -> PackedLights {
    let snapshot = collect_light_scene_snapshot(world);
    PackedLights::from_snapshot(&snapshot)
}

#[inline]
pub(super) fn collect_light_scene_snapshot(world: &newengine_ecs::World) -> LightSceneSnapshot {
    let ambient = world.resource::<AmbientLight>().copied().unwrap_or_default();
    let directional = primary_directional_light(world);
    let mut point_lights = Vec::new();
    for (e, light, gt) in world.query2::<PointLight, GlobalTransform>() {
        let m = gt.0;
        point_lights.push(PointLightSnapshot {
            stable_id: e.stable_u64(),
            light: *light,
            position: Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.z),
        });
    }
    point_lights.sort_by(|a, b| a.stable_id.cmp(&b.stable_id));
    LightSceneSnapshot { ambient, directional, point_lights }
}

#[inline]
pub(super) fn primary_directional_light(world: &newengine_ecs::World) -> Option<DirectionalLight> {
    let mut best_dir: Option<(u64, DirectionalLight)> = None;
    for (e, l) in world.query::<DirectionalLight>() {
        let k = e.stable_u64();
        if best_dir.map(|(bk, _)| k < bk).unwrap_or(true) {
            best_dir = Some((k, *l));
        }
    }
    best_dir.map(|(_, l)| l)
}

#[inline]
pub(super) fn primary_point_light(world: &newengine_ecs::World) -> Option<(PointLight, Vec3)> {
    let mut best: Option<(u64, PointLight, Vec3)> = None;
    for (e, l, gt) in world.query2::<PointLight, GlobalTransform>() {
        let k = e.stable_u64();
        let m = gt.0;
        let pos = Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.z);
        if best.map(|(bk, _, _)| k < bk).unwrap_or(true) {
            best = Some((k, *l, pos));
        }
    }
    best.map(|(_, l, pos)| (l, pos))
}
