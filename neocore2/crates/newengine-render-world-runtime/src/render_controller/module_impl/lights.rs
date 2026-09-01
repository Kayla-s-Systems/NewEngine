#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_lighting::{AmbientLight, DirectionalLight, PointLight, SpotLight};
use newengine_math::Vec3;
use newengine_render_feature_api::{
    LightSceneSnapshot, PackedLights, PointLightSnapshot, SpotLightSnapshot,
};
use newengine_transform::GlobalTransform;

#[inline]
pub(super) fn collect_lights(world: &newengine_ecs::World) -> PackedLights {
    let snapshot = collect_light_scene_snapshot(world);
    let packed = PackedLights::from_snapshot(&snapshot);
    let packed = world
        .resource::<newengine_gameplay_world_runtime::gameplay::CloudShadowRenderState>()
        .copied()
        .map(|cloud| {
            packed.with_cloud_shadow(cloud.map0, cloud.map1, cloud.map2, cloud.map3, cloud.map4)
        })
        .unwrap_or(packed);
    world
        .resource::<newengine_gameplay_world_runtime::gameplay::SkyCloudProfileRenderState>()
        .copied()
        .map(|profile| packed.with_sky_cloud_profile(profile.profile0, profile.profile1))
        .unwrap_or(packed)
}

#[inline]
pub(super) fn collect_light_scene_snapshot(world: &newengine_ecs::World) -> LightSceneSnapshot {
    let ambient = world
        .resource::<AmbientLight>()
        .copied()
        .unwrap_or_default();
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
    point_lights.sort_by_key(|light| light.stable_id);

    let mut spot_lights = Vec::new();
    for (e, light, gt) in world.query2::<SpotLight, GlobalTransform>() {
        let m = gt.0;
        spot_lights.push(SpotLightSnapshot {
            stable_id: e.stable_u64(),
            light: *light,
            position: Vec3::new(m.w_axis.x, m.w_axis.y, m.w_axis.z),
        });
    }
    spot_lights.sort_by_key(|light| light.stable_id);

    LightSceneSnapshot {
        ambient,
        directional,
        point_lights,
        spot_lights,
    }
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
